use priv_prelude::*;
use rand;

/// Used to configure gateway settings before creating a gateway with `build`.
pub struct GatewayBuilder {
    public_ip: Ipv4Addr,
    public_mac_addr: EthernetAddress,
    private_mac_addr: EthernetAddress,
    subnet: SubnetV4,
    udp_forwards: HashMap<u16, SocketAddrV4>,
}

impl GatewayBuilder {
    /// Create a new gateway on the given subnet.
    pub fn new(subnet: SubnetV4) -> GatewayBuilder {
        let public_mac_addr = EthernetAddress(rand::random());
        let private_mac_addr = EthernetAddress(rand::random());
        GatewayBuilder {
            public_ip: Ipv4Addr::random_global(),
            public_mac_addr: public_mac_addr,
            private_mac_addr: private_mac_addr,
            subnet: subnet,
            udp_forwards: HashMap::new(),
        }
    }

    /*
    pub fn subnet(&mut self, subnet: SubnetV4) -> &mut GatewayBuilder {
        self.subnet = subnet;
        self
    }
    */

    pub fn forward_udp_port(
        &mut self,
        internal_addr: SocketAddrV4,
        external_port: u16,
    ) {
        self.udp_forwards.insert(external_port, internal_addr);
    }


    /// Build the gateway. The gateway acts as a NAT on `channel`. The returned gateway can be used
    /// to read/write NATed packets from the public side of the gateway.
    pub fn build(self, channel: EtherBox) -> Gateway {
        let private_ip = self.subnet.gateway_ip();
        let mut public_veth = VethV4::new(self.public_mac_addr, self.public_ip);
        public_veth.add_route(RouteV4::new(SubnetV4::new(ipv4!("0.0.0.0"), 0), None));
        let mut private_veth = VethV4::new(self.private_mac_addr, private_ip);
        private_veth.add_route(RouteV4::new(self.subnet, None));
        let mut udp_map = PortMap::new();
        for (external_port, internal_addr) in self.udp_forwards {
            udp_map.add_mapping(external_port, internal_addr);
        }
        Gateway {
            udp_map: PortMap::new(),
            channel: channel,
            public_veth: public_veth,
            private_veth: private_veth,
            sending_frame: None,
        }
    }
}

struct PortMap {
    next_free_port: u16,
    map_out: HashMap<SocketAddrV4, u16>,
    map_in: HashMap<u16, SocketAddrV4>,
}

impl PortMap {
    pub fn new() -> PortMap {
        PortMap {
            next_free_port: 1001,
            map_out: HashMap::new(),
            map_in: HashMap::new(),
        }
    }

    pub fn map_out(&mut self, addr: SocketAddrV4) -> Option<u16> {
        match self.map_out.get(&addr) {
            Some(port) => return Some(*port),
            None => (),
        };

        if self.map_in.len() == (u16::MAX - 1000) as usize {
            return None;
        }

        let port = loop {
            let port = self.next_free_port;
            self.next_free_port = self.next_free_port.checked_add(1).unwrap_or(1000);
            if !self.map_in.contains_key(&port) {
                break port;
            }
        };

        self.map_out.insert(addr, port);
        self.map_in.insert(port, addr);
        Some(port)
    }

    pub fn map_in(&mut self, port: u16) -> Option<SocketAddrV4> {
        self.map_in.get(&port).map(|addr| *addr)
    }

    pub fn add_mapping(&mut self, external_port: u16, internal_addr: SocketAddrV4) {
        self.map_out.insert(internal_addr, external_port);
        self.map_in.insert(external_port, internal_addr);
    }
}

/// A NAT gateway. This must be created using `GatewayBuilder`. Can be used to read/write raw
/// ethernet data on the public side of the gateway.
pub struct Gateway {
    //tcp_map: PortMap,
    udp_map: PortMap,
    channel: EtherBox,
    public_veth: VethV4,
    private_veth: VethV4,
    sending_frame: Option<EthernetFrame<Bytes>>,
}

impl Gateway {
    /// Get the gateway's public IP address.
    pub fn public_ip(&self) -> Ipv4Addr {
        self.public_veth.ip()
    }
}

