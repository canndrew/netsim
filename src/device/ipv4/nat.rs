use priv_prelude::*;
use rand;

#[derive(Debug)]
/// An Ipv4 NAT.
pub struct Ipv4Nat {
    private_plug: Ipv4Plug,
    public_plug: Ipv4Plug,
    public_ip: Ipv4Addr,
    subnet: Ipv4Range, 
    hair_pinning: bool,
    udp_map: PortMap,
    tcp_map: PortMap,
    blacklist_unrecognized_addrs: bool,
    blacklisted_addrs: HashSet<SocketAddrV4>,
}

#[derive(Debug)]
enum PortAllocator {
    Sequential {
        next_original_port: u16,
        next_for_local_endpoint: HashMap<SocketAddrV4, u16>,
    },
    Random,
}

impl PortAllocator {
    pub fn next_port(&mut self, local_endpoint: SocketAddrV4) -> u16 {
        match *self {
            PortAllocator::Sequential {
                ref mut next_original_port,
                ref mut next_for_local_endpoint,
            } => {
                match next_for_local_endpoint.entry(local_endpoint) {
                    hash_map::Entry::Occupied(mut oe) => {
                        let port = *oe.get();
                        *oe.get_mut() = oe.get().checked_add(1).unwrap_or(49152);
                        port
                    },
                    hash_map::Entry::Vacant(mut ve) => {
                        let port = *next_original_port;
                        *next_original_port = next_original_port.wrapping_add(16);
                        if *next_original_port < 49152 { *next_original_port += 49153 };
                        ve.insert(port);
                        port
                    },
                }
            },
            PortAllocator::Random => {
                loop {
                    let port = rand::random();
                    if port >= 1000 {
                        break port;
                    }
                }
            }
        }
    }
}

impl Default for PortAllocator {
    fn default() -> PortAllocator {
        PortAllocator::Sequential {
            next_original_port: 49152,
            next_for_local_endpoint: HashMap::new(),
        }
    }
}

#[derive(Debug, Default)]
struct PortMap {
    map_out: HashMap<SocketAddrV4, u16>,
    map_in: HashMap<u16, SocketAddrV4>,
    allowed_endpoints: Option<HashMap<u16, SocketAddrV4>>,
    symmetric_map: Option<SymmetricMap>,
    port_allocator: PortAllocator,
}

#[derive(Debug, Default)]
struct SymmetricMap {
    map_out: HashMap<(SocketAddrV4, SocketAddrV4), u16>,
    map_in: HashMap<u16, (SocketAddrV4, SocketAddrV4)>,
}

impl PortMap {
    pub fn new() -> PortMap {
        PortMap::default()
    }

    pub fn forward_port(&mut self, port: u16, local_addr: SocketAddrV4) {
        self.map_out.insert(local_addr, port);
        self.map_in.insert(port, local_addr);
    }

    pub fn get_inbound_addr(&self, remote_addr: SocketAddrV4, port: u16) -> Option<SocketAddrV4> {
        if let Some(ref allowed_endpoints) = self.allowed_endpoints {
            if !allowed_endpoints.get(&port).map(|allowed| *allowed == remote_addr).unwrap_or(false) {
                trace!("NAT dropping packet from restricted address {}. allowed endpoints: {:?}", remote_addr, allowed_endpoints);
                return None;
            }
        }
        if let Some(addr) = self.map_in.get(&port) {
            return Some(*addr);
        }
        if let Some(ref symmetric_map) = self.symmetric_map {
            if let Some(&(addr, allowed_remote_addr)) = symmetric_map.map_in.get(&port) {
                if allowed_remote_addr == remote_addr {
                    return Some(addr);
                }
            }
        }
        None
    }

    pub fn map_port(&mut self, remote_addr: SocketAddrV4, source_addr: SocketAddrV4) -> u16 {
        let port = match self.map_out.entry(source_addr) {
            hash_map::Entry::Occupied(oe) => *oe.get(),
            hash_map::Entry::Vacant(ve) => {
                if let Some(ref mut symmetric_map) = self.symmetric_map {
                    match symmetric_map.map_out.entry((source_addr, remote_addr)) {
                        hash_map::Entry::Occupied(oe) => *oe.get(),
                        hash_map::Entry::Vacant(ve) => {
                            let port = loop {
                                let port = self.port_allocator.next_port(source_addr);
                                if self.map_in.contains_key(&port) {
                                    continue;
                                }
                                if symmetric_map.map_in.contains_key(&port) {
                                    continue;
                                }
                                break port;
                            };

                            ve.insert(port);
                            symmetric_map.map_in.insert(port, (source_addr, remote_addr));
                            port
                        },
                    }
                } else {
                    let port = loop {
                        let port = self.port_allocator.next_port(source_addr);
                        if self.map_in.contains_key(&port) {
                            continue;
                        }
                        break port;
                    };

                    ve.insert(port);
                    self.map_in.insert(port, source_addr);
                    port
                }
            },
        };
        if let Some(ref mut allowed_endpoints) = self.allowed_endpoints {
            allowed_endpoints.insert(port, remote_addr);
        }
        port
    }
}

