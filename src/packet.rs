use crate::priv_prelude::*;

macro_rules! slice(
    ($val:expr, ..$end:literal) => (
        slice!($val, 0..$end)
    );
    ($val:expr, $start:literal..$end:literal) => ({
        const START: usize = $start;
        const END: usize = $end;
        const LEN: usize = END - START;
        std::array::from_fn::<_, LEN, _>(|index| {
            $val[index + START]
        })
    });
);

#[derive(Clone)]
pub struct Ipv4Packet {
    data: Bytes,
}

#[derive(Clone)]
pub struct Ipv6Packet {
    data: Bytes,
}

#[derive(Clone)]
pub struct IpPacket {
    data: Bytes,
}

#[derive(Clone)]
pub enum IpPacketVersion {
    V4(Ipv4Packet),
    V6(Ipv6Packet),
}

#[derive(Clone)]
pub struct Tcpv4Packet {
    data: Bytes,
}

#[derive(Clone)]
pub struct Udpv4Packet {
    data: Bytes,
}

#[derive(Clone)]
pub enum Ipv4PacketProtocol {
    Tcp(Tcpv4Packet),
    Udp(Udpv4Packet),
}

impl IpPacket {
    pub(crate) fn new(data: Bytes) -> IpPacket {
        IpPacket { data }
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.data[..]
    }

    pub fn version(self) -> IpPacketVersion {
        let IpPacket { data } = self;
        let version = data[0] >> 4;
        match version {
            4 => IpPacketVersion::V4(Ipv4Packet { data }),
            6 => IpPacketVersion::V6(Ipv6Packet { data }),
            _ => panic!("invalid packet version field"),
        }
    }

    pub fn source_addr(&self) -> IpAddr {
        match self.clone().version() {
            IpPacketVersion::V4(packet) => IpAddr::V4(packet.source_addr()),
            IpPacketVersion::V6(packet) => IpAddr::V6(packet.source_addr()),
        }
    }

    pub fn destination_addr(&self) -> IpAddr {
        match self.clone().version() {
            IpPacketVersion::V4(packet) => IpAddr::V4(packet.destination_addr()),
            IpPacketVersion::V6(packet) => IpAddr::V6(packet.destination_addr()),
        }
    }
}

impl Ipv4Packet {
    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.data[..]
    }

    pub fn source_addr(&self) -> Ipv4Addr {
        let addr = slice!(&self.data, 12..16);
        Ipv4Addr::from(addr)
    }

    pub fn destination_addr(&self) -> Ipv4Addr {
        let addr = slice!(&self.data, 16..20);
        Ipv4Addr::from(addr)
    }

    pub fn protocol(self) -> Option<Ipv4PacketProtocol> {
        let Ipv4Packet { data } = self;
        let protocol = data[9];
        match protocol {
            6 => Some(Ipv4PacketProtocol::Tcp(Tcpv4Packet { data })),
            17 => Some(Ipv4PacketProtocol::Udp(Udpv4Packet { data })),
            _ => None,
        }
    }
}

impl Ipv6Packet {
    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.data[..]
    }

    pub fn source_addr(&self) -> Ipv6Addr {
        let addr = slice!(&self.data, 8..24);
        Ipv6Addr::from(addr)
    }

    pub fn destination_addr(&self) -> Ipv6Addr {
        let addr = slice!(&self.data, 24..40);
        Ipv6Addr::from(addr)
    }
}

impl Tcpv4Packet {
    pub fn source_addr(&self) -> SocketAddrV4 {
        let ip_addr = slice!(&self.data, 12..16);
        let ip_addr = Ipv4Addr::from(ip_addr);
        let ip_header_len = (self.data[0] & 0x0f) as usize * 4;
        let port = slice!(&self.data[ip_header_len..], 0..2);
        let port = u16::from_be_bytes(port);
        SocketAddrV4::new(ip_addr, port)
    }

    pub fn destination_addr(&self) -> SocketAddrV4 {
        let ip_addr = slice!(&self.data, 16..20);
        let ip_addr = Ipv4Addr::from(ip_addr);
        let ip_header_len = (self.data[0] & 0x0f) as usize * 4;
        let port = slice!(&self.data[ip_header_len..], 2..4);
        let port = u16::from_be_bytes(port);
        SocketAddrV4::new(ip_addr, port)
    }
}

impl Udpv4Packet {
    pub fn source_addr(&self) -> SocketAddrV4 {
        let ip_addr = slice!(&self.data, 12..16);
        let ip_addr = Ipv4Addr::from(ip_addr);
        let ip_header_len = (self.data[0] & 0x0f) as usize * 4;
        let port = slice!(&self.data[ip_header_len..], 0..2);
        let port = u16::from_be_bytes(port);
        SocketAddrV4::new(ip_addr, port)
    }

    pub fn destination_addr(&self) -> SocketAddrV4 {
        let ip_addr = slice!(&self.data, 16..20);
        let ip_addr = Ipv4Addr::from(ip_addr);
        let ip_header_len = (self.data[0] & 0x0f) as usize * 4;
        let port = slice!(&self.data[ip_header_len..], 2..4);
        let port = u16::from_be_bytes(port);
        SocketAddrV4::new(ip_addr, port)
    }
}

