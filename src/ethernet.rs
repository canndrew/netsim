use ip::{Ipv4Packet, Ipv6Packet};
use bytes::{Bytes, BytesMut};

pub struct MacAddr {
    bytes: [u8; 6],
}

/*
impl Display for MacAddr {
}
*/

pub struct EtherFrame {
    data: Bytes,
}

pub enum EtherPayload {
    Ipv4(Ipv4Packet),
    Ipv6(Ipv6Packet),
    Unknown([u8; 2], Bytes),
}

impl EtherFrame {
    pub fn from_bytes(data: Bytes) -> EtherFrame {
        EtherFrame {
            data,
        }
    }

    pub fn source(&self) -> MacAddr {
        let mut bytes = [0u8; 6];
        bytes[..].clone_from_slice(&self.data[0..6]);
        MacAddr { bytes }
    }

    pub fn destination(&self) -> MacAddr {
        let mut bytes = [0u8; 6];
        bytes[..].clone_from_slice(&self.data[6..12]);
        MacAddr { bytes }
    }

    pub fn data(&self) -> &Bytes {
        &self.data
    }

    pub fn payload(&self) -> EtherPayload {
        match (self.data[12], self.data[13]) {
            (0x08, 0x00) => EtherPayload::Ipv4(Ipv4Packet::new(self.data.slice_from(14))),
            (0x86, 0xdd) => EtherPayload::Ipv6(Ipv6Packet::new(self.data.slice_from(14))),
            (x, y) => EtherPayload::Unknown([x, y], self.data.slice_from(14)),
        }
    }
}

