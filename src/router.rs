use priv_prelude::*;

pub fn new_v4(handle: &Handle, ipv4_addr: Ipv4Addr, connections: Vec<(Ipv4Plug, Vec<RouteV4>)>) {
    let mut rxs = Vec::with_capacity(connections.len());
    let mut txs = Vec::with_capacity(connections.len());
    for (Ipv4Plug { tx, rx }, routes) in connections {
        rxs.push(rx);
        txs.push((tx, routes));
    }

    let router = RouterV4 { rxs, txs, ipv4_addr };
    handle.spawn(router.infallible());
}

struct RouterV4 {
    rxs: Vec<UnboundedReceiver<Ipv4Packet>>,
    txs: Vec<(UnboundedSender<Ipv4Packet>, Vec<RouteV4>)>,
    ipv4_addr: Ipv4Addr,
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
                                        continue 'next_packet;
                                    }
                                }
                            }
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

