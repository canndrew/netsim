use crate::priv_prelude::*;

pub struct PortMap {
    outgoing_map: HashMap<SocketAddrV4, u16>,
    incoming_map: HashMap<u16, SocketAddrV4>,
    next_port: u16,
}

impl PortMap {
    const INITIAL_PORT: u16 = 1025;

    pub fn new() -> PortMap {
        PortMap {
            outgoing_map: HashMap::new(),
            incoming_map: HashMap::new(),
            next_port: PortMap::INITIAL_PORT,
        }
    }

    pub fn outgoing_port(&mut self, internal_addr: SocketAddrV4) -> u16 {
        match self.outgoing_map.entry(internal_addr) {
            hash_map::Entry::Occupied(entry) => *entry.get(),
            hash_map::Entry::Vacant(entry) => {
                let mut attempts = PortMap::INITIAL_PORT;
                let port = loop {
                    let port = self.next_port;
                    self.next_port = {
                        self.next_port.checked_add(1).unwrap_or(PortMap::INITIAL_PORT)
                    };
                    match self.incoming_map.entry(port) {
                        hash_map::Entry::Occupied(mut entry) => {
                            attempts = match attempts.checked_add(1) {
                                Some(attempts) => attempts,
                                None => {
                                    entry.insert(internal_addr);
                                    break port;
                                },
                            };
                        },
                        hash_map::Entry::Vacant(entry) => {
                            entry.insert(internal_addr);
                            break port;
                        },
                    }
                };
                *entry.insert(port)
            },
        }
    }

    pub fn incoming_addr(&self, port: u16) -> Option<SocketAddrV4> {
        self.incoming_map.get(&port).copied()
    }
}

