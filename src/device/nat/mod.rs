use {
    crate::priv_prelude::*,
    self::{
        port_map::PortMap,
        restrictions::Restrictions,
    },
};
mod port_map;
mod restrictions;

/// A simple NAT (network address translation) implementation.
///
/// For testing network code across NATs.
pub struct Nat {
    iface_sender: mpsc::UnboundedSender<Pin<Box<dyn IpSinkStream>>>,
}

/// Builder for creating a [`Nat`](crate::device::Nat).
pub struct NatBuilder {
    external_ipv4: Ipv4Addr,
    internal_ipv4_network: Ipv4Network,
    hair_pinning: bool,
    address_restricted: bool,
    port_restricted: bool,
    reply_with_rst_to_unexpected_tcp_packets: bool,
}

impl NatBuilder {
    /// Starts building a [`Nat`](crate::device::Nat). Use to configure the NAT then call
    /// [`build`](crate::device::NatBuilder::build) to create the NAT.
    ///
    /// * `external_ipv4` is the IPv4 address that the NAT uses on its external side.
    /// * `internal_ipv4_network` is the IPv4 network (eg. 192.168.0.0/16) on the internal side of
    /// the NAT. The NAT won't forward any packets on its internal side that don't originate from
    /// this network.
    pub fn new(external_ipv4: Ipv4Addr, internal_ipv4_network: Ipv4Network) -> NatBuilder {
        NatBuilder {
            external_ipv4,
            internal_ipv4_network,
            hair_pinning: false,
            address_restricted: false,
            port_restricted: false,
            reply_with_rst_to_unexpected_tcp_packets: false,
        }
    }

    /// Enables [NAT hair-pinning](https://en.wikipedia.org/wiki/Network_address_translation#NAT_hairpinning).
    pub fn hair_pinning(mut self) -> Self {
        self.hair_pinning = true;
        self
    }

    pub fn reply_with_rst_to_unexpected_tcp_packets(mut self) -> Self {
        self.reply_with_rst_to_unexpected_tcp_packets = true;
        self
    }

    /// Makes this NAT [address restricted](https://en.wikipedia.org/wiki/Network_address_translation#Methods_of_translation).
    pub fn address_restricted(mut self) -> Self {
        self.address_restricted = true;
        self
    }

    /// Makes this NAT [port restricted](https://en.wikipedia.org/wiki/Network_address_translation#Methods_of_translation).
    pub fn port_restricted(mut self) -> Self {
        self.port_restricted = true;
        self
    }

    /// Build the NAT. The returned `IpChannel` is the external interface of the NAT.
    pub fn build(self) -> (Nat, IpChannel) {
        let NatBuilder {
            external_ipv4,
            internal_ipv4_network,
            hair_pinning,
            address_restricted,
            port_restricted,
            reply_with_rst_to_unexpected_tcp_packets,
        } = self;
        let (iface_sender, iface_receiver) = mpsc::unbounded();
        let (channel_0, channel_1) = IpChannel::new(1);
        let tcpv4_restrictions = match (port_restricted, address_restricted) {
            (false, false) => Restrictions::Unrestricted,
            (false, true) => Restrictions::RestrictIpAddr { sent_to: HashMap::new() },
            (true, _) => Restrictions::RestrictSocketAddr { sent_to: HashMap::new() },
        };
        let udpv4_restrictions = match (port_restricted, address_restricted) {
            (false, false) => Restrictions::Unrestricted,
            (false, true) => Restrictions::RestrictIpAddr { sent_to: HashMap::new() },
            (true, _) => Restrictions::RestrictSocketAddr { sent_to: HashMap::new() },
        };
        let task = NatTask {
            iface_receiver,
            external_iface_opt: Some(channel_0),
            internal_ifaces: HashMap::new(),
            next_internal_iface_index: 0,
            external_ipv4,
            internal_ipv4_network,
            internal_addr_indexes: HashMap::new(),
            tcpv4_port_map: PortMap::new(),
            udpv4_port_map: PortMap::new(),
            hair_pinning,
            tcpv4_restrictions,
            udpv4_restrictions,
            reply_with_rst_to_unexpected_tcp_packets,
        };
        tokio::spawn(task);
        let nat = Nat { iface_sender };
        (nat, channel_1)
    }
}

