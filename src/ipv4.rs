use priv_prelude::*;
use rand;
use util;

pub trait Ipv4AddrExt {
    /// Get a random, global IPv4 address.
    fn random_global() -> Ipv4Addr;
    /// Returns `true` if this is a global IPv4 address
    fn is_global(&self) -> bool;
    /// Returns `true` if this is a reserved IPv4 address.
    fn is_reserved(&self) -> bool;
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
        ||  self.is_reserved()
        )
    }

    fn is_reserved(&self) -> bool {
        u32::from(*self) & 0xf0000000 == 0xf0000000
    }
}

pub trait Ipv4PacketExt {
    fn new_udp<T>(
        src_addr: Ipv4Addr,
        dst_addr: Ipv4Addr,
        hop_limit: u8,
        udp: &UdpPacket<T>,
    ) -> Ipv4Packet<Bytes>
    where
        T: AsRef<[u8]>;
}

impl Ipv4PacketExt for Ipv4Packet<Bytes> {
    fn new_udp<T>(
        src_addr: Ipv4Addr,
        dst_addr: Ipv4Addr,
        hop_limit: u8,
        udp: &UdpPacket<T>,
    ) -> Ipv4Packet<Bytes>
    where
        T: AsRef<[u8]>
    {
        let bytes = udp.as_ref();

        let packet_repr = Ipv4Repr {
            src_addr: src_addr.into(),
            dst_addr: dst_addr.into(),
            protocol: IpProtocol::Udp,
            hop_limit: hop_limit,
            payload_len: bytes.len(),
        };
        
        let len = bytes.len() + packet_repr.buffer_len();
        let mut packet = Ipv4Packet::new(util::bytes_mut_zeroed(len));
        packet_repr.emit(&mut packet, &ChecksumCapabilities::ignored());
        packet.payload_mut().clone_from_slice(bytes);
        packet.fill_checksum();
        Ipv4Packet::new(packet.into_inner().freeze())
    }
}