impl Stream for Gateway {
    type Item = EthernetFrame<Bytes>;
    type Error = io::Error;

    fn poll(&mut self) -> io::Result<Async<Option<EthernetFrame<Bytes>>>> {
        let _ = self.poll_complete()?;

        loop {
            match self.channel.poll()? {
                Async::Ready(Some(frame)) => {
                    trace!("received frame on private side: {}", PrettyPrinter::print(&frame));
                    self.private_veth.recv_frame(frame);
                },
                Async::Ready(None) => {
                    return Ok(Async::Ready(None));
                },
                Async::NotReady => break,
            }
        }

        while let Async::Ready(ipv4) = self.private_veth.next_incoming() {
            trace!("incoming IP packet from private side: {}", PrettyPrinter::print(&ipv4));
            let mut ipv4 = Ipv4Packet::new(BytesMut::from(ipv4.into_inner()));

            let hop_limit = match ipv4.hop_limit().checked_sub(1) {
                Some(n) => n,
                None => continue,
            };
            ipv4.set_hop_limit(hop_limit);

            match ipv4.protocol() {
                IpProtocol::Udp => {
                    let src_ipv4_addr = ipv4.src_addr().into();
                    let dst_ipv4_addr = ipv4.dst_addr().into();
                    let mut udp = UdpPacket::new(ipv4.payload_mut());
                    let src_addr = SocketAddrV4::new(src_ipv4_addr, udp.src_port());
                    let mapped_port = match self.udp_map.map_out(src_addr) {
                        Some(port) => port,
                        None => {
                            trace!("cannot map packet. ignoring");
                            continue;
                        },
                    };
                    udp.set_src_port(mapped_port);
                    udp.fill_checksum(&wire::IpAddress::Ipv4(self.public_veth.ip().into()), &wire::IpAddress::Ipv4(dst_ipv4_addr));
                },
                _ => {
                    trace!("packet is non-udp. ignoring");
                    continue;
                },
            };

            ipv4.set_src_addr(self.public_veth.ip().into());
            trace!("old checksum: {}", ipv4.checksum());
            ipv4.fill_checksum();
            trace!("new checksum: {}", ipv4.checksum());
            trace!("ok checksum: {}", ipv4.verify_checksum());

            let ipv4 = Ipv4Packet::new(ipv4.into_inner().freeze());
            trace!("still ok checksum: {}", ipv4.verify_checksum());

            trace!("enqueing packet to send on public side: {}", PrettyPrinter::print(&ipv4));
            self.public_veth.send_packet(ipv4);
        }

        if let Async::Ready(frame) = self.public_veth.next_outgoing() {
            trace!("sending frame on public side: {}", PrettyPrinter::print(&frame));
            return Ok(Async::Ready(Some(frame)));
        }

        Ok(Async::NotReady)
    }
}

impl Sink for Gateway {
    type SinkItem = EthernetFrame<Bytes>;
    type SinkError = io::Error;

    fn start_send(&mut self, frame: EthernetFrame<Bytes>) -> io::Result<AsyncSink<EthernetFrame<Bytes>>> {
        trace!("received frame on public side: {}", PrettyPrinter::print(&frame));
        self.public_veth.recv_frame(frame);
        
        while let Async::Ready(ipv4) = self.public_veth.next_incoming() {
            trace!("incoming IP packet from public side: {}", PrettyPrinter::print(&ipv4));

            if Ipv4Addr::from(ipv4.dst_addr()) != self.public_veth.ip() {
                trace!("packet was not addressed to our public IP. dropping.");
                continue;
            }

            let mut ipv4 = Ipv4Packet::new(BytesMut::from(ipv4.into_inner()));

            let hop_limit = match ipv4.hop_limit().checked_sub(1) {
                Some(n) => n,
                None => continue,
            };
            ipv4.set_hop_limit(hop_limit);

            let addr = match ipv4.protocol() {
                IpProtocol::Udp => {
                    let src_addr = ipv4.src_addr();
                    let mut udp = UdpPacket::new(ipv4.payload_mut());
                    let unmapped_addr = match self.udp_map.map_in(udp.dst_port()) {
                        Some(addr) => addr,
                        None => {
                            trace!("no internal mapping for port {}. dropping.", udp.dst_port());
                            return Ok(AsyncSink::Ready);
                        },
                    };
                    udp.set_dst_port(unmapped_addr.port());
                    let dst_addr = *unmapped_addr.ip();
                    udp.fill_checksum(
                        &wire::IpAddress::Ipv4(src_addr),
                        &wire::IpAddress::Ipv4(dst_addr.into()),
                    );
                    dst_addr
                },
                _ => {
                    trace!("packet has unknown ipv4 protocol type. dropping.");
                    continue;
                },
            };
            ipv4.set_dst_addr(addr.into());
            ipv4.fill_checksum();
            let ipv4 = Ipv4Packet::new(ipv4.into_inner().freeze());
            self.private_veth.send_packet(ipv4);
        }

        Ok(AsyncSink::Ready)
    }
    
