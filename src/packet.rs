use crate::priv_prelude::*;
use std::{
    rc::Rc,
    borrow::Borrow,
    mem::transmute,
};

struct Ipv4Hasher {
    sum: u16,
}

impl Ipv4Hasher {
    pub fn new() -> Ipv4Hasher {
        Ipv4Hasher {
            sum: 0,
        }
    }

    pub fn write_u16(&mut self, word: u16) {
        let mut sum = self.sum as u32;
        sum += word as u32;
        self.sum = loop {
            match u16::try_from(sum) {
                Ok(sum) => break sum,
                Err(_) => {
                    let hi = sum >> 16;
                    let lo = sum & 0xffff;
                    sum = hi + lo;
                },
            }
        };
    }

    pub fn write_u32(&mut self, word: u32) {
        let hi = (word >> 16) as u16;
        let lo = (word & 0xffff) as u16;
        self.write_u16(hi);
        self.write_u16(lo);
    }

    pub fn finish(self) -> u16 {
        !self.sum
    }
}

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

macro_rules! slice_mut(
    ($val:expr, ..$end:literal) => (
        slice!($val, 0..$end)
    );
    ($val:expr, $start:literal..$end:literal) => ({
        const START: usize = $start;
        const END: usize = $end;
        const LEN: usize = END - START;
        let arr: &mut [_; LEN] = TryFrom::try_from(&mut $val[START..END]).unwrap();
        arr
    });
);

macro_rules! packet_type(
    ($name:ident, [$($parent_box:ident/$parent_arc:ident/$parent_ref:ident/$parent_mut:ident: $parent:ident),* $(,)?] $(,)?) => (
        #[repr(transparent)]
        pub struct $name {
            data: [u8],
        }

        impl Clone for Box<$name> {
            fn clone(&self) -> Box<$name> {
                let boxed: Box<[u8]> = Box::from(&self.data[..]);
                unsafe { transmute(boxed) }
            }
        }

        impl $name {
            pub fn len(&self) -> usize {
                self.data.len()
            }

            pub fn as_bytes(&self) -> &[u8] {
                &self.data[..]
            }

            $(
                pub fn $parent_box(self: Box<$name>) -> Box<$parent> {
                    unsafe { transmute(self) }
                }

                pub fn $parent_arc(self: Arc<$name>) -> Arc<$parent> {
                    unsafe { transmute(self) }
                }

                pub fn $parent_ref<'a>(&'a self) -> &'a $parent {
                    unsafe { transmute(self) }
                }

                pub fn $parent_mut<'a>(&'a mut self) -> &'a mut $parent {
                    unsafe { transmute(self) }
                }
            )*
        }
    );
);

mod protocol_numbers {
    pub const ICMP_V4: u8 = 1;
    pub const ICMP_V6: u8 = 58;
    pub const TCP: u8 = 6;
    pub const UDP: u8 = 17;
}

#[derive(Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct TcpPacketFlags {
    pub cwr: bool,
    pub ece: bool,
    pub urg: bool,
    pub ack: bool,
    pub psh: bool,
    pub rst: bool,
    pub syn: bool,
    pub fin: bool,
}

impl fmt::Debug for TcpPacketFlags {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        let mut written = false;
        let mut write = |s| {
            if written {
                write!(formatter, " | {}", s)
            } else {
                written = true;
                write!(formatter, "{}", s)
            }
        };
        if self.cwr { write("CWR")? };
        if self.ece { write("ECE")? };
        if self.urg { write("URG")? };
        if self.ack { write("ACK")? };
        if self.psh { write("PSH")? };
        if self.rst { write("RST")? };
        if self.syn { write("SYN")? };
        if self.fin { write("FIN")? };
        Ok(())
    }
}

pub trait Pointer<T: ?Sized + 'static>: Borrow<T> {
    type InsteadPointTo<U: ?Sized + 'static>: Sized;
}

