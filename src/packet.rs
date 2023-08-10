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
    ($name:ident, $name_ref:ident, $name_mut:ident) => (
        #[derive(Clone)]
        pub struct $name {
            data: BytesMut,
        }

        #[derive(Clone, Copy)]
        pub struct $name_ref<'a> {
            data: &'a [u8],
        }

        pub struct $name_mut<'a> {
            data: &'a mut BytesMut,
        }

        impl $name {
            pub fn len(&self) -> usize {
                self.data.len()
            }

            pub fn as_bytes(&self) -> &[u8] {
                &self.data[..]
            }

            pub fn as_ref<'a>(&'a self) -> $name_ref<'a> {
                $name_ref {
                    data: &self.data[..],
                }
            }

            pub fn as_mut<'a>(&'a mut self) -> $name_mut<'a> {
                $name_mut {
                    data: &mut self.data,
                }
            }
        }

        impl<'a> $name_ref<'a> {
            pub fn len(&self) -> usize {
                self.data.len()
            }

            pub fn as_bytes(&self) -> &[u8] {
                &self.data[..]
            }
        }

        impl<'a> $name_mut<'a> {
            pub fn len(&self) -> usize {
                self.data.len()
            }

            pub fn as_bytes(&self) -> &[u8] {
                &self.data[..]
            }

            pub fn as_ref<'b>(&'b self) -> $name_ref<'b> {
                $name_ref {
                    data: self.data,
                }
            }

            pub fn as_mut<'b>(&'b mut self) -> $name_mut<'b> {
                $name_mut {
                    data: self.data,
                }
            }

            pub fn to_ref(self) -> $name_ref<'a> {
                $name_ref {
                    data: self.data,
                }
            }
        }
    );
);

macro_rules! packet_enum(
    ($name:ident, $name_ref:ident, $name_mut:ident, [
        $(($variant:ident, $inner:ident, $inner_ref:ident, $inner_mut:ident),)*
    ]) => (
        #[derive(Clone)]
        pub enum $name {
            $(
                $variant($inner),
            )*
        }

        #[derive(Clone, Copy)]
        pub enum $name_ref<'a> {
            $(
                $variant($inner_ref<'a>),
            )*
        }

        pub enum $name_mut<'a> {
            $(
                $variant($inner_mut<'a>),
            )*
        }
    );
);

packet_type!(IpPacket, IpPacketRef, IpPacketMut);
packet_type!(Ipv4Packet, Ipv4PacketRef, Ipv4PacketMut);
packet_type!(Ipv6Packet, Ipv6PacketRef, Ipv6PacketMut);
packet_type!(Tcpv4Packet, Tcpv4PacketRef, Tcpv4PacketMut);
packet_type!(Udpv4Packet, Udpv4PacketRef, Udpv4PacketMut);

packet_enum!(IpPacketVersion, IpPacketVersionRef, IpPacketVersionMut, [
    (V4, Ipv4Packet, Ipv4PacketRef, Ipv4PacketMut),
    (V6, Ipv6Packet, Ipv6PacketRef, Ipv6PacketMut),
]);
packet_enum!(Ipv4PacketProtocol, Ipv4PacketProtocolRef, Ipv4PacketProtocolMut, [
    (Tcp, Tcpv4Packet, Tcpv4PacketRef, Tcpv4PacketMut),
    (Udp, Udpv4Packet, Udpv4PacketRef, Udpv4PacketMut),
]);

impl IpPacket {
    pub(crate) fn new(data: BytesMut) -> IpPacket {
        IpPacket { data }
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
        self.as_ref().source_addr()
    }

    pub fn destination_addr(&self) -> IpAddr {
        self.as_ref().destination_addr()
    }
}

impl<'a> IpPacketRef<'a> {
    pub fn version(self) -> IpPacketVersionRef<'a> {
        let IpPacketRef { data } = self;
        let version = data[0] >> 4;
        match version {
            4 => IpPacketVersionRef::V4(Ipv4PacketRef { data }),
            6 => IpPacketVersionRef::V6(Ipv6PacketRef { data }),
            _ => panic!("invalid packet version field"),
        }
    }

    pub fn source_addr(&self) -> IpAddr {
        match self.version() {
            IpPacketVersionRef::V4(packet) => IpAddr::V4(packet.source_addr()),
            IpPacketVersionRef::V6(packet) => IpAddr::V6(packet.source_addr()),
        }
    }

    pub fn destination_addr(&self) -> IpAddr {
        match self.version() {
            IpPacketVersionRef::V4(packet) => IpAddr::V4(packet.destination_addr()),
            IpPacketVersionRef::V6(packet) => IpAddr::V6(packet.destination_addr()),
        }
    }
}