struct NatTask {
    iface_receiver: mpsc::UnboundedReceiver<Pin<Box<dyn IpSinkStream>>>,
    external_iface_opt: Option<IpChannel>,
    internal_ifaces: HashMap<usize, Pin<Box<dyn IpSinkStream>>>,
    next_internal_iface_index: usize,
    external_ipv4: Ipv4Addr,
    internal_ipv4_network: Ipv4Network,
    internal_addr_indexes: HashMap<IpAddr, usize>,
    tcpv4_port_map: PortMap,
    udpv4_port_map: PortMap,
    hair_pinning: bool,
    tcpv4_restrictions: Restrictions,
    udpv4_restrictions: Restrictions,
    reply_with_rst_to_unexpected_tcp_packets: bool,
}

impl Nat {
    /// Insert an interface into the internal side of this NAT. Packets sent by this interface to
    /// addresses outside the NAT's internal network will be address translated and sent out
    /// the NAT's external interface. This creates a port-mapping which allows external hosts to
    /// send packets back through the NAT to this interface.
    pub fn insert_iface<S>(&mut self, iface: S)
    where
        S: IpSinkStream,
    {
        let iface = Box::pin(iface);
        self.iface_sender.unbounded_send(iface).unwrap();
    }
}

impl NatTask {
    fn poll_flush_outgoing(&mut self, cx: &mut task::Context) -> Poll<()> {
        let mut any_pending = false;

        match &mut self.external_iface_opt {
            None => (),
            Some(external_iface) => {
                match Pin::new(external_iface).poll_flush(cx) {
                    Poll::Ready(Ok(())) => (),
                    Poll::Ready(Err(_)) => {
                        self.external_iface_opt = None;
                    },
                    Poll::Pending => {
                        any_pending = true;
                    },
                }
            },
        }

        let mut defunct_indexes = Vec::new();
        for (index, internal_iface) in &mut self.internal_ifaces {
            match Pin::new(internal_iface).poll_flush(cx) {
                Poll::Ready(Ok(())) => (),
                Poll::Ready(Err(_)) => {
                    defunct_indexes.push(*index);
                },
                Poll::Pending => {
                    any_pending = true;
                },
            }
        }
        for index in defunct_indexes {
            self.internal_ifaces.remove(&index).unwrap();
        }
        if any_pending {
            Poll::Pending
        } else {
            Poll::Ready(())
        }
    }

    fn poll_ready_outgoing(&mut self, cx: &mut task::Context) -> Poll<()> {
        match self.poll_flush_outgoing(cx) {
            Poll::Ready(()) => return Poll::Ready(()),
            Poll::Pending => (),
        }

        let mut any_pending = false;

        match &mut self.external_iface_opt {
            None => (),
            Some(external_iface) => {
                match Pin::new(external_iface).poll_ready(cx) {
                    Poll::Ready(Ok(())) => (),
                    Poll::Ready(Err(_)) => {
                        self.external_iface_opt = None;
                    },
                    Poll::Pending => {
                        any_pending = true;
                    },
                }
            },
        }

        let mut defunct_indexes = Vec::new();
        for (index, internal_iface) in &mut self.internal_ifaces {
            match Pin::new(internal_iface).poll_ready(cx) {
                Poll::Ready(Ok(())) => (),
                Poll::Ready(Err(_)) => {
                    defunct_indexes.push(*index);
                },
                Poll::Pending => {
                    any_pending = true;
                },
            }
        }
        for index in defunct_indexes {
            self.internal_ifaces.remove(&index).unwrap();
        }
        if any_pending {
            Poll::Pending
        } else {
            Poll::Ready(())
        }
    }

