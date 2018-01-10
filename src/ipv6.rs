use priv_prelude::*;

/// An IPv6 packet
#[derive(Clone)]
pub struct Ipv6Packet {
    data: Bytes,
}

impl fmt::Debug for Ipv6Packet {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f
        .debug_struct("Ipv6Packet")
        .field("source", &self.source())
        .field("destination", &self.destination())
        .field("hop_limit", &self.hop_limit())
        .field("payload", &self.payload())
        .finish()
    }
}

impl Ipv6Packet {
    /// Create an IPv6 packet from raw packet data.
    pub fn from_bytes(data: Bytes) -> Ipv6Packet {
        Ipv6Packet {
            data,
        }
    }

    /// Return the raw packet data of the packet.
    pub fn as_bytes(&self) -> &Bytes {
        &self.data
    }

    /// The source IP address.
    pub fn source(&self) -> Ipv6Addr {
        let mut addr = [0u8; 16];
        addr[..].clone_from_slice(&self.data[8..24]);
        Ipv6Addr::from(addr)
    }

    /// The destination IP address.
    pub fn destination(&self) -> Ipv6Addr {
        let mut addr = [0u8; 16];
        addr[..].clone_from_slice(&self.data[24..40]);
        Ipv6Addr::from(addr)
    }

    /// The hop limit of the packet. What IPv6 calls TTL.
    pub fn hop_limit(&self) -> u8 {
        self.data[7]
    }

    /// Get the packet payload
    pub fn payload(&self) -> Ipv6Payload {
        parse_payload(self.data[6], self.data.slice_from(40))
    }
}

fn parse_payload(kind: u8, data: Bytes) -> Ipv6Payload {
    match kind {
        0 => {
            let next_header = data[0];
            let ext_len = data[1];
            let offset = ext_len as usize * 8 + 8;
            let payload = parse_payload(next_header, data.slice_from(offset));
            Ipv6Payload::HopByHop(Box::new(payload))
        },
        58 => {
            let payload = Icmpv6Packet::from_bytes(data);
            Ipv6Payload::Icmp(payload)
        },
        kind => Ipv6Payload::Unknown(kind, data),
    }
}

/// Payload of an IPv6 packet.
#[derive(Debug)]
pub enum Ipv6Payload {
    /// Packet contains a hop-by-hop header, followed by the given payload.
    HopByHop(Box<Ipv6Payload>),
    /// An ICMPv6 packet.
    Icmp(Icmpv6Packet),
    /// An unknown payload with the given protocol number and raw payload data.
    Unknown(u8, Bytes),
}

