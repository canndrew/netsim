use priv_prelude::*;
use rand;

/// Used to configure gateway settings before creating a gateway with `build`.
pub struct GatewayBuilder {
    public_ip: Ipv4Addr,
    public_mac_addr: MacAddr,
    private_mac_addr: MacAddr,
    subnet: SubnetV4,
}

impl GatewayBuilder {
    /// Create a new gateway on the given subnet.
    pub fn new(subnet: SubnetV4) -> GatewayBuilder {
        let public_mac_addr = rand::random();
        let private_mac_addr = rand::random();
        GatewayBuilder {
            public_ip: Ipv4Addr::random_global(),
            public_mac_addr: public_mac_addr,
            private_mac_addr: private_mac_addr,
            subnet: subnet,
        }
    }

    /*
    pub fn subnet(&mut self, subnet: SubnetV4) -> &mut GatewayBuilder {
        self.subnet = subnet;
        self
    }
    */

    /// Build the gateway. The gateway acts as a NAT on `channel`. The returned gateway can be used
    /// to read/write NATed packets from the public side of the gateway.
    pub fn build(self, channel: EtherBox) -> Gateway {
        let private_ip = self.subnet.gateway_ip();
        let mut public_veth = VethV4::new(self.public_mac_addr, self.public_ip);
        public_veth.add_route(RouteV4::new(SubnetV4::new(ipv4!("0.0.0.0"), 0), None));
        let mut private_veth = VethV4::new(self.private_mac_addr, private_ip);
        private_veth.add_route(RouteV4::new(self.subnet, None));
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
}

/// A NAT gateway. This must be created using `GatewayBuilder`. Can be used to read/write raw
/// ethernet data on the public side of the gateway.
pub struct Gateway {
    //tcp_map: PortMap,
    udp_map: PortMap,
    channel: EtherBox,
    public_veth: VethV4,
    private_veth: VethV4,
    sending_frame: Option<EtherFrame>,
}

impl Gateway {
    /// Get the gateway's public IP address.
    pub fn public_ip(&self) -> Ipv4Addr {
        self.public_veth.ip()
    }
}

impl Stream for Gateway {
    type Item = EtherFrame;
    type Error = io::Error;

    fn poll(&mut self) -> io::Result<Async<Option<EtherFrame>>> {
        let _ = self.poll_complete()?;

        loop {
            match self.channel.poll()? {
                Async::Ready(Some(frame)) => {
                    trace!("recevied frame on private side: {:?}", frame);
                    self.private_veth.recv_frame(frame);
                },
                Async::Ready(None) => {
                    return Ok(Async::Ready(None));
                },
                Async::NotReady => break,
            }
        }

        while let Async::Ready(mut packet) = self.private_veth.next_incoming() {
            trace!("incoming IP packet from private side: {:?}", packet);
            let mut udp = match packet.payload() {
                Ipv4Payload::Udp(payload) => payload,
                _ => {
                    trace!("packet is non-udp. ignoring");
                    continue;
                },
            };

            let source_addr = SocketAddrV4::new(packet.source(), udp.source_port());
            let mapped_port = match self.udp_map.map_out(source_addr) {
                Some(port) => port,
                None => {
                    trace!("cannot map packet. ignoring");
                    continue;
                },
            };

            udp.set_source_port(mapped_port);
            packet.set_payload(Ipv4Payload::Udp(udp));
            packet.set_source(self.public_veth.ip());
            trace!("enqueing packet to send on public side: {:?}", packet);
            self.public_veth.send_packet(packet);
        }

        if let Async::Ready(frame) = self.public_veth.next_outgoing() {
            trace!("sending frame on public side: {:?}", frame);
            return Ok(Async::Ready(Some(frame)));
        }

        Ok(Async::NotReady)
    }
}

impl Sink for Gateway {
    type SinkItem = EtherFrame;
    type SinkError = io::Error;

    fn start_send(&mut self, frame: EtherFrame) -> io::Result<AsyncSink<EtherFrame>> {
        trace!("received frame on public side: {:?}", frame);
        self.public_veth.recv_frame(frame);
        
        while let Async::Ready(mut packet) = self.public_veth.next_incoming() {
            trace!("incoming IP packet from public side: {:?}", packet);
            let mut udp = match packet.payload() {
                Ipv4Payload::Udp(payload) => payload,
                _ => {
                    trace!("packet has unknown ipv4 protocol type. dropping.");
                    continue;
                },
            };

            if packet.destination() != self.public_veth.ip() {
                trace!("packet was not addressed to our public IP. dropping.");
                continue;
            }

            let unmapped_addr = match self.udp_map.map_in(udp.destination_port()) {
                Some(addr) => addr,
                None => {
                    trace!("no internal mapping for port {}. dropping.", udp.destination_port());
                    return Ok(AsyncSink::Ready);
                },
            };

            udp.set_destination_port(unmapped_addr.port());
            packet.set_payload(Ipv4Payload::Udp(udp));
            packet.set_destination(*unmapped_addr.ip());
            self.private_veth.send_packet(packet);
        }

        Ok(AsyncSink::Ready)
    }
    