impl<T: ?Sized + 'static> Pointer<T> for Box<T> {
    type InsteadPointTo<U: ?Sized + 'static> = Box<U>;
}

impl<T: ?Sized + 'static> Pointer<T> for Arc<T> {
    type InsteadPointTo<U: ?Sized + 'static> = Arc<U>;
}

impl<T: ?Sized + 'static> Pointer<T> for Rc<T> {
    type InsteadPointTo<U: ?Sized + 'static> = Rc<U>;
}

impl<'a, T: ?Sized + 'static> Pointer<T> for &'a T {
    type InsteadPointTo<U: ?Sized + 'static> = &'a U;
}

impl<'a, T: ?Sized + 'static> Pointer<T> for &'a mut T {
    type InsteadPointTo<U: ?Sized + 'static> = &'a mut U;
}

packet_type!(IpPacket, []);
packet_type!(Ipv4Packet, [
    ip_packet_box/ip_packet_arc/ip_packet_ref/ip_packet_mut: IpPacket,
]);
packet_type!(Ipv6Packet, [
    ip_packet_box/ip_packet_arc/ip_packet_ref/ip_packet_mut: IpPacket,
]);
packet_type!(Tcpv4Packet, [
    ip_packet_box/ip_packet_arc/ip_packet_ref/ip_packet_mut: IpPacket,
    ipv4_packet_box/ipv4_packet_arc/ipv4_packet_ref/ipv4_packet_mut: Ipv4Packet,
]);
packet_type!(Udpv4Packet, [
    ip_packet_box/ip_packet_arc/ip_packet_ref/ip_packet_mut: IpPacket,
    ipv4_packet_box/ipv4_packet_arc/ipv4_packet_ref/ipv4_packet_mut: Ipv4Packet,
]);
packet_type!(Icmpv4Packet, [
    ip_packet_box/ip_packet_arc/ip_packet_ref/ip_packet_mut: IpPacket,
    ipv4_packet_box/ipv4_packet_arc/ipv4_packet_ref/ipv4_packet_mut: Ipv4Packet,
]);
packet_type!(Icmpv6Packet, [
    ip_packet_box/ip_packet_arc/ip_packet_ref/ip_packet_mut: IpPacket,
    ipv6_packet_box/ipv6_packet_arc/ipv6_packet_ref/ipv6_packet_mut: Ipv6Packet,
]);

pub enum IpPacketVersion<P>
where
    P: Pointer<IpPacket>,
{
    V4(P::InsteadPointTo<Ipv4Packet>),
    V6(P::InsteadPointTo<Ipv6Packet>),
}

impl IpPacket {
    pub(crate) fn new_box(data: Box<[u8]>) -> Box<IpPacket> {
        unsafe { transmute(data) }
    }

    pub fn version_box(self: Box<IpPacket>) -> IpPacketVersion<Box<IpPacket>> {
        match self.data[0] >> 4 {
            4 => IpPacketVersion::V4(unsafe { transmute(self) }),
            6 => IpPacketVersion::V6(unsafe { transmute(self) }),
            _ => panic!("unknown packet version"),
        }
    }

    pub fn version_ref<'a>(&'a self) -> IpPacketVersion<&'a IpPacket> {
        match self.data[0] >> 4 {
            4 => IpPacketVersion::V4(unsafe { transmute(self) }),
            6 => IpPacketVersion::V6(unsafe { transmute(self) }),
            _ => panic!("unknown packet version"),
        }
    }

    pub fn version_mut<'a>(&'a mut self) -> IpPacketVersion<&'a mut IpPacket> {
        match self.data[0] >> 4 {
            4 => IpPacketVersion::V4(unsafe { transmute(self) }),
            6 => IpPacketVersion::V6(unsafe { transmute(self) }),
            _ => panic!("unknown packet version"),
        }
    }

    pub fn source_addr(&self) -> IpAddr {
        match self.version_ref() {
            IpPacketVersion::V4(packet) => IpAddr::V4(packet.source_addr()),
            IpPacketVersion::V6(packet) => IpAddr::V6(packet.source_addr()),
        }
    }

    pub fn destination_addr(&self) -> IpAddr {
        match self.version_ref() {
            IpPacketVersion::V4(packet) => IpAddr::V4(packet.destination_addr()),
            IpPacketVersion::V6(packet) => IpAddr::V6(packet.destination_addr()),
        }
    }
}