impl<'a> IpPacketMut<'a> {
    pub fn version(self) -> IpPacketVersionMut<'a> {
        let IpPacketMut { data } = self;
        let version = data[0] >> 4;
        match version {
            4 => IpPacketVersionMut::V4(Ipv4PacketMut { data }),
            6 => IpPacketVersionMut::V6(Ipv6PacketMut { data }),
            _ => panic!("invalid packet version field"),
        }
    }

    pub fn source_addr(&self) -> IpAddr {
        self.as_ref().source_addr()
    }

    pub fn destination_addr(&self) -> IpAddr {
        self.as_ref().destination_addr()
    }
}

impl Ipv4Packet {
    pub fn ip_packet(self) -> IpPacket {
        let Ipv4Packet { data } = self;
        IpPacket { data }
    }

    pub fn source_addr(&self) -> Ipv4Addr {
        self.as_ref().source_addr()
    }

    pub fn destination_addr(&self) -> Ipv4Addr {
        self.as_ref().destination_addr()
    }

    pub fn set_source_addr(&mut self, addr: Ipv4Addr) {
        self.as_mut().set_source_addr(addr)
    }

    pub fn set_destination_addr(&mut self, addr: Ipv4Addr) {
        self.as_mut().set_destination_addr(addr)
    }

    pub fn ipv4_header_len(&self) -> usize {
        self.as_ref().ipv4_header_len()
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

impl<'a> Ipv4PacketRef<'a> {
    pub fn ip_packet(self) -> IpPacketRef<'a> {
        let Ipv4PacketRef { data } = self;
        IpPacketRef { data }
    }

    pub fn source_addr(&self) -> Ipv4Addr {
        let addr = slice!(&self.data, 12..16);
        Ipv4Addr::from(addr)
    }

    pub fn destination_addr(&self) -> Ipv4Addr {
        let addr = slice!(&self.data, 16..20);
        Ipv4Addr::from(addr)
    }

    pub fn ipv4_header_len(&self) -> usize {
        (self.data[0] & 0x0f) as usize * 4
    }

    pub fn protocol(self) -> Option<Ipv4PacketProtocolRef<'a>> {
        let Ipv4PacketRef { data } = self;
        let protocol = data[9];
        match protocol {
            6 => Some(Ipv4PacketProtocolRef::Tcp(Tcpv4PacketRef { data })),
            17 => Some(Ipv4PacketProtocolRef::Udp(Udpv4PacketRef { data })),
            _ => None,
        }
    }
}

impl<'a> Ipv4PacketMut<'a> {
    pub fn ip_packet(self) -> IpPacketMut<'a> {
        let Ipv4PacketMut { data } = self;
        IpPacketMut { data }
    }

    pub fn source_addr(&self) -> Ipv4Addr {
        self.as_ref().source_addr()
    }

    pub fn destination_addr(&self) -> Ipv4Addr {
        self.as_ref().destination_addr()
    }

    pub fn set_source_addr(&mut self, addr: Ipv4Addr) {
        *slice_mut!(self.data, 12..16) = addr.octets();
    }

    pub fn set_destination_addr(&mut self, addr: Ipv4Addr) {
        *slice_mut!(self.data, 16..20) = addr.octets();
    }

    pub fn ipv4_header_len(&self) -> usize {
        self.as_ref().ipv4_header_len()
    }

    pub fn protocol(self) -> Option<Ipv4PacketProtocolMut<'a>> {
        let Ipv4PacketMut { data } = self;
        let protocol = data[9];
        match protocol {
            6 => Some(Ipv4PacketProtocolMut::Tcp(Tcpv4PacketMut { data })),
            17 => Some(Ipv4PacketProtocolMut::Udp(Udpv4PacketMut { data })),
            _ => None,
        }
    }
}

impl Ipv6Packet {
    pub fn ip_packet(self) -> IpPacket {
        let Ipv6Packet { data } = self;
        IpPacket { data }
    }

    pub fn source_addr(&self) -> Ipv6Addr {
        self.as_ref().source_addr()
    }

    pub fn destination_addr(&self) -> Ipv6Addr {
        self.as_ref().destination_addr()
    }

    pub fn set_source_addr(&mut self, addr: Ipv6Addr) {
        self.as_mut().set_source_addr(addr);
    }

    pub fn set_destination_addr(&mut self, addr: Ipv6Addr) {
        self.as_mut().set_destination_addr(addr);
    }
}

