use priv_prelude::*;
use rand;

/// Used to configure gateway settings before creating a gateway with `build`.
pub struct GatewayBuilder {
    public_ip: Ipv4Addr,
    mac_addr: MacAddr,
    subnet: SubnetV4,
}

impl GatewayBuilder {
    /// Create a new gateway on the given subnet.
    pub fn new(subnet: SubnetV4) -> GatewayBuilder {
        GatewayBuilder {
            public_ip: Ipv4Addr::random_global(),
            mac_addr: rand::random(),
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
        Gateway {
            udp_map: PortMap::new(),
            cfg: self,
            channel: channel,
            arp_table: HashMap::new(),
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
            next_free_port: 0,
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
    cfg: GatewayBuilder,
    arp_table: HashMap<Ipv4Addr, MacAddr>,
}

impl Gateway {
    /// Get the gateway's public IP address.
    pub fn public_ip(&self) -> Ipv4Addr {
        self.cfg.public_ip
    }
}

impl Stream for Gateway {
    type Item = EtherFrame;
    type Error = io::Error;

    fn poll(&mut self) -> io::Result<Async<Option<EtherFrame>>> {
        loop {
            let mut frame = match self.channel.poll()? {
                Async::Ready(Some(frame)) => frame,
                Async::Ready(None) => return Ok(Async::Ready(None)),
                Async::NotReady => return Ok(Async::NotReady),
            };
            trace!("read a frame: {:?}", frame);
            
            if !frame.destination().matches(self.cfg.mac_addr) {
                continue;
            }

            let mut ipv4 = match frame.payload() {
                EtherPayload::Ipv4(payload) => payload,
                EtherPayload::Arp(mut arp) => {
                    if arp.operation() != ArpOperation::Request {
                        continue;
                    }
                    if arp.destination_ip() != self.cfg.subnet.gateway_addr() {
                        continue;
                    }
                    self.arp_table.insert(arp.source_ip(), arp.source_mac());
                    let arp = arp.response(self.cfg.mac_addr);
                    frame.set_source(self.cfg.mac_addr);
                    frame.set_destination(arp.destination_mac());
                    frame.set_payload(EtherPayload::Arp(arp));

                    trace!("replying with arp frame: {:?}", frame);

                    // TODO: is there a better way than this to try and respond?
                    let _ = self.channel.start_send(frame);
                    let _ = self.channel.poll_complete();

                    continue;
                },
                _ => {
                    trace!("frame is non-ipv4. ignoring");
                    continue;
                },
            };

            let mut udp = match ipv4.payload() {
                Ipv4Payload::Udp(payload) => payload,
                _ => {
                    trace!("packet is non-udp. ignoring");
                    continue;
                },
            };

            let source_addr = SocketAddrV4::new(ipv4.source(), udp.source_port());
            let mapped_port = match self.udp_map.map_out(source_addr) {
                Some(port) => port,
                None => {
                    trace!("cannot map packet. ignoring");
                    continue;
                },
            };

            udp.set_source_port(mapped_port);
            ipv4.set_payload(Ipv4Payload::Udp(udp));
            ipv4.set_source(self.cfg.public_ip);
            frame.set_payload(EtherPayload::Ipv4(ipv4));
            frame.set_source(self.cfg.mac_addr);
            frame.set_destination(MacAddr::broadcast());

            return Ok(Async::Ready(Some(frame)));
        }
    }
}

impl Sink for Gateway {
    type SinkItem = EtherFrame;
    type SinkError = io::Error;

    fn start_send(&mut self, frame: EtherFrame) -> io::Result<AsyncSink<EtherFrame>> {
        trace!("sending packet in through gateway");
        let mut ipv4 = match frame.payload() {
            EtherPayload::Ipv4(payload) => payload,
            _ => {
                trace!("packet has unknown ethernet payload type. dropping.");
                return Ok(AsyncSink::Ready);
            },
        };

        let mut udp = match ipv4.payload() {
            Ipv4Payload::Udp(payload) => payload,
            _ => {
                trace!("packet has unknown ipv4 protocol type. dropping.");
                return Ok(AsyncSink::Ready);
            },
        };

        if ipv4.destination() != self.cfg.public_ip {
            trace!("packet was not addressed to our public IP. dropping.");
            return Ok(AsyncSink::Ready);
        }

        let unmapped_addr = match self.udp_map.map_in(udp.destination_port()) {
            Some(addr) => addr,
            None => {
                trace!("no internal mapping for port {}. dropping.", udp.destination_port());
                return Ok(AsyncSink::Ready);
            },
        };

        udp.set_destination_port(unmapped_addr.port());
        ipv4.set_payload(Ipv4Payload::Udp(udp));
        ipv4.set_destination(*unmapped_addr.ip());

        let mut send_frame = frame.clone();
        send_frame.set_payload(EtherPayload::Ipv4(ipv4));
        send_frame.set_source(self.cfg.mac_addr);
        send_frame.set_destination(self.arp_table.get(unmapped_addr.ip()).cloned().unwrap_or(MacAddr::broadcast()));

        match self.channel.start_send(send_frame) {
            Ok(AsyncSink::Ready) => Ok(AsyncSink::Ready),
            Ok(AsyncSink::NotReady(_send_frame)) => {
                trace!("underlying channel is not ready");
                Ok(AsyncSink::NotReady(frame))
            },
            Err(e) => {
                trace!("error on underlying channel: {}", e);
                Err(e)
            },
        }
    }
    
    fn poll_complete(&mut self) -> io::Result<Async<()>> {
        self.channel.poll_complete()
    }
}

#[cfg(test)]
mod test {
    use priv_prelude::*;
    use std::net::UdpSocket;
    use spawn;
    use util;
    use env_logger;

    #[test]
    fn udp_port_mapping() {
        let _ = env_logger::init();
        let mut core = unwrap!(Core::new());
        let handle = core.handle();
        let res = core.run(future::lazy(|| {
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

            let (gateway_tx, gateway_rx) = gateway.split();

            gateway_rx
            .into_future()
            .map_err(|(e, _gateway_rx)| {
                panic!("recv error: {}", e)
            })
            /*
            .and_then(move |(frame_opt, gateway_rx)| {
                let frame = unwrap!(frame_opt);
                match frame.payload() {
                    EtherPayload::Ipv6(..) => (),
                    _ => panic!("expected initial ipv6 packet :/"),
                };

                gateway_rx
                .into_future()
                .map_err(|(e, _gateway_rx)| {
                    panic!("recv error: {}", e)
                })
            })
            */
            .and_then(move |(frame_opt, gateway_rx)| {
                let mut frame = unwrap!(frame_opt);
                trace!("received first packet");
                let mut ipv4 = match frame.payload() {
                    EtherPayload::Ipv4(ipv4) => ipv4,
                    payload => panic!("unexpected ethernet payload: {:?}", payload),
                };
                let mut udp = match ipv4.payload() {
                    Ipv4Payload::Udp(udp) => udp,
                    payload => panic!("unexpected ipv4 payload: {:?}", payload),
                };
                let udp_source_port = udp.source_port();
                let udp_destination_port = udp.destination_port();
                udp.set_source_port(udp_destination_port);
                udp.set_destination_port(udp_source_port);
                
                let ipv4_source = ipv4.source();
                let ipv4_destination = ipv4.destination();
                assert_eq!(ipv4_destination, dest_addr);
                assert_eq!(ipv4_source, public_ip);
                ipv4.set_source(ipv4_destination);
                ipv4.set_destination(ipv4_source);
                ipv4.set_payload(Ipv4Payload::Udp(udp));

                let frame_source = frame.source();
                let frame_destination = frame.destination();
                frame.set_source(frame_destination);
                frame.set_destination(frame_source);
                frame.set_payload(EtherPayload::Ipv4(ipv4));

                gateway_tx
                .send(frame)
                .map_err(|e| panic!("send error: {}", e))
                .and_then(move |gateway_tx| {
                    gateway_rx
                    .into_future()
                    .map_err(|(e, _gateway_rx)| {
                        panic!("recv error: {}", e)
                    })
                    .map(|(frame_opt, _gateway_rx)| (gateway_tx, frame_opt))
                })
            })
            .and_then(move |(gateway_tx, frame_opt)| {
                let mut frame = unwrap!(frame_opt);
                trace!("received second packet");
                let mut ipv4 = match frame.payload() {
                    EtherPayload::Ipv4(ipv4) => ipv4,
                    payload => panic!("unexpected ethernet payload: {:?}", payload),
                };
                let mut udp = match ipv4.payload() {
                    Ipv4Payload::Udp(udp) => udp,
                    payload => panic!("unexpected ipv4 payload: {:?}", payload),
                };
                let udp_source_port = udp.source_port();
                let udp_destination_port = udp.destination_port();
                udp.set_source_port(udp_destination_port);
                udp.set_destination_port(udp_source_port);
                
                let ipv4_source = ipv4.source();
                let ipv4_destination = ipv4.destination();
                assert_eq!(ipv4_destination, dest_addr);
                assert_eq!(ipv4_source, public_ip);
                ipv4.set_source(ipv4_destination);
                ipv4.set_destination(ipv4_source);
                ipv4.set_payload(Ipv4Payload::Udp(udp));

                let frame_source = frame.source();
                let frame_destination = frame.destination();
                frame.set_source(frame_destination);
                frame.set_destination(frame_source);
                frame.set_payload(EtherPayload::Ipv4(ipv4));

                gateway_tx
                .send(frame)
                .map_err(|e| panic!("send error: {}", e))
                .map(move |_gateway_tx| unwrap!(join_handle.join()))
            })
        }));
        res.void_unwrap();
    }
}