pub enum Ipv4PacketProtocol<P>
where
    P: Pointer<Ipv4Packet>,
{
    Tcp(P::InsteadPointTo<Tcpv4Packet>),
    Udp(P::InsteadPointTo<Udpv4Packet>),
    Icmp(P::InsteadPointTo<Icmpv4Packet>),
    Unknown {
        protocol_number: u8,
    },
}

impl Ipv4Packet {
    pub fn protocol_box(self: Box<Ipv4Packet>) -> Ipv4PacketProtocol<Box<Ipv4Packet>> {
        match self.data[9] {
            protocol_numbers::TCP => Ipv4PacketProtocol::Tcp(unsafe { transmute(self) }),
            protocol_numbers::UDP => Ipv4PacketProtocol::Udp(unsafe { transmute(self) }),
            protocol_numbers::ICMP_V4 => Ipv4PacketProtocol::Icmp(unsafe { transmute(self) }),
            protocol_number => Ipv4PacketProtocol::Unknown { protocol_number },
        }
    }

    pub fn protocol_ref<'a>(&'a self) -> Ipv4PacketProtocol<&'a Ipv4Packet> {
        match self.data[9] {
            protocol_numbers::TCP => Ipv4PacketProtocol::Tcp(unsafe { transmute(self) }),
            protocol_numbers::UDP => Ipv4PacketProtocol::Udp(unsafe { transmute(self) }),
            protocol_numbers::ICMP_V4 => Ipv4PacketProtocol::Icmp(unsafe { transmute(self) }),
            protocol_number => Ipv4PacketProtocol::Unknown { protocol_number },
        }
    }

    pub fn protocol_mut<'a>(&'a mut self) -> Ipv4PacketProtocol<&'a mut Ipv4Packet> {
        match self.data[9] {
            protocol_numbers::TCP => Ipv4PacketProtocol::Tcp(unsafe { transmute(self) }),
            protocol_numbers::UDP => Ipv4PacketProtocol::Udp(unsafe { transmute(self) }),
            protocol_numbers::ICMP_V4 => Ipv4PacketProtocol::Icmp(unsafe { transmute(self) }),
            protocol_number => Ipv4PacketProtocol::Unknown { protocol_number },
        }
    }

    pub fn source_addr(&self) -> Ipv4Addr {
        let addr = slice!(&self.data, 12..16);
        Ipv4Addr::from(addr)
    }

    pub fn destination_addr(&self) -> Ipv4Addr {
        let addr = slice!(&self.data, 16..20);
        Ipv4Addr::from(addr)
    }

    pub fn set_source_addr(&mut self, addr: Ipv4Addr) {
        *slice_mut!(self.data, 12..16) = addr.octets();
        self.fix_checksum();
    }

    pub fn set_destination_addr(&mut self, addr: Ipv4Addr) {
        *slice_mut!(self.data, 16..20) = addr.octets();
        self.fix_checksum();
    }

    pub fn ipv4_header_len(&self) -> usize {
        (self.data[0] & 0x0f) as usize * 4
    }

    fn fix_checksum(&mut self) {
        let mut hasher = Ipv4Hasher::new();
        let header_len = self.ipv4_header_len();
        let mut i = 0;
        while i < header_len {
            if i != 10 {
                hasher.write_u16(u16::from_be_bytes(slice!(&self.data[i..], 0..2)));
            }
            i += 2;
        }
        *slice_mut!(self.data, 10..12) = hasher.finish().to_be_bytes();
    }
}

