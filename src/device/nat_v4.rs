use priv_prelude::*;

#[derive(Debug)]
/// An Ipv4 NAT.
pub struct NatV4 {
    private_plug: Ipv4Plug,
    public_plug: Ipv4Plug,
    public_ip: Ipv4Addr,
    subnet: SubnetV4, 
    hair_pinning: bool,
    udp_map: PortMap,
    tcp_map: PortMap,
}

#[derive(Debug)]
struct PortMap {
    map_out: HashMap<SocketAddrV4, u16>,
    map_in: HashMap<u16, SocketAddrV4>,
    next_port: u16,
}

impl Default for PortMap {
    fn default() -> PortMap {
        PortMap {
            map_out: HashMap::new(),
            map_in: HashMap::new(),
            next_port: 1000,
        }
    }
}

impl PortMap {
    pub fn new() -> PortMap {
        PortMap::default()
    }

    pub fn forward_port(&mut self, port: u16, local_addr: SocketAddrV4) {
        self.map_out.insert(local_addr, port);
        self.map_in.insert(port, local_addr);
    }

    pub fn get_inbound_addr(&self, port: u16) -> Option<SocketAddrV4> {
        self.map_in.get(&port).map(|x| *x)
    }

    pub fn map_port(&mut self, addr: SocketAddrV4) -> u16 {
        match self.map_out.entry(addr) {
            hash_map::Entry::Occupied(oe) => *oe.get(),
            hash_map::Entry::Vacant(ve) => {
                let port = loop {
                    if self.map_in.contains_key(&self.next_port) {
                        self.next_port += 1;
                        continue;
                    }
                    break self.next_port;
                };
                ve.insert(port);
                self.map_in.insert(port, addr);
                self.next_port = port.checked_add(1).unwrap_or(1000);
                port
            },
        }
    }
}

impl NatV4 {
    /// Create a new Ipv4 NAT
    pub fn new(
        public_plug: Ipv4Plug,
        private_plug: Ipv4Plug,
        public_ip: Ipv4Addr,
        subnet: SubnetV4,
    ) -> NatV4 {
        let ret = NatV4 {
            private_plug: private_plug,
            public_plug: public_plug,
            public_ip: public_ip,
            subnet: subnet,
            hair_pinning: false,
            udp_map: PortMap::new(),
            tcp_map: PortMap::new(),
        };
        debug!("building {:?}", ret);
        ret
    }

    /// Create a new Ipv4 NAT, spawning it directly onto the tokio event loop.
    pub fn spawn(
        handle: &Handle,
        public_plug: Ipv4Plug,
        private_plug: Ipv4Plug,
        public_ip: Ipv4Addr,
        subnet: SubnetV4,
    ) {
        let nat_v4 = NatV4::new(public_plug, private_plug, public_ip, subnet);
        handle.spawn(nat_v4.infallible());
    }
}

#[derive(Default)]
/// A builder for `NatV4`
pub struct NatV4Builder {
    subnet: Option<SubnetV4>,
    hair_pinning: bool,
    udp_map: PortMap,
    tcp_map: PortMap,
}

impl NatV4Builder {
    /// Start building an Ipv4 NAT
    pub fn new() -> NatV4Builder {
        NatV4Builder::default()
    }

    /// Set the subnet used on the local side of the NAT. If left unset, a random subnet will be
    /// chosen.
    pub fn subnet(mut self, subnet: SubnetV4) -> NatV4Builder {
        self.subnet = Some(subnet);
        self
    }

    /// Get the subnet set by the last call to `subnet` (if any).
    pub fn get_subnet(&self) -> Option<SubnetV4> {
        self.subnet
    }

    /// Enable/disable hair-pinning.
    pub fn hair_pinning(mut self, hair_pinning: bool) -> NatV4Builder {
        self.hair_pinning = hair_pinning;
        self
    }

    /// Manually forward a UDP port.
    pub fn forward_udp_port(mut self, port: u16, local_addr: SocketAddrV4) -> NatV4Builder {
        self.udp_map.forward_port(port, local_addr);
        self
    }

    /// Manually forward a TCP port.
    pub fn forward_tcp_port(mut self, port: u16, local_addr: SocketAddrV4) -> NatV4Builder {
        self.tcp_map.forward_port(port, local_addr);
        self
    }

