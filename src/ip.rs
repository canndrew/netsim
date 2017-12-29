use std::net::Ipv4Addr;
use bytes::Bytes;
use udp::UdpPacket;

#[derive(Debug)]
pub struct Ipv4Packet {
    data: Bytes,
}

#[derive(Debug)]
pub enum Ipv4Payload {
    Udp(UdpPacket),
    Unknown(u8, Bytes),
}

#[derive(Debug)]
pub struct Ipv6Packet {
    data: Bytes,
}

impl Ipv4Packet {
    pub fn new(data: Bytes) -> Ipv4Packet {
        Ipv4Packet {
            data,
        }
    }

    pub fn ttl(&self) -> u16 {
        ((self.data[8] as u16) << 8) | (self.data[9] as u16)
    }

    /*
    pub fn set_ttl(&mut self, ttl: u16) {
        self.data[8] = (ttl >> 8) as u8;
        self.data[9] = (ttl & 0xff) as u8;
    }
    */

    pub fn source(&self) -> Ipv4Addr {
        Ipv4Addr::from([self.data[12], self.data[13], self.data[14], self.data[15]])
    }

    pub fn destination(&self) -> Ipv4Addr {
        Ipv4Addr::from([self.data[16], self.data[17], self.data[18], self.data[19]])
    }

    pub fn payload(&self) -> Ipv4Payload {
        let ihl = self.data[0] & 0x0f;
        let payload = self.data.slice_from(4 * ihl as usize);
        match self.data[9] {
            17 => Ipv4Payload::Udp(UdpPacket::new(payload)),
            x => Ipv4Payload::Unknown(x, payload),
        }
    }
}

impl Ipv6Packet {
    pub fn new(data: Bytes) -> Ipv6Packet {
        Ipv6Packet {
            data,
        }
    }
}

