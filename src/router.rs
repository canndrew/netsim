/*
pub struct RouterClient {
    client: Box<EtherChannel>,
    mac: MacAddr,
    routes: Vec<RouteV4>,
}
*/

struct Client {
    channel: EtherBox,
    outgoing: VecDeque<Frame>,
}

pub struct Router {
    //clients: Vec<Client>,
    //
    //  maybe use a Slab here?
    //  need to know which index of this Vec arp packets came from so they can be replied to
    //  and so that we can map MacAddr => index,
    //
    mac_addr: MacAddr,
    ip: Ipv4Addr,
    dest_table: HashMap<Ipv4Addr, MacAddr>,
}

impl Router {
    pub fn new() -> Router {
        Router {
            clients: Vec::new(),
        }
    }

    pub fn add(&mut self, client: EtherBox) {
        let client = Client {
            channel: client,
            outgoing: VecDeque::new(),
        };
        clients.push(client);
    }
}

impl Future for Router {
    type Item = ();
    type Error = io::Error;

    fn poll(&mut self) -> io::Result<Async<Void>> {
        let mut incoming = VecDeque::new();
        let mut i = 0;
        while i < self.clients.len() {
            match self.clients[i].poll()? {
                Async::Ready(Some(frame)) => {
                    incoming.push_back(frame);
                    continue;
                },
                Async::Ready(None) => {
                    self.clients.swap_remove(i);
                    continue;
                },
                Async::NotReady => (),
            }
            i += 1;
        }

        while let Some(frame) = incoming.pop_front() {
            if !frame.destination().matches(self.mac_addr) {
                continue;
            }

            match frame.payload() {
                EtherPayload::Arp(arp) => {
                    // TODO: this should be de-duplicated with the code in gateway.
                    self.dest_table.insert(arp.source_ip(), arp.source_mac());
                    match arp.operation() {
                        ArpOperation::Request => {
                            if arp.destination_ip() != self.ip {
                                continue;
                            }
                            let arp = arp.response(self.mac_addr);
                            frame.set_source(self.mac_addr);
                            frame.set_destination(arp.destination_mac());
                            frame.set_payload(EtherPayload::Arp(arp));
                            outgoing.push_back(frame);
                        },
                        ArpOperation::Response => {
                            let pending = mem::replace(&mut self.waiting_on_arp, Vec::new());
                            incoming.extend(pending);
                        },
                    }
                },
                EtherPayload::Ipv4(payload) => {
                    let dest = payload.destination();
                    let dest_mac = match self.dest_table.get(&dest) {
                        Some(dest_mac) => desc_mac,
                        None => continue,
                    };
                    let ttl = match payload.ttl().checked_sub(1) {
                        Some(ttl) => ttl,
                        None => continue,
                    };
                    payload.set_ttl(ttl);
                    frame.set_payload(EtherPayload::Ipv4(payload));
                    frame.set_source(self.mac_addr);
                    frame.set_destination(dest_mac);
                    outgoing.push_back(frame);
                },
                _ => (),
            }
        }

        while let Some(frame) = outgoing.pop_front() {

        }

    }
}