    fn poll_complete(&mut self) -> io::Result<Async<()>> {
        let complete = loop {
            if let Some(frame) = self.sending_frame.take() {
                trace!("sending frame on private side: {}", PrettyPrinter::print(&frame));
                match self.channel.start_send(frame)? {
                    AsyncSink::Ready => (),
                    AsyncSink::NotReady(frame) => {
                        self.sending_frame = Some(frame);
                        break false;
                    },
                }
            }

            if let Async::Ready(frame) = self.private_veth.next_outgoing() {
                self.sending_frame = Some(frame);
            } else {
                break true;
            }
        };

        let complete = match self.channel.poll_complete()? {
            Async::Ready(()) => complete,
            Async::NotReady => false,
        };

        if complete {
            Ok(Async::Ready(()))
        } else {
            Ok(Async::NotReady)
        }
    }
}

#[cfg(test)]
mod test {
    use priv_prelude::*;
    use future_utils::FinishInner;
    use std::net::UdpSocket;
    use spawn;
    use util;
    use env_logger;
    use flush;

    #[test]
    fn udp_port_mapping() {
        let _ = env_logger::init();
        let mut core = unwrap!(Core::new());
        let handle = core.handle();
        let res = core.run(future::lazy(move || {
            let dest_addr = Ipv4Addr::random_global();
            trace!("dest_addr == {}", dest_addr);
            let (join_handle, gateway) = spawn::behind_gateway(&handle, move || {
                const PACKET_LEN: usize = 1024;

                let sock0 = unwrap!(UdpSocket::bind(addr!("0.0.0.0:0")));
                let sock1 = unwrap!(UdpSocket::bind(addr!("0.0.0.0:0")));

                let bytes0 = util::random_vec(PACKET_LEN);
                let bytes1 = util::random_vec(PACKET_LEN);
                trace!("behind gateway: sending first packet");
                let n = unwrap!(sock0.send_to(&bytes0, SocketAddrV4::new(dest_addr, 123)));
                assert_eq!(n, bytes0.len());
                trace!("behind gateway: sending second packet");
                let n = unwrap!(sock1.send_to(&bytes1, SocketAddrV4::new(dest_addr, 123)));
                assert_eq!(n, bytes1.len());

                let mut buffer = [0u8; PACKET_LEN + 1];
                trace!("behind gateway: receiving bounced first packet");
                let (n, addr) = unwrap!(sock0.recv_from(&mut buffer[..]));
                assert_eq!(n, PACKET_LEN);
                assert_eq!(addr.ip(), dest_addr);
                if buffer[..n] != bytes0[..] {
                    panic!("behind gateway: reply packet is mangled. ({} bytes) != ({} bytes)", n, bytes0.len());
                }
                trace!("behind gateway: receiving bounced second packet");
                let (n, addr) = unwrap!(sock1.recv_from(&mut buffer[..]));
                assert_eq!(n, PACKET_LEN);
                assert_eq!(addr.ip(), dest_addr);
                if buffer[..n] != bytes1[..] {
                    panic!("behind gateway: reply packet is mangled. ({} bytes) != ({} bytes)", n, bytes1.len());
                }
                trace!("behind gateway: gateway thread done");
            });

            let public_ip = gateway.public_ip();
            assert!(Ipv4AddrExt::is_global(&public_ip));

            let mut gateway_adapted = VethAdaptorV4::new(dest_addr, Box::new(gateway));
            gateway_adapted.add_route(RouteV4::new(SubnetV4::new(ipv4!("0.0.0.0"), 0), None));
            let (gateway_tx, gateway_rx) = gateway_adapted.split();
            
            gateway_rx
            .into_future()
            .while_driving(flush::new(gateway_tx))
            .map_err(|((e, _gateway_rx), _flush_gateway_tx)| {
                panic!("recv error: {}", e)
            })
            .and_then(move |((packet_opt, gateway_rx), flush_gateway_tx)| {
                let packet = unwrap!(packet_opt);
                trace!("received first packet");
                let mut udp = match packet.protocol() {
                    IpProtocol::Udp => {
                        let packet = Ipv4Packet::new(packet.as_ref());
                        UdpPacket::new(packet.payload().to_owned())
                    },
                    protocol => panic!("unexpected ipv4 payload: {:?}", protocol),
                };

                let udp_src_port = udp.src_port();
                let udp_dst_port = udp.dst_port();
                udp.set_src_port(udp_dst_port);
                udp.set_dst_port(udp_src_port);

                assert_eq!(Ipv4Addr::from(packet.src_addr()), public_ip);
                assert_eq!(Ipv4Addr::from(packet.dst_addr()), dest_addr);
                let packet = Ipv4Packet::new_udp(
                    dest_addr,
                    public_ip,
                    16,
                    &udp,
                );

                let gateway_tx = match flush_gateway_tx.into_inner() {
                    FinishInner::Running(flush_gateway_tx) => flush_gateway_tx.into_inner(),
                    FinishInner::Ran(res) => unwrap!(res),
                };

                trace!("sending packet to gateway: {}", PrettyPrinter::print(&packet));

                gateway_tx
                .send(packet)
                .map_err(|e| panic!("send error: {}", e))
                .and_then(move |gateway_tx| {
                    gateway_rx
                    .into_future()
                    .while_driving(flush::new(gateway_tx))
                    .map_err(|((e, _gateway_rx), _flush_gateway_tx)| {
                        panic!("recv error: {}", e)
                    })
                    .map(|((packet_opt, _gateway_rx), flush_gateway_tx)| {
                        let gateway_tx = match flush_gateway_tx.into_inner() {
                            FinishInner::Running(flush_gateway_tx) => flush_gateway_tx.into_inner(),
                            FinishInner::Ran(res) => unwrap!(res),
                        };
                        (gateway_tx, packet_opt)
                    })
                })
            })
            .and_then(move |(gateway_tx, packet_opt)| {
                let packet = unwrap!(packet_opt);
                let packet = Ipv4Packet::new(packet.as_ref());
                trace!("received second packet");
                let mut udp = match packet.protocol() {
                    IpProtocol::Udp => {
                        UdpPacket::new(packet.payload().to_owned())
                    },
                    protocol => panic!("unexpected ipv4 payload: {:?}", protocol),
                };

                let udp_src_port = udp.src_port();
                let udp_dst_port = udp.dst_port();
                udp.set_src_port(udp_dst_port);
                udp.set_dst_port(udp_src_port);

                let packet_source = packet.src_addr().into();
                let packet_destination = packet.dst_addr().into();
                assert_eq!(packet_destination, dest_addr);
                assert_eq!(packet_source, public_ip);
                let packet = Ipv4Packet::new_udp(
                    packet_destination,
                    packet_source,
                    16,
                    &udp,
                );

                gateway_tx
                .send(packet)
                .map_err(|e| panic!("send error: {}", e))
                .map(move |_gateway_tx| {
                    trace!("echo task done");
                    unwrap!(join_handle.join())
                })
            })
        }));
        res.void_unwrap();
    }
}

