use crate::priv_prelude::*;

pub enum Restrictions {
    Unrestricted,
    RestrictIpAddr {
        sent_to: HashMap<u16, HashSet<Ipv4Addr>>,
    },
    RestrictSocketAddr {
        sent_to: HashMap<u16, HashSet<SocketAddrV4>>,
    },
}

impl Restrictions {
    pub fn sending(&mut self, external_port: u16, destination_addr: SocketAddrV4) {
        match self {
            Restrictions::Unrestricted => (),
            Restrictions::RestrictIpAddr { sent_to } => {
                sent_to.entry(external_port).or_default().insert(*destination_addr.ip());
            },
            Restrictions::RestrictSocketAddr { sent_to } => {
                sent_to.entry(external_port).or_default().insert(destination_addr);
            },
        }
    }

    pub fn incoming_allowed(&self, external_port: u16, source_addr: SocketAddrV4) -> bool {
        match self {
            Restrictions::Unrestricted => true,
            Restrictions::RestrictIpAddr { sent_to } => {
                match sent_to.get(&external_port) {
                    None => false,
                    Some(ipv4_addrs) => ipv4_addrs.contains(source_addr.ip()),
                }
            },
            Restrictions::RestrictSocketAddr { sent_to } => {
                match sent_to.get(&external_port) {
                    None => false,
                    Some(socket_addrs) => socket_addrs.contains(&source_addr),
                }
            },
        }
    }
}

