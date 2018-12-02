use priv_prelude::*;

/// Builder for creating a `Ipv4Router`.
pub struct Ipv4RouterBuilder {
    ipv4_addr: Ipv4Addr,
    connections: Vec<(Ipv4Plug, Vec<Ipv4Route>)>,
}

impl Ipv4RouterBuilder {
    /// Start creating a new `Ipv4Router` with the given IP address.
    pub fn new(ipv4_addr: Ipv4Addr) -> Ipv4RouterBuilder {
        Ipv4RouterBuilder {
            ipv4_addr,
            connections: Vec::new(),
        }
    }

    /// Connect another client to the router. `routes` indicates what packets from other clients
    /// should be routed down this connection. When determining where to route a packet, the
    /// `Ipv4Router` will examine each connection and set of routes in the order they were added
    /// using this function.
    pub fn connect(mut self, plug: Ipv4Plug, routes: Vec<Ipv4Route>) -> Ipv4RouterBuilder {
        self.connections.push((plug, routes));
        self
    }

    /// Build the `Ipv4Router`
    pub fn build(self) -> Ipv4Router {
        Ipv4Router::new(self.ipv4_addr, self.connections)
    }

    /// Build the `Ipv4Router`, spawning it directly onto the tokio event loop.
    pub fn spawn(self, handle: &NetworkHandle) {
        Ipv4Router::spawn(handle, self.ipv4_addr, self.connections)
    }
}

/// Connects a bunch of Ipv4 networks and routes packets between them.
pub struct Ipv4Router {
    // the only reason it's optional, is to make the borrow checker happy
    rxs: Option<Vec<Ipv4Receiver>>,
    txs: Vec<(Ipv4Sender, Vec<Ipv4Route>)>,
    ipv4_addr: Ipv4Addr,
}

impl Ipv4Router {
    /// Create a new router with the given IP address and connections. You can also use
    /// `Ipv4RouterBuilder` to add connections individually.
    pub fn new(ipv4_addr: Ipv4Addr, connections: Vec<(Ipv4Plug, Vec<Ipv4Route>)>) -> Ipv4Router {
        let (rxs, txs) = split_conn_plugs(connections);
        Ipv4Router {
            rxs: Some(rxs),
            txs,
            ipv4_addr,
        }
    }

    /// Create a new `Ipv4Router`, spawning it directly onto the tokio event loop.
    pub fn spawn(
        handle: &NetworkHandle,
        ipv4_addr: Ipv4Addr,
        connections: Vec<(Ipv4Plug, Vec<Ipv4Route>)>,
    ) {
        let router_v4 = Ipv4Router::new(ipv4_addr, connections);
        handle.spawn(router_v4.infallible());
    }

    /// Checks if given packet, is destined to the router itself.
    fn is_packet_to_me(&self, packet: &Ipv4Packet) -> bool {
        packet.dest_ip() == self.ipv4_addr
    }

    /// Find a plug for given packet by it's destination address and send the packet.
    /// Returns true if packet was sent, false otherwise.
    fn send_packet(&mut self, packet: Ipv4Packet) -> bool {
        for &mut (ref mut tx, ref routes) in &mut self.txs {
            for route in routes {
                if route.destination().contains(packet.dest_ip()) {
                    info!(
                        "router {} routing packet on route {:?} {:?}",
                        self.ipv4_addr, route, packet,
                    );
                    tx.unbounded_send(packet);
                    return true;
                }
            }
        }
        info!(
            "router {} dropping unroutable packet {:?}",
            self.ipv4_addr, packet
        );
        false
    }
}

fn split_conn_plugs(
    connections: Vec<(Ipv4Plug, Vec<Ipv4Route>)>,
) -> (Vec<Ipv4Receiver>, Vec<(Ipv4Sender, Vec<Ipv4Route>)>) {
    let mut rxs = Vec::with_capacity(connections.len());
    let mut txs = Vec::with_capacity(connections.len());
    for (plug, routes) in connections {
        let (tx, rx) = plug.split();
        rxs.push(rx);
        txs.push((tx, routes));
    }
    (rxs, txs)
}

impl Future for Ipv4Router {
    type Item = ();
    type Error = Void;

    fn poll(&mut self) -> Result<Async<()>, Void> {
        let mut all_disconnected = true;
        let mut rxs = unwrap!(self.rxs.take());
        for rx in &mut rxs {
            all_disconnected &= loop {
                match rx.poll().void_unwrap() {
                    Async::NotReady => break false,
                    Async::Ready(None) => break true,
                    Async::Ready(Some(packet)) => {
                        if !self.is_packet_to_me(&packet) {
                            let _ = self.send_packet(packet);
                        }
                    }
                }
            };
        }
        self.rxs = Some(rxs);

        if all_disconnected {
            return Ok(Async::Ready(()));
        }

        Ok(Async::NotReady)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod ipv4_router {
        use super::*;

        mod send_packet {
            use super::*;

            fn udp_packet_v4(src: SocketAddrV4, dst: SocketAddrV4) -> Ipv4Packet {
                Ipv4Packet::new_from_fields(
                    Ipv4Fields {
                        source_ip: src.ip().clone(),
                        dest_ip: dst.ip().clone(),
                        ttl: 10,
                    },
                    &Ipv4Payload::Udp(UdpPacket::new_from_fields_v4(
                        &UdpFields {
                            source_port: src.port(),
                            dest_port: dst.port(),
                        },
                        src.ip().clone(),
                        dst.ip().clone(),
                        &Bytes::new(),
                    )),
                )
            }

            #[test]
            fn it_returns_false_when_packet_sender_is_not_found_for_packet_destination_ip() {
                let mut router = Ipv4Router::new(ipv4!("192.168.1.1"), vec![]);
                let packet =
                    udp_packet_v4(addrv4!("192.168.1.100:5000"), addrv4!("192.168.1.200:6000"));

                let sent = router.send_packet(packet);

                assert!(!sent);
            }

            #[test]
            fn it_sends_packet_to_the_channel_associated_with_packet_destination_address() {
                let (plug1_a, mut plug1_b) = Ipv4Plug::new_pair();
                let conns = vec![(
                    plug1_a,
                    vec![Ipv4Route::new(
                        Ipv4Range::new(ipv4!("192.168.1.0"), 24),
                        None,
                    )],
                )];
                let mut router = Ipv4Router::new(ipv4!("192.168.1.1"), conns);
                let packet =
                    udp_packet_v4(addrv4!("192.168.1.100:5000"), addrv4!("192.168.1.200:6000"));

                let sent = router.send_packet(packet.clone());

                assert!(sent);
                let received_packet = plug1_b.poll();
                assert_eq!(received_packet, Ok(Async::Ready(Some(packet))));
            }
        }
    }
}
