use priv_prelude::*;
use util;

pub trait ArpPacketExt {
    fn new_request(
        src_mac_addr: EthernetAddress,
        src_ipv4_addr: Ipv4Addr,
        dst_ipv4_addr: Ipv4Addr,
    ) -> ArpPacket<Bytes>;

    fn new_reply(
        src_mac_addr: EthernetAddress,
        src_ipv4_addr: Ipv4Addr,
        dst_mac_addr: EthernetAddress,
        dst_ipv4_addr: Ipv4Addr,
    ) -> ArpPacket<Bytes>;
}

impl ArpPacketExt for ArpPacket<Bytes> {
    fn new_request(
        src_mac_addr: EthernetAddress,
        src_ipv4_addr: Ipv4Addr,
        dst_ipv4_addr: Ipv4Addr,
    ) -> ArpPacket<Bytes> {
        let packet_repr = ArpRepr::EthernetIpv4 {
            operation: ArpOperation::Request,
            source_hardware_addr: src_mac_addr,
            source_protocol_addr: src_ipv4_addr.into(),
            target_hardware_addr: EthernetAddress::BROADCAST,
            target_protocol_addr: dst_ipv4_addr.into(),
        };
        let len = packet_repr.buffer_len();
        let mut packet = ArpPacket::new(util::bytes_mut_zeroed(len));
        packet_repr.emit(&mut packet);
        ArpPacket::new(packet.into_inner().freeze())
    }

    fn new_reply(
        src_mac_addr: EthernetAddress,
        src_ipv4_addr: Ipv4Addr,
        dst_mac_addr: EthernetAddress,
        dst_ipv4_addr: Ipv4Addr,
    ) -> ArpPacket<Bytes> {
        let packet_repr = ArpRepr::EthernetIpv4 {
            operation: ArpOperation::Reply,
            source_hardware_addr: src_mac_addr,
            source_protocol_addr: src_ipv4_addr.into(),
            target_hardware_addr: dst_mac_addr,
            target_protocol_addr: dst_ipv4_addr.into(),
        };
        let len = packet_repr.buffer_len();
        let mut packet = ArpPacket::new(util::bytes_mut_zeroed(len));
        packet_repr.emit(&mut packet);
        ArpPacket::new(packet.into_inner().freeze())
    }
}