pub enum Ipv6PacketProtocol<P>
where
    P: Pointer<Ipv6Packet>,
{
    Icmp(P::InsteadPointTo<Icmpv6Packet>),
    Unknown {
        protocol_number: u8,
    },
}

impl Ipv6Packet {
    pub fn protocol_box(self: Box<Ipv6Packet>) -> Ipv6PacketProtocol<Box<Ipv6Packet>> {
        match self.data[6] {
            protocol_numbers::ICMP_V6 => Ipv6PacketProtocol::Icmp(unsafe { transmute(self) }),
            protocol_number => Ipv6PacketProtocol::Unknown { protocol_number },
        }
    }

    pub fn protocol_ref<'a>(&'a self) -> Ipv6PacketProtocol<&'a Ipv6Packet> {
        match self.data[6] {
            protocol_numbers::ICMP_V6 => Ipv6PacketProtocol::Icmp(unsafe { transmute(self) }),
            protocol_number => Ipv6PacketProtocol::Unknown { protocol_number },
        }
    }

    pub fn protocol_mut<'a>(&'a mut self) -> Ipv6PacketProtocol<&'a mut Ipv6Packet> {
        match self.data[6] {
            protocol_numbers::ICMP_V6 => Ipv6PacketProtocol::Icmp(unsafe { transmute(self) }),
            protocol_number => Ipv6PacketProtocol::Unknown { protocol_number },
        }
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
    pub fn source_ip_addr(&self) -> Ipv4Addr {
        self.ipv4_packet_ref().source_addr()
    }

    pub fn destination_ip_addr(&self) -> Ipv4Addr {
        self.ipv4_packet_ref().destination_addr()
    }

    pub fn source_port(&self) -> u16 {
        let header_len = self.ipv4_packet_ref().ipv4_header_len();
        let port = slice!(&self.data[header_len..], 0..2);
        u16::from_be_bytes(port)
    }

    pub fn destination_port(&self) -> u16 {
        let header_len = self.ipv4_packet_ref().ipv4_header_len();
        let port = slice!(&self.data[header_len..], 2..4);
        u16::from_be_bytes(port)
    }

    pub fn source_addr(&self) -> SocketAddrV4 {
        let ip_addr = self.source_ip_addr();
        let port = self.source_port();
        SocketAddrV4::new(ip_addr, port)
    }

    pub fn destination_addr(&self) -> SocketAddrV4 {
        let ip_addr = self.destination_ip_addr();
        let port = self.destination_port();
        SocketAddrV4::new(ip_addr, port)
    }

    pub fn flags(&self) -> TcpPacketFlags {
        let header_len = self.ipv4_packet_ref().ipv4_header_len();
        let flags = self.data[header_len + 13];
        TcpPacketFlags {
            cwr: (flags >> 7) & 1 == 1,
            ece: (flags >> 6) & 1 == 1,
            urg: (flags >> 5) & 1 == 1,
            ack: (flags >> 4) & 1 == 1,
            psh: (flags >> 3) & 1 == 1,
            rst: (flags >> 2) & 1 == 1,
            syn: (flags >> 1) & 1 == 1,
            fin: (flags >> 0) & 1 == 1,
        }
    }

    pub fn set_source_port(&mut self, port: u16) {
        self.set_source_port_inner(port);
        self.fix_checksum();
    }

    fn set_source_port_inner(&mut self, port: u16) {
        let header_len = self.ipv4_packet_ref().ipv4_header_len();
        *slice_mut!(&mut self.data[header_len..], 0..2) = port.to_be_bytes();
    }

    pub fn set_destination_port(&mut self, port: u16) {
        self.set_destination_port_inner(port);
        self.fix_checksum();
    }

    fn set_destination_port_inner(&mut self, port: u16) {
        let header_len = self.ipv4_packet_ref().ipv4_header_len();
        *slice_mut!(&mut self.data[header_len..], 2..4) = port.to_be_bytes();
    }

    pub fn set_source_addr(&mut self, addr: SocketAddrV4) {
        self.ipv4_packet_mut().set_source_addr(*addr.ip());
        self.set_source_port_inner(addr.port());
        self.fix_checksum();
    }

    pub fn set_destination_addr(&mut self, addr: SocketAddrV4) {
        self.ipv4_packet_mut().set_destination_addr(*addr.ip());
        self.set_destination_port_inner(addr.port());
        self.fix_checksum();
    }

    fn fix_checksum(&mut self) {
        let ipv4_header_len = self.ipv4_packet_ref().ipv4_header_len();
        let mut hasher = Ipv4Hasher::new();
        hasher.write_u32(u32::from(self.ipv4_packet_ref().source_addr()));
        hasher.write_u32(u32::from(self.ipv4_packet_ref().destination_addr()));
        hasher.write_u16(protocol_numbers::TCP as u16);
        hasher.write_u16((self.data.len() - ipv4_header_len) as u16);
        let mut i = ipv4_header_len;
        while i + 1 < self.data.len() {
            if i != ipv4_header_len + 16 {
                hasher.write_u16(u16::from_be_bytes(slice!(&self.data[i..], 0..2)));
            }
            i += 2;
        }
        if i < self.data.len() {
            debug_assert_eq!(i + 1, self.data.len());
            hasher.write_u16((self.data[i] as u16) << 8);
        }
        *slice_mut!(&mut self.data[ipv4_header_len..], 16..18) = hasher.finish().to_be_bytes();
    }
}