impl<'a> Ipv6PacketRef<'a> {
    pub fn ip_packet(self) -> IpPacketRef<'a> {
        let Ipv6PacketRef { data } = self;
        IpPacketRef { data }
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

impl<'a> Ipv6PacketMut<'a> {
    pub fn ip_packet(self) -> IpPacketMut<'a> {
        let Ipv6PacketMut { data } = self;
        IpPacketMut { data }
    }

    pub fn source_addr(&self) -> Ipv6Addr {
        self.as_ref().source_addr()
    }

    pub fn destination_addr(&self) -> Ipv6Addr {
        self.as_ref().destination_addr()
    }

    pub fn set_source_addr(&mut self, addr: Ipv6Addr) {
        *slice_mut!(self.data, 8..24) = addr.octets();
    }

    pub fn set_destination_addr(&mut self, addr: Ipv6Addr) {
        *slice_mut!(self.data, 24..40) = addr.octets();
    }
}

impl Tcpv4Packet {
    pub fn ip_packet(self) -> IpPacket {
        let Tcpv4Packet { data } = self;
        IpPacket { data }
    }

    pub fn ipv4_packet(self) -> Ipv4Packet {
        let Tcpv4Packet { data } = self;
        Ipv4Packet { data }
    }

    pub fn source_ip_addr(&self) -> Ipv4Addr {
        self.as_ref().source_ip_addr()
    }

    pub fn destination_ip_addr(&self) -> Ipv4Addr {
        self.as_ref().destination_ip_addr()
    }

    pub fn source_port(&self) -> u16 {
        self.as_ref().source_port()
    }

    pub fn destination_port(&self) -> u16 {
        self.as_ref().destination_port()
    }

    pub fn set_source_port(&mut self, port: u16) {
        self.as_mut().set_source_port(port);
    }

    pub fn set_destination_port(&mut self, port: u16) {
        self.as_mut().set_destination_port(port);
    }

    pub fn source_addr(&self) -> SocketAddrV4 {
        self.as_ref().source_addr()
    }

    pub fn destination_addr(&self) -> SocketAddrV4 {
        self.as_ref().destination_addr()
    }

    pub fn set_source_addr(&mut self, addr: SocketAddrV4) {
        self.as_mut().set_source_addr(addr);
    }

    pub fn set_destination_addr(&mut self, addr: SocketAddrV4) {
        self.as_mut().set_destination_addr(addr);
    }
}

impl<'a> Tcpv4PacketRef<'a> {
    pub fn ip_packet(self) -> IpPacketRef<'a> {
        let Tcpv4PacketRef { data } = self;
        IpPacketRef { data }
    }

    pub fn ipv4_packet(self) -> Ipv4PacketRef<'a> {
        let Tcpv4PacketRef { data } = self;
        Ipv4PacketRef { data }
    }

    pub fn source_ip_addr(&self) -> Ipv4Addr {
        self.ipv4_packet().source_addr()
    }

    pub fn destination_ip_addr(&self) -> Ipv4Addr {
        self.ipv4_packet().destination_addr()
    }

    pub fn source_port(&self) -> u16 {
        let header_len = self.ipv4_packet().ipv4_header_len();
        let port = slice!(&self.data[header_len..], 0..2);
        u16::from_be_bytes(port)
    }

    pub fn destination_port(&self) -> u16 {
        let header_len = self.ipv4_packet().ipv4_header_len();
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
}

impl<'a> Tcpv4PacketMut<'a> {
    pub fn ip_packet(self) -> IpPacketMut<'a> {
        let Tcpv4PacketMut { data } = self;
        IpPacketMut { data }
    }

    pub fn ipv4_packet(self) -> Ipv4PacketMut<'a> {
        let Tcpv4PacketMut { data } = self;
        Ipv4PacketMut { data }
    }

    pub fn source_ip_addr(&self) -> Ipv4Addr {
        self.as_ref().source_ip_addr()
    }

    pub fn destination_ip_addr(&self) -> Ipv4Addr {
        self.as_ref().destination_ip_addr()
    }

    pub fn source_port(&self) -> u16 {
        self.as_ref().source_port()
    }

    pub fn destination_port(&self) -> u16 {
        self.as_ref().destination_port()
    }

    pub fn set_source_port(&mut self, port: u16) {
        *slice_mut!(self.data, 0..2) = port.to_be_bytes();
    }

    pub fn set_destination_port(&mut self, port: u16) {
        *slice_mut!(self.data, 2..4) = port.to_be_bytes();
    }

    pub fn source_addr(&self) -> SocketAddrV4 {
        self.as_ref().source_addr()
    }

    pub fn destination_addr(&self) -> SocketAddrV4 {
        self.as_ref().destination_addr()
    }

    pub fn set_source_addr(&mut self, addr: SocketAddrV4) {
        self.as_mut().ipv4_packet().set_source_addr(*addr.ip());
        self.set_source_port(addr.port());
    }

    pub fn set_destination_addr(&mut self, addr: SocketAddrV4) {
        self.as_mut().ipv4_packet().set_destination_addr(*addr.ip());
        self.set_destination_port(addr.port());
    }
}

