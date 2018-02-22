use priv_prelude::*;

pub struct RouterV4Builder {
    ipv4_addr: Ipv4Addr,
    connections: Vec<(Ipv4Plug, Vec<RouteV4>)>,
}

impl RouterV4Builder {
    pub fn new(ipv4_addr: Ipv4Addr) -> RouterV4Builder {
        RouterV4Builder {
            ipv4_addr: ipv4_addr,
            connections: Vec::new(),
        }
    }

    pub fn connect(mut self, plug: Ipv4Plug, routes: Vec<RouteV4>) -> RouterV4Builder {
        self.connections.push((plug, routes));
        self
    }

    pub fn build(self) -> RouterV4 {
        RouterV4::new(self.ipv4_addr, self.connections)
    }

    pub fn spawn(self, handle: &Handle) {
        RouterV4::spawn(handle, self.ipv4_addr, self.connections)
    }
}

pub struct RouterV4 {
    rxs: Vec<UnboundedReceiver<Ipv4Packet>>,
    txs: Vec<(UnboundedSender<Ipv4Packet>, Vec<RouteV4>)>,
    ipv4_addr: Ipv4Addr,
}

impl RouterV4 {
    pub fn new(ipv4_addr: Ipv4Addr, connections: Vec<(Ipv4Plug, Vec<RouteV4>)>) -> RouterV4 {
        let mut rxs = Vec::with_capacity(connections.len());
        let mut txs = Vec::with_capacity(connections.len());
        for (Ipv4Plug { tx, rx }, routes) in connections {
            rxs.push(rx);
            txs.push((tx, routes));
        }

        RouterV4 { rxs, txs, ipv4_addr }
    }

    pub fn spawn(
        handle: &Handle,
        ipv4_addr: Ipv4Addr,
        connections: Vec<(Ipv4Plug, Vec<RouteV4>)>,
    ) {
        let router_v4 = RouterV4::new(ipv4_addr, connections);
        handle.spawn(router_v4.infallible());
    }
}

impl Future for RouterV4 {
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
                                        let _ = tx.unbounded_send(packet);
                                        info!("router {} routing packet on route {:?}", self.ipv4_addr, route);
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