impl Udpv4Packet {
    pub fn source_ip_addr(&self) -> Ipv4Addr {
        self.ipv4_packet_ref().source_addr()
    }

    pub fn destination_ip_addr(&self) -> Ipv4Addr {
        self.ipv4_packet_ref().destination_addr()
    }

    pub fn source_port(&self) -> u16 {
        let header_len = self.ipv4_packet_ref().ipv4_header_len();
        let port = slice!(&self.data[header_len..], 0..2);
        u16::from_be_bytes(port)
    }

    pub fn destination_port(&self) -> u16 {
        let header_len = self.ipv4_packet_ref().ipv4_header_len();
        let port = slice!(&self.data[header_len..], 2..4);
        u16::from_be_bytes(port)
    }

    pub fn source_addr(&self) -> SocketAddrV4 {
        let ip_addr = self.source_ip_addr();
        let port = self.source_port();
        SocketAddrV4::new(ip_addr, port)
    }

    pub fn destination_addr(&self) -> SocketAddrV4 {
        let ip_addr = self.destination_ip_addr();
        let port = self.destination_port();
        SocketAddrV4::new(ip_addr, port)
    }

    pub fn set_source_port(&mut self, port: u16) {
        self.set_source_port_inner(port);
        self.fix_checksum();
    }

    fn set_source_port_inner(&mut self, port: u16) {
        let header_len = self.ipv4_packet_ref().ipv4_header_len();
        *slice_mut!(&mut self.data[header_len..], 0..2) = port.to_be_bytes();
    }

    pub fn set_destination_port(&mut self, port: u16) {
        self.set_destination_port_inner(port);
        self.fix_checksum();
    }

    fn set_destination_port_inner(&mut self, port: u16) {
        let header_len = self.ipv4_packet_ref().ipv4_header_len();
        *slice_mut!(&mut self.data[header_len..], 2..4) = port.to_be_bytes();
    }

    pub fn set_source_addr(&mut self, addr: SocketAddrV4) {
        self.ipv4_packet_mut().set_source_addr(*addr.ip());
        self.set_source_port_inner(addr.port());
        self.fix_checksum();
    }

    pub fn set_destination_addr(&mut self, addr: SocketAddrV4) {
        self.ipv4_packet_mut().set_destination_addr(*addr.ip());
        self.set_destination_port_inner(addr.port());
        self.fix_checksum();
    }