    fn poll_complete(&mut self) -> io::Result<Async<()>> {
        let complete = loop {
            if let Some(frame) = self.sending_frame.take() {
                trace!("sending frame on private side: {:?}", frame);
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
    use void;

    #[test]
    fn udp_port_mapping() {
        let _ = env_logger::init();
        let mut core = unwrap!(Core::new());
        let handle = core.handle();
        let res = core.run(future::lazy(move || {
            let dest_addr = Ipv4Addr::random_global();
            trace!("dest_addr == {}", dest_addr);
            let (join_handle, gateway) = spawn::spawn_behind_gateway(&handle, move || {
                const PACKET_LEN: usize = 1024;

                let sock0 = unwrap!(UdpSocket::bind(addr!("0.0.0.0:0")));
                let sock1 = unwrap!(UdpSocket::bind(addr!("0.0.0.0:0")));

                let bytes0 = util::random_vec(PACKET_LEN);
                let bytes1 = util::random_vec(PACKET_LEN);
                trace!("sending first packet");
                let n = unwrap!(sock0.send_to(&bytes0, SocketAddrV4::new(dest_addr, 123)));
                assert_eq!(n, bytes0.len());
                trace!("sending second packet");
                let n = unwrap!(sock1.send_to(&bytes1, SocketAddrV4::new(dest_addr, 123)));
                assert_eq!(n, bytes1.len());

                let mut buffer = [0u8; PACKET_LEN + 1];
                trace!("receiving bounced first packet");
                let (n, addr) = unwrap!(sock0.recv_from(&mut buffer[..]));
                assert_eq!(n, PACKET_LEN);
                assert_eq!(addr.ip(), dest_addr);
                if buffer[..n] != bytes0[..] {
                    panic!("reply packet is mangled. ({} bytes) != ({} bytes)", n, bytes0.len());
                }
                trace!("receiving bounced second packet");
                let (n, addr) = unwrap!(sock1.recv_from(&mut buffer[..]));
                assert_eq!(n, PACKET_LEN);
                assert_eq!(addr.ip(), dest_addr);
                if buffer[..n] != bytes1[..] {
                    panic!("reply packet is mangled. ({} bytes) != ({} bytes)", n, bytes1.len());
                }
                trace!("gateway thread done");
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
                let mut packet = unwrap!(packet_opt);
                trace!("received first packet");
                let mut udp = match packet.payload() {
                    Ipv4Payload::Udp(udp) => udp,
                    payload => panic!("unexpected ipv4 payload: {:?}", payload),
                };
                let udp_source_port = udp.source_port();
                let udp_destination_port = udp.destination_port();
                udp.set_source_port(udp_destination_port);
                udp.set_destination_port(udp_source_port);
                
                let packet_source = packet.source();
                let packet_destination = packet.destination();
                assert_eq!(packet_destination, dest_addr);
                assert_eq!(packet_source, public_ip);
                packet.set_source(packet_destination);
                packet.set_destination(packet_source);
                packet.set_payload(Ipv4Payload::Udp(udp));

                let gateway_tx = match flush_gateway_tx.into_inner() {
                    FinishInner::Running(flush_gateway_tx) => flush_gateway_tx.into_inner(),
                    FinishInner::Ran(res) => unwrap!(res),
                };

                gateway_tx
                .send(packet)
                .map_err(|e| panic!("send error: {}", e))
                .and_then(move |gateway_tx| {
                    gateway_rx
                    .into_future()
                    .while_driving(flush::new(gateway_tx))
                    .map_err(|((e, _gateway_rx), flush_gateway_tx)| {
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
                let mut packet = unwrap!(packet_opt);
                trace!("received second packet");
                let mut udp = match packet.payload() {
                    Ipv4Payload::Udp(udp) => udp,
                    payload => panic!("unexpected ipv4 payload: {:?}", payload),
                };
                let udp_source_port = udp.source_port();
                let udp_destination_port = udp.destination_port();
                udp.set_source_port(udp_destination_port);
                udp.set_destination_port(udp_source_port);
                
                let packet_source = packet.source();
                let packet_destination = packet.destination();
                assert_eq!(packet_destination, dest_addr);
                assert_eq!(packet_source, public_ip);
                packet.set_source(packet_destination);
                packet.set_destination(packet_source);
                packet.set_payload(Ipv4Payload::Udp(udp));

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

