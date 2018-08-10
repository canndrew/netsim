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
    rxs: Vec<Ipv4Receiver>,
    txs: Vec<(Ipv4Sender, Vec<Ipv4Route>)>,
    ipv4_addr: Ipv4Addr,
}

impl Ipv4Router {
    /// Create a new router with the given connections. You can also use `Ipv4RouterBuilder` to add
    /// connections individually.
    pub fn new(ipv4_addr: Ipv4Addr, connections: Vec<(Ipv4Plug, Vec<Ipv4Route>)>) -> Ipv4Router {
        let mut rxs = Vec::with_capacity(connections.len());
        let mut txs = Vec::with_capacity(connections.len());
        for (plug, routes) in connections {
            let (tx, rx) = plug.split();
            rxs.push(rx);
            txs.push((tx, routes));
        }

        Ipv4Router { rxs, txs, ipv4_addr }
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
}

impl Future for Ipv4Router {
    type Item = ();
    type Error = Void;

    fn poll(&mut self) -> Result<Async<()>, Void> {
        let mut all_disconnected = true;
        for rx in &mut self.rxs {
            all_disconnected &= {
                'next_packet: loop {
                    match rx.poll().void_unwrap() {
                        Async::NotReady => break false,
                        Async::Ready(None) => break true,
                        Async::Ready(Some(packet)) => {
                            let dest_ip = packet.dest_ip();
                            if dest_ip == self.ipv4_addr {
                                continue;
                            }

                            for &mut (ref mut tx, ref routes) in &mut self.txs {
                                for route in routes {
                                    if route.destination().contains(dest_ip) {
                                        info!("router {} routing packet on route {:?} {:?}", self.ipv4_addr, route, packet);
                                        tx.unbounded_send(packet);
                                        continue 'next_packet;
                                    }
                                }
                            }

                            info!("router {} dropping unroutable packet {:?}", self.ipv4_addr, packet);
                        },
                    }
                }
            };
        }

        if all_disconnected {
            return Ok(Async::Ready(()));
        }

        Ok(Async::NotReady)
    }
}