impl Ipv4Nat {
    /// Create a new Ipv4 NAT
    pub fn new(
        public_plug: Ipv4Plug,
        private_plug: Ipv4Plug,
        public_ip: Ipv4Addr,
        subnet: Ipv4Range,
    ) -> Ipv4Nat {
        let ret = Ipv4Nat {
            private_plug: private_plug,
            public_plug: public_plug,
            public_ip: public_ip,
            subnet: subnet,
            hair_pinning: false,
            udp_map: PortMap::new(),
            tcp_map: PortMap::new(),
            blacklist_unrecognized_addrs: false,
            blacklisted_addrs: HashSet::new(),
        };
        debug!("building {:?}", ret);
        ret
    }

    /// Create a new Ipv4 NAT, spawning it directly onto the tokio event loop.
    pub fn spawn(
        handle: &NetworkHandle,
        public_plug: Ipv4Plug,
        private_plug: Ipv4Plug,
        public_ip: Ipv4Addr,
        subnet: Ipv4Range,
    ) {
        let nat_v4 = Ipv4Nat::new(public_plug, private_plug, public_ip, subnet);
        handle.spawn(nat_v4.infallible());
    }
}

#[derive(Default)]
/// A builder for `Ipv4Nat`
pub struct Ipv4NatBuilder {
    subnet: Option<Ipv4Range>,
    hair_pinning: bool,
    udp_map: PortMap,
    tcp_map: PortMap,
    blacklist_unrecognized_addrs: bool,
}

impl Ipv4NatBuilder {
    /// Start building an Ipv4 NAT
    pub fn new() -> Ipv4NatBuilder {
        Ipv4NatBuilder::default()
    }

    /// Set the subnet used on the local side of the NAT. If left unset, a random subnet will be
    /// chosen.
    pub fn subnet(mut self, subnet: Ipv4Range) -> Ipv4NatBuilder {
        self.subnet = Some(subnet);
        self
    }

    /// Get the subnet set by the last call to `subnet` (if any).
    pub fn get_subnet(&self) -> Option<Ipv4Range> {
        self.subnet
    }

    /// Enable/disable hair-pinning.
    pub fn hair_pinning(mut self, hair_pinning: bool) -> Ipv4NatBuilder {
        self.hair_pinning = hair_pinning;
        self
    }

    /// Manually forward a UDP port.
    pub fn forward_udp_port(mut self, port: u16, local_addr: SocketAddrV4) -> Ipv4NatBuilder {
        self.udp_map.forward_port(port, local_addr);
        self
    }

    /// Manually forward a TCP port.
    pub fn forward_tcp_port(mut self, port: u16, local_addr: SocketAddrV4) -> Ipv4NatBuilder {
        self.tcp_map.forward_port(port, local_addr);
        self
    }

    /// Causes the NAT to permanently block all traffic from an address A if it receives traffic
    /// from A directed at an endpoint for which is doesn't have a mapping.
    pub fn blacklist_unrecognized_addrs(mut self) -> Ipv4NatBuilder {
        self.blacklist_unrecognized_addrs = true;
        self
    }

    /// Only allow incoming traffic on a port from remote addresses that we have already sent
    /// data to from that port. Makes this a port-restricted NAT.
    pub fn restrict_endpoints(mut self) -> Ipv4NatBuilder {
        self.tcp_map.allowed_endpoints = Some(HashMap::new());
        self.udp_map.allowed_endpoints = Some(HashMap::new());
        self
    }

    /// Use random, rather than sequential (the default) port allocation.
    pub fn randomize_port_allocation(mut self) -> Ipv4NatBuilder {
        self.tcp_map.port_allocator = PortAllocator::Random;
        self.udp_map.port_allocator = PortAllocator::Random;
        self
    }