impl Udpv4Packet {
    pub fn ip_packet(self) -> IpPacket {
        let Udpv4Packet { data } = self;
        IpPacket { data }
    }

    pub fn ipv4_packet(self) -> Ipv4Packet {
        let Udpv4Packet { data } = self;
        Ipv4Packet { data }
    }

    pub fn source_ip_addr(&self) -> Ipv4Addr {
        self.as_ref().source_ip_addr()
    }

    pub fn destination_ip_addr(&self) -> Ipv4Addr {
        self.as_ref().destination_ip_addr()
    }

    pub fn source_port(&self) -> u16 {
        self.as_ref().source_port()
    }

    pub fn destination_port(&self) -> u16 {
        self.as_ref().destination_port()
    }

    pub fn set_source_port(&mut self, port: u16) {
        self.as_mut().set_source_port(port);
    }

    pub fn set_destination_port(&mut self, port: u16) {
        self.as_mut().set_destination_port(port);
    }

    pub fn source_addr(&self) -> SocketAddrV4 {
        self.as_ref().source_addr()
    }

    pub fn destination_addr(&self) -> SocketAddrV4 {
        self.as_ref().destination_addr()
    }

    pub fn set_source_addr(&mut self, addr: SocketAddrV4) {
        self.as_mut().set_source_addr(addr);
    }

    pub fn set_destination_addr(&mut self, addr: SocketAddrV4) {
        self.as_mut().set_destination_addr(addr);
    }
}

impl<'a> Udpv4PacketRef<'a> {
    pub fn ip_packet(self) -> IpPacketRef<'a> {
        let Udpv4PacketRef { data } = self;
        IpPacketRef { data }
    }

    pub fn ipv4_packet(self) -> Ipv4PacketRef<'a> {
        let Udpv4PacketRef { data } = self;
        Ipv4PacketRef { data }
    }

    pub fn source_ip_addr(&self) -> Ipv4Addr {
        self.ipv4_packet().source_addr()
    }

    pub fn destination_ip_addr(&self) -> Ipv4Addr {
        self.ipv4_packet().destination_addr()
    }

    pub fn source_port(&self) -> u16 {
        let header_len = self.ipv4_packet().ipv4_header_len();
        let port = slice!(&self.data[header_len..], 0..2);
        u16::from_be_bytes(port)
    }

    pub fn destination_port(&self) -> u16 {
        let header_len = self.ipv4_packet().ipv4_header_len();
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
}

impl<'a> Udpv4PacketMut<'a> {
    pub fn ip_packet(self) -> IpPacketMut<'a> {
        let Udpv4PacketMut { data } = self;
        IpPacketMut { data }
    }

    pub fn ipv4_packet(self) -> Ipv4PacketMut<'a> {
        let Udpv4PacketMut { data } = self;
        Ipv4PacketMut { data }
    }

    pub fn source_ip_addr(&self) -> Ipv4Addr {
        self.as_ref().source_ip_addr()
    }

    pub fn destination_ip_addr(&self) -> Ipv4Addr {
        self.as_ref().destination_ip_addr()
    }

    pub fn source_port(&self) -> u16 {
        self.as_ref().source_port()
    }

    pub fn destination_port(&self) -> u16 {
        self.as_ref().destination_port()
    }

    pub fn set_source_port(&mut self, port: u16) {
        *slice_mut!(self.data, 0..2) = port.to_be_bytes();
    }

    pub fn set_destination_port(&mut self, port: u16) {
        *slice_mut!(self.data, 2..4) = port.to_be_bytes();
    }

    pub fn source_addr(&self) -> SocketAddrV4 {
        self.as_ref().source_addr()
    }

    pub fn destination_addr(&self) -> SocketAddrV4 {
        self.as_ref().destination_addr()
    }

    pub fn set_source_addr(&mut self, addr: SocketAddrV4) {
        self.as_mut().ipv4_packet().set_source_addr(*addr.ip());
        self.set_source_port(addr.port());
    }

    pub fn set_destination_addr(&mut self, addr: SocketAddrV4) {
        self.as_mut().ipv4_packet().set_destination_addr(*addr.ip());
        self.set_destination_port(addr.port());
    }
}