    /// Build the NAT
    pub fn build(
        self, 
        public_plug: Ipv4Plug,
        private_plug: Ipv4Plug,
        public_ip: Ipv4Addr,
    ) -> NatV4 {
        let subnet = self.subnet.unwrap_or_else(SubnetV4::random_local);
        let ret = NatV4 {
            private_plug: private_plug,
            public_plug: public_plug,
            public_ip: public_ip,
            subnet: subnet, 
            hair_pinning: self.hair_pinning,
            udp_map: self.udp_map,
            tcp_map: self.tcp_map,
        };
        debug!("building {:?}", ret);
        ret
    }

    /// Build the NAT, spawning it directly onto the tokio event loop.
    pub fn spawn(
        self,
        handle: &Handle,
        public_plug: Ipv4Plug,
        private_plug: Ipv4Plug,
        public_ip: Ipv4Addr,
    ) {
        let nat_v4 = self.build(public_plug, private_plug, public_ip);
        handle.spawn(nat_v4.infallible());
    }
}

impl Future for NatV4 {
    type Item = ();
    type Error = Void;

    fn poll(&mut self) -> Result<Async<()>, Void> {
        let private_unplugged = loop {
            match self.private_plug.rx.poll().void_unwrap() {
                Async::NotReady => break false,
                Async::Ready(None) => break true,
                Async::Ready(Some(packet)) => {
                    let source_ip = packet.source_ip();
                    let dest_ip = packet.dest_ip();
                    let ipv4_fields = packet.fields();

                    if !self.subnet.contains(source_ip) {
                        info!("nat {:?} dropping outbound packet which does not originate from our \
                               subnet. {:?}", self.public_ip, packet);
                        continue;
                    }

                    let next_ttl = match ipv4_fields.ttl.checked_sub(1) {
                        Some(ttl) => ttl,
                        None => {
                            info!(
                                "nat {:?} dropping outbound packet with ttl zero {:?}",
                                self.public_ip, packet
                            );
                            continue;
                        },
                    };

                    if self.hair_pinning && dest_ip == self.public_ip {
                        match packet.payload() {
                            Ipv4Payload::Udp(udp) => {
                                let dest_port = udp.dest_port();
                                let private_dest_addr = match self.udp_map.get_inbound_addr(dest_port) {
                                    Some(addr) => addr,
                                    None => continue,
                                };

                                let bounced_packet = Ipv4Packet::new_from_fields_recursive(
                                    Ipv4Fields {
                                        dest_ip: *private_dest_addr.ip(),
                                        ttl: next_ttl,
                                        .. ipv4_fields
                                    },
                                    Ipv4PayloadFields::Udp {
                                        fields: UdpFields::V4 {
                                            source_addr: SocketAddrV4::new(packet.source_ip(), udp.source_port()),
                                            dest_addr: private_dest_addr,
                                        },
                                        payload: udp.payload(),
                                    }
                                );

                                let _ = self.private_plug.tx.unbounded_send(bounced_packet);
                            },
                            Ipv4Payload::Tcp(tcp) => {
                                let tcp_fields = tcp.fields_v4(packet.source_ip(), packet.dest_ip());
                                let dest_port = tcp.dest_port();
                                let private_dest_addr = match self.tcp_map.get_inbound_addr(dest_port) {
                                    Some(addr) => addr,
                                    None => continue,
                                };

                                let bounced_packet = Ipv4Packet::new_from_fields_recursive(
                                    Ipv4Fields {
                                        dest_ip: *private_dest_addr.ip(),
                                        ttl: next_ttl,
                                        .. ipv4_fields
                                    },
                                    Ipv4PayloadFields::Tcp {
                                        fields: TcpFields {
                                            addrs: TcpAddrs::V4 {
                                                source_addr: SocketAddrV4::new(packet.source_ip(), tcp.source_port()),
                                                dest_addr: private_dest_addr,
                                            },
                                            .. tcp_fields
                                        },
                                        payload: tcp.payload(),
                                    }
                                );

                                let _ = self.private_plug.tx.unbounded_send(bounced_packet);
                            },
                            _ => (),
                        }
                        continue;
                    }

                    match packet.payload() {
                        Ipv4Payload::Udp(udp) => {
                            let source_port = udp.source_port();
                            let source_addr = SocketAddrV4::new(source_ip, source_port);
                            let mapped_source_port = self.udp_map.map_port(source_addr);
                            let natted_packet = Ipv4Packet::new_from_fields_recursive(
                                Ipv4Fields {
                                    source_ip: self.public_ip,
                                    ttl: next_ttl,
                                    .. ipv4_fields
                                },
                                Ipv4PayloadFields::Udp {
                                    fields: UdpFields::V4 {
                                        source_addr: SocketAddrV4::new(self.public_ip, mapped_source_port),
                                        dest_addr: SocketAddrV4::new(packet.dest_ip(), udp.dest_port()),
                                    },
                                    payload: udp.payload(),
                                }
                            );

                            info!(
                                "nat {} rewrote packet source address: {:?}",
                                self.public_ip, natted_packet,
                            );

                            let _ = self.public_plug.tx.unbounded_send(natted_packet);
                        },
                        Ipv4Payload::Tcp(tcp) => {
                            let tcp_fields = tcp.fields_v4(packet.source_ip(), packet.dest_ip());
                            let source_port = tcp.source_port();
                            let source_addr = SocketAddrV4::new(source_ip, source_port);
                            let mapped_source_port = self.tcp_map.map_port(source_addr);
                            let natted_packet = Ipv4Packet::new_from_fields_recursive(
                                Ipv4Fields {
                                    source_ip: self.public_ip,
                                    ttl: next_ttl,
                                    .. ipv4_fields
                                },
                                Ipv4PayloadFields::Tcp {
                                    fields: TcpFields {
                                        addrs: TcpAddrs::V4 {
                                            source_addr: SocketAddrV4::new(self.public_ip, mapped_source_port),
                                            dest_addr: SocketAddrV4::new(packet.dest_ip(), tcp.dest_port()),
                                        },
                                        .. tcp_fields
                                    },
                                    payload: tcp.payload(),
                                }
                            );

                            info!(
                                "nat {} rewrote packet source address: {:?}",
                                self.public_ip, natted_packet,
                            );

                            let _ = self.public_plug.tx.unbounded_send(natted_packet);
                        },
                        _ => (),
                    }
                },
            }
        };

        let public_unplugged = loop {
            match self.public_plug.rx.poll().void_unwrap() {
                Async::NotReady => break false,
                Async::Ready(None) => break true,
                Async::Ready(Some(packet)) => {
                    let ipv4_fields = packet.fields();
                    if packet.dest_ip() != self.public_ip {
                        info!(
                            "nat {} dropping inbound packet not directed at our public ip: {:?}",
                            self.public_ip, packet,
                        );
                        continue;
                    }
                    let next_ttl = match ipv4_fields.ttl.checked_sub(1) {
                        Some(ttl) => ttl,
                        None => {
                            info!(
                                "nat {} dropping inbound packet with ttl zero {:?}",
                                self.public_ip, packet,
                            );
                            continue
                        },
                    };
                    match packet.payload() {
                        Ipv4Payload::Udp(udp) => {
                            let dest_port = udp.dest_port();
                            match self.udp_map.get_inbound_addr(dest_port) {
                                Some(private_dest_addr) => {
                                    let natted_packet = Ipv4Packet::new_from_fields_recursive(
                                        Ipv4Fields {
                                            dest_ip: *private_dest_addr.ip(),
                                            ttl: next_ttl,
                                            .. ipv4_fields
                                        },
                                        Ipv4PayloadFields::Udp {
                                            fields: UdpFields::V4 {
                                                source_addr: SocketAddrV4::new(packet.source_ip(), udp.source_port()),
                                                dest_addr: private_dest_addr,
                                            },
                                            payload: udp.payload(),
                                        }
                                    );

                                    info!(
                                        "nat {} rewrote destination of inbound packet: {:?}",
                                        self.public_ip, natted_packet,
                                    );

                                    let _ = self.private_plug.tx.unbounded_send(natted_packet);
                                },
                                None => (),
                            }
                        },
                        Ipv4Payload::Tcp(tcp) => {
                            let tcp_fields = tcp.fields_v4(packet.source_ip(), packet.dest_ip());
                            let dest_port = tcp.dest_port();
                            match self.tcp_map.get_inbound_addr(dest_port) {
                                Some(private_dest_addr) => {
                                    let natted_packet = Ipv4Packet::new_from_fields_recursive(
                                        Ipv4Fields {
                                            dest_ip: *private_dest_addr.ip(),
                                            ttl: next_ttl,
                                            .. ipv4_fields
                                        },
                                        Ipv4PayloadFields::Tcp {
                                            fields: TcpFields {
                                                addrs: TcpAddrs::V4 {
                                                    source_addr: SocketAddrV4::new(packet.source_ip(), tcp.source_port()),
                                                    dest_addr: private_dest_addr,
                                                },
                                                .. tcp_fields
                                            },
                                            payload: tcp.payload(),
                                        }
                                    );

                                    info!(
                                        "nat {} rewrote destination of inbound packet: {:?}",
                                        self.public_ip, natted_packet,
                                    );

                                    let _ = self.private_plug.tx.unbounded_send(natted_packet);
                                },
                                None => (),
                            }
                        },
                        _ => (),
                    }
                },
            }
        };

        if private_unplugged && public_unplugged {
            return Ok(Async::Ready(()));
        }

        Ok(Async::NotReady)
    }
}