    /// Makes this NAT a symmetric NAT, meaning packets sent to different remote addresses from the
    /// same internal address will appear to originate from different external ports.
    pub fn symmetric(mut self) -> Ipv4NatBuilder {
        self.tcp_map.symmetric_map = Some(SymmetricMap::default());
        self.udp_map.symmetric_map = Some(SymmetricMap::default());
        self
    }

    /// Build the NAT
    pub fn build(
        self, 
        public_plug: Ipv4Plug,
        private_plug: Ipv4Plug,
        public_ip: Ipv4Addr,
    ) -> Ipv4Nat {
        let subnet = self.subnet.unwrap_or_else(Ipv4Range::random_local_subnet);
        let ret = Ipv4Nat {
            private_plug: private_plug,
            public_plug: public_plug,
            public_ip: public_ip,
            subnet: subnet, 
            hair_pinning: self.hair_pinning,
            udp_map: self.udp_map,
            tcp_map: self.tcp_map,
            blacklist_unrecognized_addrs: false,
            blacklisted_addrs: HashSet::new(),
        };
        debug!("building {:?}", ret);
        ret
    }

    /// Build the NAT, spawning it directly onto the tokio event loop.
    pub fn spawn(
        self,
        handle: &NetworkHandle,
        public_plug: Ipv4Plug,
        private_plug: Ipv4Plug,
        public_ip: Ipv4Addr,
    ) {
        let nat_v4 = self.build(public_plug, private_plug, public_ip);
        handle.spawn(nat_v4.infallible());
    }
}

impl Ipv4Nat {
    fn process_outgoing(&mut self) -> bool {
        loop {
            match self.private_plug.poll_incoming() {
                Async::NotReady => return false,
                Async::Ready(None) => return true,
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
                                let udp_fields = udp.fields();
                                let dest_port = udp.dest_port();
                                let dest_addr = SocketAddrV4::new(self.public_ip, dest_port);
                                let source_port = udp.source_port();
                                let source_addr = SocketAddrV4::new(source_ip, source_port);
                                let external_source_port = self.udp_map.map_port(dest_addr, source_addr);
                                let external_source_addr = SocketAddrV4::new(
                                    self.public_ip,
                                    external_source_port,
                                );

                                let private_dest_addr = match self.udp_map.get_inbound_addr(external_source_addr, dest_port) {
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
                                        fields: UdpFields {
                                            dest_port: private_dest_addr.port(),
                                            .. udp_fields
                                        },
                                        payload: udp.payload(),
                                    }
                                );

                                let _ = self.private_plug.unbounded_send(bounced_packet);
                            },
                            Ipv4Payload::Tcp(tcp) => {
                                let tcp_fields = tcp.fields();
                                let dest_port = tcp.dest_port();
                                let dest_addr = SocketAddrV4::new(self.public_ip, dest_port);
                                let source_port = tcp.source_port();
                                let source_addr = SocketAddrV4::new(source_ip, source_port);
                                let external_source_port = self.tcp_map.map_port(dest_addr, source_addr);
                                let external_source_addr = SocketAddrV4::new(
                                    self.public_ip,
                                    external_source_port,
                                );

                                let private_dest_addr = match self.tcp_map.get_inbound_addr(external_source_addr, dest_port) {
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
                                            dest_port: private_dest_addr.port(),
                                            .. tcp_fields
                                        },
                                        payload: tcp.payload(),
                                    }
                                );

