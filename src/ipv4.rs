use priv_prelude::*;
use rand;

/// An IPv4 packet.
#[derive(Clone)]
pub struct Ipv4Packet {
    data: Bytes,
}

/// The payload of an IPv4 packet.
#[derive(Debug, Clone)]
pub enum Ipv4Payload {
    /// A UDP packet
    Udp(UdpPacket),
    /// Unknown payload with the protocol number and payload data
    Unknown(u8, Bytes),
}

impl fmt::Debug for Ipv4Packet {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f
        .debug_struct("Ipv4Packet")
        .field("source", &self.source())
        .field("destination", &self.destination())
        .field("ttl", &self.ttl())
        .field("payload", &self.payload())
        .finish()
    }
}

impl Ipv4Packet {
    /// Create an IPv4 packet from raw packet data.
    pub fn from_bytes(data: Bytes) -> Ipv4Packet {
        Ipv4Packet {
            data,
        }
    }

    /// Return the raw packet data of the packet.
    pub fn as_bytes(&self) -> &Bytes {
        &self.data
    }

    /// Get the TTL of the packet.
    pub fn ttl(&self) -> u8 {
        self.data[8]
    }

    /// Set the TTL of the packet.
    pub fn set_ttl(&mut self, ttl: u8) {
        let bytes = mem::replace(&mut self.data, Bytes::new());
        let mut bytes_mut = BytesMut::from(bytes);
        bytes_mut[8] = ttl;
        self.data = bytes_mut.into();
    }

    /// Get the source IP address.
    pub fn source(&self) -> Ipv4Addr {
        Ipv4Addr::from([self.data[12], self.data[13], self.data[14], self.data[15]])
    }

    /// Get the destination IP address.
    pub fn destination(&self) -> Ipv4Addr {
        Ipv4Addr::from([self.data[16], self.data[17], self.data[18], self.data[19]])
    }

    /// Get the payload of the packet.
    pub fn payload(&self) -> Ipv4Payload {
        let payload = self.data.slice_from(self.header_len());
        match self.data[9] {
            17 => Ipv4Payload::Udp(UdpPacket::from_bytes(payload)),
            x => Ipv4Payload::Unknown(x, payload),
        }
    }

    /// Set the source IP address
    pub fn set_source(&mut self, addr: Ipv4Addr) {
        let bytes = mem::replace(&mut self.data, Bytes::new());
        let mut bytes_mut = BytesMut::from(bytes);
        bytes_mut[12..16].clone_from_slice(&addr.octets());
        self.data = bytes_mut.into()
    }

    /// Set the destination IP address
    pub fn set_destination(&mut self, addr: Ipv4Addr) {
        let bytes = mem::replace(&mut self.data, Bytes::new());
        let mut bytes_mut = BytesMut::from(bytes);
        bytes_mut[16..20].clone_from_slice(&addr.octets());
        self.data = bytes_mut.into()
    }

    /// Set the payload of the packet.
    pub fn set_payload(&mut self, payload: Ipv4Payload) {
        let mut bytes_mut = BytesMut::new();
        bytes_mut.extend_from_slice(&self.data[..self.header_len()]);
        match payload {
            Ipv4Payload::Udp(udp) => {
                bytes_mut[9] = 17;
                bytes_mut.extend_from_slice(&udp.as_bytes());
            },
            Ipv4Payload::Unknown(x, payload) => {
                bytes_mut[9] = x;
                bytes_mut.extend_from_slice(&payload);
            },
        };
        self.data = bytes_mut.into();
    }

    /// Get the length (in bytes) of the IPv4 header, before the payload starts.
    pub fn header_len(&self) -> usize {
        let ihl = self.data[0] & 0x0f;
        4 * ihl as usize
    }
}

pub trait Ipv4AddrExt {
    fn random_global() -> Ipv4Addr;
    fn is_global(&self) -> bool;
}

impl Ipv4AddrExt for Ipv4Addr {
    fn random_global() -> Ipv4Addr {
        loop {
            let x: u32 = rand::random();
            let ip = Ipv4Addr::from(x);
            if Ipv4AddrExt::is_global(&ip) {
                return ip;
            }
        }
    }

    fn is_global(&self) -> bool {
        !(  self.is_loopback()
        ||  self.is_private()
        ||  self.is_link_local()
        ||  self.is_multicast()
        ||  self.is_broadcast()
        ||  self.is_documentation()
        )
    }
}