    fn poll_next_incoming_external(&mut self, cx: &mut task::Context) -> Poll<Box<IpPacket>> {
        match &mut self.external_iface_opt {
            None => Poll::Pending,
            Some(external_iface) => {
                match Pin::new(external_iface).poll_next(cx) {
                    Poll::Ready(Some(Ok(packet))) => Poll::Ready(packet),
                    Poll::Ready(Some(Err(_))) | Poll::Ready(None) => {
                        self.external_iface_opt = None;
                        Poll::Pending
                    },
                    Poll::Pending => Poll::Pending,
                }
            },
        }
    }

    fn poll_next_incoming_internal(&mut self, cx: &mut task::Context) -> Poll<(usize, Box<IpPacket>)> {
        let mut defunct_indexes = Vec::new();
        let mut index_packet_opt = None;
        for (index, internal_iface) in &mut self.internal_ifaces {
            match Pin::new(internal_iface).poll_next(cx) {
                Poll::Ready(Some(Ok(packet))) => {
                    index_packet_opt = Some((*index, packet));
                    break;
                },
                Poll::Ready(Some(Err(_))) | Poll::Ready(None) => {
                    defunct_indexes.push(*index);
                },
                Poll::Pending => (),
            }
        }
        for index in defunct_indexes {
            self.internal_ifaces.remove(&index).unwrap();
        }
        match index_packet_opt {
            Some((index, packet)) => Poll::Ready((index, packet)),
            None => Poll::Pending,
        }
    }

    fn dispatch_incoming_external(&mut self, packet: Box<IpPacket>) {
        if log_enabled!(Level::Debug) {
            debug!("{}: received from external iface: {:?}", self.external_ipv4, packet);
        }

        match packet.version_box() {
            IpPacketVersion::V6(_) => (),
            IpPacketVersion::V4(packet) => {
                if packet.destination_addr() != self.external_ipv4 {
                    debug!(
                        "{}: dropping external packet addressed to different ip {}",
                        self.external_ipv4, packet.destination_addr(),
                    );
                    return;
                }
                match packet.protocol_box() {
                    Ipv4PacketProtocol::Tcp(mut packet) => {
                        let port = packet.destination_port();
                        let mapped_addr_opt = if self.tcpv4_restrictions.incoming_allowed(port, packet.source_addr()) {
                            self.tcpv4_port_map.incoming_addr(port)
                        } else {
                            None
                        };
                        let mapped_addr = match mapped_addr_opt {
                            Some(mapped_addr) => mapped_addr,
                            None => {
                                if self.reply_with_rst_to_unexpected_tcp_packets {
                                    let mut rst_packet = Tcpv4Packet::new();
                                    rst_packet.set_flags(TcpPacketFlags {
                                        rst: true,
                                        ack: true,
                                        .. TcpPacketFlags::default()
                                    });
                                    rst_packet.set_source_addr(packet.destination_addr());
                                    rst_packet.set_destination_addr(packet.source_addr());
                                    rst_packet.set_ack_number(packet.seq_number().wrapping_add(1));
                                    match &mut self.external_iface_opt {
                                        None => (),
                                        Some(external_iface) => {
                                            match Pin::new(external_iface).start_send(rst_packet.ip_packet_box()) {
                                                Ok(()) => (),
                                                Err(_) => {
                                                    self.external_iface_opt = None;
                                                },
                                            }
                                        },
                                    }
                                }
                                debug!(
                                    "{}: dropping external packet addressed to unmapped or disallowed port {}",
                                    self.external_ipv4, packet.destination_addr(),
                                );
                                return;
                            },
                        };
                        let iface_index = match self.internal_addr_indexes.get(&IpAddr::V4(*mapped_addr.ip())) {
                            Some(iface_index) => iface_index,
                            None => return,
                        };
                        let internal_iface = match self.internal_ifaces.get_mut(iface_index) {
                            Some(internal_iface) => internal_iface,
                            None => return,
                        };
                        packet.set_destination_addr(mapped_addr);
                        if log_enabled!(Level::Debug) {
                            debug!(
                                "{}: forwarding translated packet on internal iface #{} {:?}",
                                self.external_ipv4,
                                iface_index,
                                packet,
                            );
                        }
                        match Pin::new(internal_iface).start_send(packet.ip_packet_box()) {
                            Ok(()) => (),
                            Err(_) => {
                                self.internal_ifaces.remove(iface_index);
                            },
                        }
                    },
                    Ipv4PacketProtocol::Udp(mut packet) => {
                        let port = packet.destination_port();
                        let mapped_addr_opt = if self.udpv4_restrictions.incoming_allowed(port, packet.source_addr()) {
                            self.udpv4_port_map.incoming_addr(port)
                        } else {
                            None
                        };
                        let mapped_addr = match mapped_addr_opt {
                            Some(mapped_addr) => mapped_addr,
                            None => {
                                debug!(
                                    "{}: dropping external packet addressed to unmapped or disallowed port {}",
                                    self.external_ipv4, packet.destination_addr(),
                                );
                                return;
                            },
                        };
                        let iface_index = match self.internal_addr_indexes.get(&IpAddr::V4(*mapped_addr.ip())) {
                            Some(iface_index) => iface_index,
                            None => return,
                        };
                        let internal_iface = match self.internal_ifaces.get_mut(iface_index) {
                            Some(internal_iface) => internal_iface,
                            None => return,
                        };
                        packet.set_destination_addr(mapped_addr);
                        if log_enabled!(Level::Debug) {
                            debug!(
                                "{}: forwarding translated packet on internal iface #{} {:?}",
                                self.external_ipv4,
                                iface_index,
                                packet,
                            );
                        }
                        match Pin::new(internal_iface).start_send(packet.ip_packet_box()) {
                            Ok(()) => (),
                            Err(_) => {
                                self.internal_ifaces.remove(iface_index);
                            },
                        }
                    },
                    Ipv4PacketProtocol::Icmp(_) => (),
                    Ipv4PacketProtocol::Unknown { .. } => (),
                }
            },
        }
    }