                                let _ = self.private_plug.unbounded_send(bounced_packet);
                            },
                            _ => (),
                        }
                        continue;
                    }

                    match packet.payload() {
                        Ipv4Payload::Udp(udp) => {
                            let udp_fields = udp.fields();
                            let dest_port = udp.dest_port();
                            let dest_addr = SocketAddrV4::new(dest_ip, dest_port);
                            let source_port = udp.source_port();
                            let source_addr = SocketAddrV4::new(source_ip, source_port);
                            let mapped_source_port = self.udp_map.map_port(dest_addr, source_addr);
                            let natted_packet = Ipv4Packet::new_from_fields_recursive(
                                Ipv4Fields {
                                    source_ip: self.public_ip,
                                    ttl: next_ttl,
                                    .. ipv4_fields
                                },
                                Ipv4PayloadFields::Udp {
                                    fields: UdpFields {
                                        source_port: mapped_source_port,
                                        .. udp_fields
                                    },
                                    payload: udp.payload(),
                                }
                            );

                            info!(
                                "nat {} rewrote packet source address: {} => {}: {:?}",
                                source_addr, SocketAddrV4::new(self.public_ip, mapped_source_port),
                                self.public_ip, natted_packet,
                            );

                            let _ = self.public_plug.unbounded_send(natted_packet);
                        },
                        Ipv4Payload::Tcp(tcp) => {
                            let tcp_fields = tcp.fields();
                            let dest_port = tcp.dest_port();
                            let dest_addr = SocketAddrV4::new(dest_ip, dest_port);
                            let source_port = tcp.source_port();
                            let source_addr = SocketAddrV4::new(source_ip, source_port);
                            let mapped_source_port = self.tcp_map.map_port(dest_addr, source_addr);
                            let natted_packet = Ipv4Packet::new_from_fields_recursive(
                                Ipv4Fields {
                                    source_ip: self.public_ip,
                                    ttl: next_ttl,
                                    .. ipv4_fields
                                },
                                Ipv4PayloadFields::Tcp {
                                    fields: TcpFields {
                                        source_port: mapped_source_port,
                                        .. tcp_fields
                                    },
                                    payload: tcp.payload(),
                                }
                            );

                            info!(
                                "nat {} rewrote packet source address: {:?}",
                                self.public_ip, natted_packet,
                            );

                            let _ = self.public_plug.unbounded_send(natted_packet);
                        },
                        _ => (),
                    }
                },
            }
        }
    }

    fn process_incoming(&mut self) -> bool {
        loop {
            match self.public_plug.poll_incoming() {
                Async::NotReady => return false,
                Async::Ready(None) => return true,
                Async::Ready(Some(packet)) => {
                    trace!("nat {} received packet from public side: {:?}", self.public_ip, packet);
                    let ipv4_fields = packet.fields();
                    let source_ip = packet.source_ip();
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
                            let udp_fields = udp.fields();
                            let source_port = udp.source_port();
                            let source_addr = SocketAddrV4::new(source_ip, source_port);
                            if self.blacklisted_addrs.contains(&source_addr) {
                                info!("nat {} dropped packet from blacklisted addr {}", self.public_ip, source_addr);
                                continue;
                            }
                            let dest_port = udp.dest_port();
                            match self.udp_map.get_inbound_addr(source_addr, dest_port) {
                                Some(private_dest_addr) => {
                                    let natted_packet = Ipv4Packet::new_from_fields_recursive(
                                        Ipv4Fields {
                                            dest_ip: *private_dest_addr.ip(),
                                            ttl: next_ttl,
                                            .. ipv4_fields
                                        },
                                        Ipv4PayloadFields::Udp {
                                            fields: UdpFields {
                                                dest_port: private_dest_addr.port(),
                                                .. udp_fields
                                            },
                                            payload: udp.payload(),
                                        }
                                    );

                                    info!(
                                        "nat {} rewrote destination of inbound packet: {:?}",
                                        self.public_ip, natted_packet,
                                    );

                                    let _ = self.private_plug.unbounded_send(natted_packet);
                                },
                                None => {
                                    if self.blacklist_unrecognized_addrs {
                                        trace!("nat {} blacklisting unknown address {}", self.public_ip, source_addr);
                                        self.blacklisted_addrs.insert(source_addr);
                                    }
                                },
                            }
                        },
                        Ipv4Payload::Tcp(tcp) => {
                            let tcp_fields = tcp.fields();
                            let source_port = tcp.source_port();
                            let source_addr = SocketAddrV4::new(source_ip, source_port);
                            if self.blacklisted_addrs.contains(&source_addr) {
                                continue;
                            }
                            let dest_port = tcp.dest_port();
                            match self.tcp_map.get_inbound_addr(source_addr, dest_port) {
                                Some(private_dest_addr) => {
                                    let natted_packet = Ipv4Packet::new_from_fields_recursive(
                                        Ipv4Fields {
                                            dest_ip: *private_dest_addr.ip(),
                                            ttl: next_ttl,
                                            .. ipv4_fields
                                        },
                                        Ipv4PayloadFields::Tcp {
                                            fields: TcpFields {
                                                dest_port: private_dest_addr.port(),
                                                .. tcp_fields
                                            },
                                            payload: tcp.payload(),
                                        }
                                    );

                                    info!(
                                        "nat {} rewrote destination of inbound packet: {:?}",
                                        self.public_ip, natted_packet,
                                    );

                                    let _ = self.private_plug.unbounded_send(natted_packet);
                                },
                                None => {
                                    if self.blacklist_unrecognized_addrs {
                                        self.blacklisted_addrs.insert(source_addr);
                                    }
                                },
                            }
                        },
                        _ => (),
                    }
                },
            }
        }
    }
}