    fn fix_checksum(&mut self) {
        let ipv4_header_len = self.ipv4_packet_ref().ipv4_header_len();
        let mut hasher = Ipv4Hasher::new();
        hasher.write_u32(u32::from(self.ipv4_packet_ref().source_addr()));
        hasher.write_u32(u32::from(self.ipv4_packet_ref().destination_addr()));
        hasher.write_u16(protocol_numbers::UDP as u16);
        hasher.write_u16((self.data.len() - ipv4_header_len) as u16);
        let mut i = ipv4_header_len;
        while i + 1 < self.data.len() {
            if i != ipv4_header_len + 6 {
                hasher.write_u16(u16::from_be_bytes(slice!(&self.data[i..], 0..2)));
            }
            i += 2;
        }
        if i < self.data.len() {
            debug_assert_eq!(i + 1, self.data.len());
            hasher.write_u16((self.data[i] as u16) << 8);
        }
        *slice_mut!(&mut self.data[ipv4_header_len..], 16..18) = hasher.finish().to_be_bytes();
    }
}

impl Icmpv4Packet {
    pub fn source_addr(&self) -> Ipv4Addr {
        self.ipv4_packet_ref().source_addr()
    }

    pub fn destination_addr(&self) -> Ipv4Addr {
        self.ipv4_packet_ref().destination_addr()
    }
}

impl Icmpv6Packet {
    pub fn source_addr(&self) -> Ipv6Addr {
        self.ipv6_packet_ref().source_addr()
    }

    pub fn destination_addr(&self) -> Ipv6Addr {
        self.ipv6_packet_ref().destination_addr()
    }
}

impl fmt::Debug for IpPacket {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        match self.version_ref() {
            IpPacketVersion::V4(packet) => fmt::Debug::fmt(&packet, formatter),
            IpPacketVersion::V6(packet) => fmt::Debug::fmt(&packet, formatter),
        }
    }
}

impl fmt::Debug for Ipv4Packet {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        match self.protocol_ref() {
            Ipv4PacketProtocol::Tcp(tcp) => fmt::Debug::fmt(&tcp, formatter),
            Ipv4PacketProtocol::Udp(udp) => fmt::Debug::fmt(&udp, formatter),
            Ipv4PacketProtocol::Icmp(icmp) => fmt::Debug::fmt(&icmp, formatter),
            Ipv4PacketProtocol::Unknown { protocol_number } => {
                formatter
                .debug_struct("Ipv4Packet")
                .field("source_addr", &self.source_addr())
                .field("destination_addr", &self.destination_addr())
                .field("protocol", &protocol_number)
                .finish()
            },
        }
    }
}

impl fmt::Debug for Ipv6Packet {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        match self.protocol_ref() {
            Ipv6PacketProtocol::Icmp(icmp) => fmt::Debug::fmt(&icmp, formatter),
            Ipv6PacketProtocol::Unknown { protocol_number } => {
                formatter
                .debug_struct("Ipv6Packet")
                .field("source_addr", &self.source_addr())
                .field("destination_addr", &self.destination_addr())
                .field("protocol", &protocol_number)
                .finish()
            },
        }
    }
}

impl fmt::Debug for Tcpv4Packet {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter
        .debug_struct("Tcpv4Packet")
        .field("source_addr", &self.source_addr())
        .field("destination_addr", &self.destination_addr())
        .field("flags", &self.flags())
        .finish()
    }
}

impl fmt::Debug for Udpv4Packet {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter
        .debug_struct("Udpv4Packet")
        .field("source_addr", &self.source_addr())
        .field("destination_addr", &self.destination_addr())
        .finish()
    }
}

impl fmt::Debug for Icmpv4Packet {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter
        .debug_struct("Icmpv4Packet")
        .field("source_addr", &self.source_addr())
        .field("destination_addr", &self.destination_addr())
        .finish()
    }
}

impl fmt::Debug for Icmpv6Packet {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter
        .debug_struct("Icmpv6Packet")
        .field("source_addr", &self.source_addr())
        .field("destination_addr", &self.destination_addr())
        .finish()
    }
}