    fn dispatch_incoming_internal(&mut self, iface_index: usize, packet: Box<IpPacket>) {
        if log_enabled!(Level::Debug) {
            debug!(
                "{}: received on internal iface #{}: {:?}",
                self.external_ipv4,
                iface_index,
                packet,
            );
        }

        match packet.version_box() {
            IpPacketVersion::V6(packet) => {
                self.internal_addr_indexes.insert(IpAddr::V6(packet.source_addr()), iface_index);
            },
            IpPacketVersion::V4(packet) => {
                if !self.internal_ipv4_network.contains(packet.source_addr()) {
                    debug!(
                        "{}: dropping internal packet from wrong network {}, {}",
                        self.external_ipv4, packet.source_addr(), self.internal_ipv4_network,
                    );
                    return;
                }
                self.internal_addr_indexes.insert(IpAddr::V4(packet.source_addr()), iface_index);
                let destination_ip = packet.destination_addr();
                if self.internal_ipv4_network.contains(destination_ip) {
                    let iface_index = match self.internal_addr_indexes.get(&IpAddr::V4(destination_ip)) {
                        Some(iface_index) => iface_index,
                        None => {
                            debug!(
                                "{}: dropping internal packet addressed to unknown internal device {}",
                                self.external_ipv4, packet.destination_addr(),
                            );
                            return;
                        },
                    };
                    let internal_iface = match self.internal_ifaces.get_mut(iface_index) {
                        Some(internal_iface) => internal_iface,
                        None => return,
                    };
                    match Pin::new(internal_iface).start_send(packet.ip_packet_box()) {
                        Ok(()) => (),
                        Err(_) => {
                            self.internal_ifaces.remove(iface_index);
                        },
                    }
                } else {
                    match packet.protocol_box() {
                        Ipv4PacketProtocol::Tcp(mut packet) => {
                            let internal_addr = packet.source_addr();
                            let port = self.tcpv4_port_map.outgoing_port(internal_addr);
                            self.tcpv4_restrictions.sending(port, packet.destination_addr());
                            packet.set_source_addr(SocketAddrV4::new(self.external_ipv4, port));
                            if log_enabled!(Level::Debug) {
                                debug!(
                                    "{}: translated outgoing packet {:?}",
                                    self.external_ipv4,
                                    packet,
                                );
                            }
                            if *packet.destination_addr().ip() == self.external_ipv4 {
                                if self.hair_pinning {
                                    self.dispatch_incoming_external(packet.ip_packet_box());
                                } else {
                                    debug!(
                                        "{}: dropped internal packet from {} addressed to own external address {} since hair-pinning is disabled",
                                        self.external_ipv4, packet.source_addr(), packet.destination_addr(),
                                    );
                                }
                            } else {
                                match &mut self.external_iface_opt {
                                    None => (),
                                    Some(external_iface) => {
                                        match Pin::new(external_iface).start_send(packet.ip_packet_box()) {
                                            Ok(()) => (),
                                            Err(_) => {
                                                self.external_iface_opt = None;
                                            },
                                        }
                                    },
                                }
                            }
                        },
                        Ipv4PacketProtocol::Udp(mut packet) => {
                            let internal_addr = packet.source_addr();
                            let port = self.udpv4_port_map.outgoing_port(internal_addr);
                            self.udpv4_restrictions.sending(port, packet.destination_addr());
                            packet.set_source_addr(SocketAddrV4::new(self.external_ipv4, port));
                            if log_enabled!(Level::Debug) {
                                debug!(
                                    "{}: translated outgoing packet {:?}",
                                    self.external_ipv4,
                                    packet,
                                );
                            }
                            if *packet.destination_addr().ip() == self.external_ipv4 {
                                if self.hair_pinning {
                                    self.dispatch_incoming_external(packet.ip_packet_box());
                                } else {
                                    debug!(
                                        "{}: dropped internal packet from {} addressed to own external address {} since hair-pinning is disabled",
                                        self.external_ipv4, packet.source_addr(), packet.destination_addr(),
                                    );
                                }
                            } else {
                                match &mut self.external_iface_opt {
                                    None => (),
                                    Some(external_iface) => {
                                        match Pin::new(external_iface).start_send(packet.ip_packet_box()) {
                                            Ok(()) => (),
                                            Err(_) => {
                                                self.external_iface_opt = None;
                                            },
                                        }
                                    },
                                }
                            }
                        },
                        Ipv4PacketProtocol::Icmp(_) => (),
                        Ipv4PacketProtocol::Unknown { .. } => (),
                    }
                }
            },
        }
    }

    fn poll_inner(&mut self, cx: &mut task::Context) -> Poll<()> {
        loop {
            match Pin::new(&mut self.iface_receiver).poll_next(cx) {
                Poll::Ready(Some(iface)) => {
                    self.internal_ifaces.insert(self.next_internal_iface_index, iface);
                    self.next_internal_iface_index += 1;
                },
                Poll::Ready(None) => return Poll::Ready(()),
                Poll::Pending => break,
            }
        }

        loop {
            match self.poll_ready_outgoing(cx) {
                Poll::Ready(()) => (),
                Poll::Pending => return Poll::Pending,
            }

            match self.poll_next_incoming_external(cx) {
                Poll::Ready(packet) => {
                    self.dispatch_incoming_external(packet);
                    continue;
                },
                Poll::Pending => (),
            }

            match self.poll_next_incoming_internal(cx) {
                Poll::Ready((index, packet)) => {
                    self.dispatch_incoming_internal(index, packet);
                    continue;
                },
                Poll::Pending => (),
            }

            break Poll::Pending;
        }
    }
}

impl Future for NatTask {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut task::Context) -> Poll<()> {
        let this = self.get_mut();
        this.poll_inner(cx)
    }
}