impl Future for Ipv4Nat {
    type Item = ();
    type Error = Void;

    fn poll(&mut self) -> Result<Async<()>, Void> {
        let private_unplugged = self.process_outgoing();
        let public_unplugged = self.process_incoming();

        if private_unplugged && public_unplugged {
            return Ok(Async::Ready(()));
        }

        Ok(Async::NotReady)
    }
}

#[test]
fn test() {
    run_test(1, || {
        use rand;
        use void;

        let mut core = unwrap!(Core::new());
        let network = Network::new(&core.handle());
        let handle = network.handle();

        let res = core.run(future::lazy(move || {
            let (public_plug_0, public_plug_1) = Ipv4Plug::new_pair();
            let (private_plug_0, private_plug_1) = Ipv4Plug::new_pair();
            let public_ip = Ipv4Addr::random_global();
            let subnet = Ipv4Range::random_local_subnet();

            Ipv4Nat::spawn(&handle, public_plug_0, private_plug_0, public_ip, subnet);

            let (public_tx, public_rx) = public_plug_1.split();
            let (private_tx, private_rx) = private_plug_1.split();

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
                    fields: UdpFields {
                        source_port: local_addr.port(),
                        dest_port: remote_addr.port(),
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
                            fields: UdpFields {
                                source_port: remote_addr.port(),
                                dest_port: mapped_port,
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
    })
}

#[test]
fn test_port_restriction() {
    let mut unrestricted = PortMap::default();
    let mut restricted = PortMap::default();
    restricted.allowed_endpoints = Some(HashMap::new());

    let subnet = Ipv4Range::random_local_subnet();
    let internal_addr = SocketAddrV4::new(subnet.random_client_addr(), rand::random());
    let remote_addr = SocketAddrV4::new(Ipv4Addr::random_global(), rand::random());
    let unknown_addr = SocketAddrV4::new(Ipv4Addr::random_global(), rand::random());

    let mapped_port = unrestricted.map_port(remote_addr, internal_addr);
    let inbound_addr = unrestricted.get_inbound_addr(remote_addr, mapped_port);
    assert_eq!(inbound_addr, Some(internal_addr));
    let inbound_addr = unrestricted.get_inbound_addr(unknown_addr, mapped_port);
    assert_eq!(inbound_addr, Some(internal_addr));

    let mapped_port = restricted.map_port(remote_addr, internal_addr);
    let inbound_addr = unrestricted.get_inbound_addr(remote_addr, mapped_port);
    assert_eq!(inbound_addr, Some(internal_addr));
    let inbound_addr = restricted.get_inbound_addr(unknown_addr, mapped_port);
    assert_eq!(inbound_addr, None);
}

#[test]
fn test_symmetric_map() {
    let mut asymmetric = PortMap::default();
    let mut symmetric = PortMap::default();
    symmetric.symmetric_map = Some(SymmetricMap::default());

    let subnet = Ipv4Range::random_local_subnet();
    let internal_addr = SocketAddrV4::new(subnet.random_client_addr(), rand::random());
    let remote_addr_0 = SocketAddrV4::new(Ipv4Addr::random_global(), rand::random());
    let remote_addr_1 = SocketAddrV4::new(Ipv4Addr::random_global(), rand::random());

    let external_port_0 = asymmetric.map_port(remote_addr_0, internal_addr);
    let external_port_1 = asymmetric.map_port(remote_addr_1, internal_addr);
    assert_eq!(external_port_0, external_port_1);
    
    let external_port_0 = symmetric.map_port(remote_addr_0, internal_addr);
    let external_port_1 = symmetric.map_port(remote_addr_1, internal_addr);
    assert!(external_port_0 != external_port_1);
    let inbound_addr_00 = symmetric.get_inbound_addr(remote_addr_0, external_port_0);
    let inbound_addr_01 = symmetric.get_inbound_addr(remote_addr_0, external_port_1);
    let inbound_addr_10 = symmetric.get_inbound_addr(remote_addr_1, external_port_0);
    let inbound_addr_11 = symmetric.get_inbound_addr(remote_addr_1, external_port_1);
    assert_eq!(inbound_addr_00, Some(internal_addr));
    assert_eq!(inbound_addr_01, None);
    assert_eq!(inbound_addr_10, None);
    assert_eq!(inbound_addr_11, Some(internal_addr));
}

