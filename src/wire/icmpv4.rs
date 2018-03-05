use priv_prelude::*;
use super::*;

/// An ICMPv4 packet
#[derive(Clone, PartialEq)]
pub struct Icmpv4Packet {
    buffer: Bytes,
}

impl fmt::Debug for Icmpv4Packet {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Icmpv4Packet::")?;
        self.kind().fmt(f)
    }
}

/// Description of an ICMPv4 packet
#[derive(Debug, Clone)]
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
    /// TTL expired error for a TCP packet
    TtlExpiredTcp {
        /// The IPv4 header of the TCP packet which expired
        ipv4_header: Ipv4Fields,
        /// The source port of the TCP packet which expired
        source_port: u16,
        /// The destination port of the TCP packet which expired
        dest_port: u16,
        /// The sequence number of the TCP packet which expired
        seq_num: u32,
    },
    /// TTL expired error for a UDP packet
    TtlExpiredUdp {
        /// The IPv4 header of the UDP packet which expired
        ipv4_header: Ipv4Fields,
        /// The source port of the UDP packet which expired
        source_port: u16,
        /// The destination port of the UDP packet which expired
        dest_port: u16,
        /// The length field of the UDP packet which expired
        len: u16,
        /// The checksum of the UDP packet which expired
        checksum: u16,
    },
}

fn set(
    buffer: &mut [u8],
    kind: Icmpv4PacketKind,
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
        Icmpv4PacketKind::TtlExpiredTcp { ipv4_header, source_port, dest_port, seq_num } => {
            buffer[0] = 11;
            buffer[1] = 0;
            buffer[4..8].clone_from_slice(&[0x00, 0x00, 0x00, 0x00]);
            buffer[17] = 6;
            let header_end = buffer.len() - 8;
            ipv4::set_fields(&mut buffer[8..header_end], ipv4_header);
            NetworkEndian::write_u16(&mut buffer[header_end..(header_end + 2)], source_port);
            NetworkEndian::write_u16(&mut buffer[(header_end + 2)..(header_end + 4)], dest_port);
            NetworkEndian::write_u32(&mut buffer[(header_end + 4)..(header_end + 8)], seq_num);
        },
        Icmpv4PacketKind::TtlExpiredUdp { ipv4_header, source_port, dest_port, len, checksum } => {
            buffer[0] = 11;
            buffer[1] = 0;
            buffer[4..8].clone_from_slice(&[0x00, 0x00, 0x00, 0x00]);
            buffer[17] = 17;
            let header_end = buffer.len() - 8;
            ipv4::set_fields(&mut buffer[8..header_end], ipv4_header);
            NetworkEndian::write_u16(&mut buffer[header_end..(header_end + 2)], source_port);
            NetworkEndian::write_u16(&mut buffer[(header_end + 2)..(header_end + 4)], dest_port);
            NetworkEndian::write_u16(&mut buffer[(header_end + 4)..(header_end + 6)], len);
            NetworkEndian::write_u16(&mut buffer[(header_end + 6)..(header_end + 8)], checksum);
        },
    }
    
    let checksum = !checksum::data(&buffer);
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
            Icmpv4PacketKind::TtlExpiredTcp { .. } |
            Icmpv4PacketKind::TtlExpiredUdp { .. } => 36,
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
    ) -> Icmpv4Packet {
        let len = kind.buffer_len();
        let mut buffer = unsafe { BytesMut::uninit(len) };
        Icmpv4Packet::write_to_buffer(&mut buffer, kind);
        Icmpv4Packet {
            buffer: buffer.freeze(),
        }
    }

    /// Write the ICMP data to the given buffer.
    pub fn write_to_buffer(
        buffer: &mut [u8],
        kind: Icmpv4PacketKind,
    ) {
        set(buffer, kind);
    }

    /// Return a parsed version of the ICMP packet
    pub fn kind(&self) -> Icmpv4PacketKind {
        match (self.buffer[0], self.buffer[1]) {
            (0, 0) => {
                let id = NetworkEndian::read_u16(&self.buffer[4..6]);
                let seq_num = NetworkEndian::read_u16(&self.buffer[6..8]);
                let payload = self.buffer.slice_from(8);
                Icmpv4PacketKind::EchoReply {
                    id, seq_num, payload,
                }
            },
            (8, 0) => {
                let id = NetworkEndian::read_u16(&self.buffer[4..6]);
                let seq_num = NetworkEndian::read_u16(&self.buffer[6..8]);
                let payload = self.buffer.slice_from(8);
                Icmpv4PacketKind::EchoRequest {
                    id, seq_num, payload,
                }
            },
            (11, 0) if self.buffer[17] == 6 => {
                let ipv4_header = Ipv4Fields::from_bytes(&self.buffer[8..]);
                let source_port = NetworkEndian::read_u16(&self.buffer[28..30]);
                let dest_port = NetworkEndian::read_u16(&self.buffer[30..32]);
                let seq_num = NetworkEndian::read_u32(&self.buffer[32..36]);
                Icmpv4PacketKind::TtlExpiredTcp {
                    ipv4_header, source_port, dest_port, seq_num,
                }
            },
            (11, 0) if self.buffer[17] == 17 => {
                let ipv4_header = Ipv4Fields::from_bytes(&self.buffer[8..]);
                let source_port = NetworkEndian::read_u16(&self.buffer[28..30]);
                let dest_port = NetworkEndian::read_u16(&self.buffer[30..32]);
                let len = NetworkEndian::read_u16(&self.buffer[30..32]);
                let checksum = NetworkEndian::read_u16(&self.buffer[32..34]);
                Icmpv4PacketKind::TtlExpiredUdp {
                    ipv4_header, source_port, dest_port, len, checksum,
                }
            },
            (ty, code) => {
                let mut rest_of_header = [0u8; 4];
                rest_of_header.clone_from_slice(&self.buffer[4..8]);
                let payload = self.buffer.slice_from(8);
                Icmpv4PacketKind::Unknown {
                    ty, code, rest_of_header, payload,
                }
            },
        }
    }

    /// Get the underlying byte buffer of this packet
    pub fn as_bytes(&self) -> &Bytes {
        &self.buffer
    }

    /// Returns `true` if this ICMP packet has a valid checksum
    pub fn verify_checksum(&self) -> bool {
        checksum::data(&self.buffer) == !0
    }
}