#[test]
fn test() {
    use rand;
    use void;

    let mut core = unwrap!(Core::new());
    let handle = core.handle();

    let res = core.run(future::lazy(move || {
        let (public_plug_0, public_plug_1) = Ipv4Plug::new_wire();
        let (private_plug_0, private_plug_1) = Ipv4Plug::new_wire();
        let public_ip = Ipv4Addr::random_global();
        let subnet = SubnetV4::random_local();

        NatV4::spawn(&handle, public_plug_0, private_plug_0, public_ip, subnet);

        let Ipv4Plug { tx: public_tx, rx: public_rx } = public_plug_1;
        let Ipv4Plug { tx: private_tx, rx: private_rx } = private_plug_1;

        let remote_addr = SocketAddrV4::new(
            Ipv4Addr::random_global(),
            rand::random::<u16>() / 2 + 1000,
        );
        let local_addr = SocketAddrV4::new(
            subnet.random_client_addr(),
            rand::random::<u16>() / 2 + 1000,
        );
        let initial_ttl = rand::random::<u8>() / 2 + 16;
        let payload = Bytes::from(&rand::random::<[u8; 8]>()[..]);
        let packet = Ipv4Packet::new_from_fields_recursive(
            Ipv4Fields {
                source_ip: *local_addr.ip(),
                dest_ip: *remote_addr.ip(),
                ttl: initial_ttl,
            },
            Ipv4PayloadFields::Udp {
                fields: UdpFields::V4 {
                    source_addr: local_addr,
                    dest_addr: remote_addr,
                },
                payload: payload.clone(),
            },
        );

        private_tx
        .send(packet)
        .map_err(|_e| panic!("private side hung up!"))
        .and_then(move |_private_tx| {
            public_rx
            .into_future()
            .map_err(|(v, _public_rx)| void::unreachable(v))
            .and_then(move |(packet_opt, _public_rx)| {
                let packet = unwrap!(packet_opt);
                assert_eq!(packet.fields(), Ipv4Fields {
                    source_ip: public_ip,
                    dest_ip: *remote_addr.ip(),
                    ttl: initial_ttl - 1,
                });
                let mapped_port = match packet.payload() {
                    Ipv4Payload::Udp(udp) => {
                        assert_eq!(udp.payload(), payload);
                        assert_eq!(udp.dest_port(), remote_addr.port());
                        udp.source_port()
                    },
                    payload => panic!("unexpected ipv4 payload: {:?}", payload),
                };
                let payload = Bytes::from(&rand::random::<[u8; 8]>()[..]);
                let packet = Ipv4Packet::new_from_fields_recursive(
                    Ipv4Fields {
                        source_ip: *remote_addr.ip(),
                        dest_ip: public_ip,
                        ttl: initial_ttl,
                    },
                    Ipv4PayloadFields::Udp {
                        fields: UdpFields::V4 {
                            source_addr: remote_addr,
                            dest_addr: SocketAddrV4::new(public_ip, mapped_port),
                        },
                        payload: payload.clone(),
                    },
                );

                public_tx
                .send(packet)
                .map_err(|_e| panic!("public side hung up!"))
                .and_then(move |_public_tx| {
                    private_rx
                    .into_future()
                    .map_err(|(v, _private_rx)| void::unreachable(v))
                    .map(move |(packet_opt, _private_rx)| {
                        let packet = unwrap!(packet_opt);
                        assert_eq!(packet.fields(), Ipv4Fields {
                            source_ip: *remote_addr.ip(),
                            dest_ip: *local_addr.ip(),
                            ttl: initial_ttl - 1,
                        });
                        match packet.payload() {
                            Ipv4Payload::Udp(udp) => {
                                assert_eq!(udp.payload(), payload);
                                assert_eq!(udp.source_port(), remote_addr.port());
                                assert_eq!(udp.dest_port(), local_addr.port());
                            },
                            payload => panic!("unexpected ipv4 payload: {:?}", payload),
                        }
                    })
                })
            })
        })
    }));
    res.void_unwrap()
}

