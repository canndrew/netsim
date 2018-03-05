use priv_prelude::*;
use super::*;

/// An ICMPv4 packet
#[derive(Clone, PartialEq)]
pub struct Icmpv4Packet {
    buffer: Bytes,
}

/// Description of an ICMPv4 packet
pub enum Icmpv4PacketKind {
    /// Unknown ICMP packet type/code.
    Unknown {
        /// Message type
        ty: u8,
        /// Message code
        code: u8,
        /// Type/code dependent, final 4 bytes of message header.
        rest_of_header: [u8; 4],
        /// Message payload.
        payload: Bytes,
    },
    /// ICMP echo request. eg. ping
    EchoRequest {
        /// Ping ID
        id: u16,
        /// Ping sequence number
        seq_num: u16,
        /// Ping payload
        payload: Bytes,
    },
    /// ICMP echo reply.
    EchoReply {
        /// Ping ID
        id: u16,
        /// Ping sequence number
        seq_num: u16,
        /// Ping payload
        payload: Bytes,
    },
    /*
    TtlExpiredTcp {
        ipv4_header: Ipv4Fields,
        source_port: u16,
        dest_port: u16,
        seq_num: u32,
    },
    TtlExpiredUdp {
        ipv4_header: Ipv4Fields,
        source_port: u16,
        dest_port: u16,
        len: u16,
    },
    */
}

fn set(
    buffer: &mut [u8],
    kind: Icmpv4PacketKind,
    source_ip: Ipv4Addr,
    dest_ip: Ipv4Addr,
) {
    NetworkEndian::write_u16(&mut buffer[2..4], 0);
    match kind {
        Icmpv4PacketKind::Unknown { ty, code, rest_of_header, payload } => {
            buffer[0] = ty;
            buffer[1] = code;
            buffer[4..8].clone_from_slice(&rest_of_header);
            buffer[8..].clone_from_slice(&payload);
        },
        Icmpv4PacketKind::EchoRequest { id, seq_num, payload } => {
            buffer[0] = 8;
            buffer[1] = 0;
            NetworkEndian::write_u16(&mut buffer[4..6], id);
            NetworkEndian::write_u16(&mut buffer[6..8], seq_num);
            buffer[8..].clone_from_slice(&payload);
        },
        Icmpv4PacketKind::EchoReply { id, seq_num, payload } => {
            buffer[0] = 0;
            buffer[1] = 0;
            NetworkEndian::write_u16(&mut buffer[4..6], id);
            NetworkEndian::write_u16(&mut buffer[6..8], seq_num);
            buffer[8..].clone_from_slice(&payload);
        },
        /*
        Icmpv4PacketKind::TtlExpiredTcp { ipv4_header, source_port, dest_port, seq_num } => {
            buffer[0] = 11;
            buffer[1] = 0;
            buffer[4..8].clone_from_slice(&[0x00, 0x00, 0x00, 0x00]);
            let header_end = buffer.len() - 8;
            ipv4::set_fields(&mut buffer[8..header_end], ipv4_header);
            NetworkEndian::write_u16(buffer[header_end..(header_end + 2)], 
        },
        */
    }
    
    let checksum = !checksum::combine(&[
        checksum::pseudo_header_ipv4(
            source_ip,
            dest_ip,
            1,
            buffer.len() as u32,
        ),
        checksum::data(&buffer[..]),
    ]);
    NetworkEndian::write_u16(&mut buffer[2..4], checksum);
}

impl Icmpv4PacketKind {
    /// Get the length of the buffer required to store the ICMP header and payload
    pub fn buffer_len(&self) -> usize {
        match *self {
            Icmpv4PacketKind::Unknown { ref payload, .. } |
            Icmpv4PacketKind::EchoRequest { ref payload, .. } |
            Icmpv4PacketKind::EchoReply { ref payload, .. } => {
                payload.len() + 8
            }
        }
    }
}

impl Icmpv4Packet {
    /// Parse an `Icmpv4Packet` from a byte buffer.
    pub fn from_bytes(buffer: Bytes) -> Icmpv4Packet {
        Icmpv4Packet {
            buffer,
        }
    }

    /// Allocate a new `Icmpv4Packet` from the given `Icmpv4PacketKind`. The source and destination
    /// IP addresses are needed for calculating the packet checksum.
    pub fn new_from_kind(
        kind: Icmpv4PacketKind,
        source_ip: Ipv4Addr,
        dest_ip: Ipv4Addr,
    ) -> Icmpv4Packet {
        let len = kind.buffer_len();
        let mut buffer = unsafe { BytesMut::uninit(len) };
        set(&mut buffer, kind, source_ip, dest_ip);
        Icmpv4Packet {
            buffer: buffer.freeze(),
        }
    }
}

