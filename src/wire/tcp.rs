use priv_prelude::*;
use super::*;

/// A TCP packet
#[derive(Clone, PartialEq)]
pub struct TcpPacket {
    buffer: Bytes,
}

impl fmt::Debug for TcpPacket {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let payload = self.payload();

        f
        .debug_struct("TcpPacket")
        .field("source_port", &self.source_port())
        .field("dest_port", &self.dest_port())
        .field("seq_num", &self.seq_num())
        .field("ack_num", &self.ack_num())
        .field("window_size", &self.window_size())
        .field("kind", &self.kind())
        .field("payload", &payload)
        .finish()
    }
}

/// The flags of a TCP packet header
#[derive(Debug, Clone, Copy)]
pub enum TcpPacketKind {
    /// A SYN packet
    Syn,
    /// An ACK packet
    Ack,
    /// A FIN packet
    Fin,
    /// An RST packet
    Rst,
}

/// The port fields of a TCP packet header. Also includes IP addresses as these are necessary for
/// calculating/verifying TCP header checksums.
#[derive(Debug, Clone, Copy)]
pub enum TcpAddrs {
    /// IPv4
    V4 {
        /// The source address of the packet
        source_addr: SocketAddrV4,
        /// The destination address of the packet
        dest_addr: SocketAddrV4,
    },
    /// IPv6
    V6 {
        /// The source address of the packet
        source_addr: SocketAddrV6,
        /// The destination address of the packet
        dest_addr: SocketAddrV6,
    },
}

/// The fields of a TCP header
#[derive(Debug, Clone, Copy)]
pub struct TcpFields {
    /// The sequence number
    pub seq_num: u32,
    /// The ACK number
    pub ack_num: u32,
    /// The window size
    pub window_size: u16,
    /// The kind of packet, as specified by the control flags
    pub kind: TcpPacketKind,
    /// The source/destination ports of the packet. IP addresses are included since these are
    /// necessary to calculate the checksum.
    pub addrs: TcpAddrs,
}

impl TcpFields {
    /// Get the length of the header described by this `TcpFields`
    pub fn header_len(&self) -> usize {
        20
    }
}

fn set_fields(buffer: &mut [u8], fields: TcpFields) {
    match fields.addrs {
        TcpAddrs::V4 { source_addr, dest_addr } => {
            NetworkEndian::write_u16(&mut buffer[0..2], source_addr.port());
            NetworkEndian::write_u16(&mut buffer[2..4], dest_addr.port());
        },
        TcpAddrs::V6 { source_addr, dest_addr } => {
            NetworkEndian::write_u16(&mut buffer[0..2], source_addr.port());
            NetworkEndian::write_u16(&mut buffer[2..4], dest_addr.port());
        },
    }
    NetworkEndian::write_u32(&mut buffer[4..8], fields.seq_num);
    NetworkEndian::write_u32(&mut buffer[8..12], fields.ack_num);
    buffer[12] = 0;
    buffer[13] = match fields.kind {
        TcpPacketKind::Syn { .. } => 0x02,
        TcpPacketKind::Ack { .. } => 0x10,
        TcpPacketKind::Fin { .. } => 0x01,
        TcpPacketKind::Rst => 0x40,
    };
    NetworkEndian::write_u16(&mut buffer[14..16], fields.window_size);
    NetworkEndian::write_u16(&mut buffer[16..18], 0);
    NetworkEndian::write_u16(&mut buffer[18..20], 0);
    let pseudo_header_checksum = match fields.addrs {
        TcpAddrs::V4 { source_addr, dest_addr } => {
            checksum::pseudo_header_ipv4(
                *source_addr.ip(),
                *dest_addr.ip(),
                6,
                buffer.len() as u32,
            )
        },
        TcpAddrs::V6 { source_addr, dest_addr } => {
            checksum::pseudo_header_ipv6(
                *source_addr.ip(),
                *dest_addr.ip(),
                6,
                buffer.len() as u32,
            )
        },
    };

    let checksum = !checksum::combine(&[
        pseudo_header_checksum,
        checksum::data(&buffer[..]),
    ]);
    NetworkEndian::write_u16(&mut buffer[16..18], checksum);
}

impl TcpPacket {
    /// Allocate a new TCP packet from the given fields and payload
    pub fn new_from_fields(
        fields: TcpFields,
        payload: Bytes,
    ) -> TcpPacket {
        // NOTE: this will break when TCP options are added
        let len = 20 + payload.len();
        let mut buffer = unsafe { BytesMut::uninit(len) };
        TcpPacket::write_to_buffer(&mut buffer, fields, payload);
        TcpPacket {
            buffer: buffer.freeze(),
        }
    }

    /// Write a TCP packet to the given empty buffer. The buffer must have the exact correct size.
    pub fn write_to_buffer(
        buffer: &mut [u8],
        fields: TcpFields,
        payload: Bytes,
    ) {
        // NOTE: this will break when TCP options are added
        buffer[20..].clone_from_slice(&payload);
        set_fields(buffer, fields);
    }

    /// Get the fields of this packet
    pub fn fields_v4(&self, source_ip: Ipv4Addr, dest_ip: Ipv4Addr) -> TcpFields {
        TcpFields {
            seq_num: self.seq_num(),
            ack_num: self.ack_num(),
            window_size: self.window_size(),
            kind: self.kind(),
            addrs: TcpAddrs::V4 {
                source_addr: SocketAddrV4::new(source_ip, self.source_port()),
                dest_addr: SocketAddrV4::new(dest_ip, self.dest_port()),
            },
        }
    }

    /// Parse a TCP packet from a byte buffer
    pub fn from_bytes(buffer: Bytes) -> TcpPacket {
        TcpPacket {
            buffer,
        }
    }

    /// Set the header fields of a TCP packet
    pub fn set_fields(&mut self, fields: TcpFields) {
        let buffer = mem::replace(&mut self.buffer, Bytes::new());
        let mut buffer = BytesMut::from(buffer);
        set_fields(&mut buffer, fields);
        self.buffer = buffer.freeze();
    }

    /// Get the source port of the packet.
    pub fn source_port(&self) -> u16 {
        NetworkEndian::read_u16(&self.buffer[0..2])
    }

    /// Get the destination port of the packet.
    pub fn dest_port(&self) -> u16 {
        NetworkEndian::read_u16(&self.buffer[2..4])
    }

    /// Get the sequence number of the packet
    pub fn seq_num(&self) -> u32 {
        NetworkEndian::read_u32(&self.buffer[4..8])
    }

    /// Get the ack number of the packet
    pub fn ack_num(&self) -> u32 {
        NetworkEndian::read_u32(&self.buffer[8..12])
    }

    /// Get the ack number of the packet
    pub fn window_size(&self) -> u16 {
        NetworkEndian::read_u16(&self.buffer[14..16])
    }

    /// What kind of TCP packet this is, according to the control bits.
    pub fn kind(&self) -> TcpPacketKind {
        match self.buffer[13] & 0x17 {
            0x02 => TcpPacketKind::Syn,
            0x10 => TcpPacketKind::Ack,
            0x11 => TcpPacketKind::Fin,
            0x14 => TcpPacketKind::Rst,
            _ => panic!("invalid tcp header flags"),
        }
    }

    /// Get the packet's payload data
    pub fn payload(&self) -> Bytes {
        // NOTE: this will break when TCP options are added
        self.buffer.slice_from(20)
    }

    /// Get the entire packet as a raw byte buffer.
    pub fn as_bytes(&self) -> &Bytes {
        &self.buffer
    }

    /// Verify the checksum of the packet. The source/destination IP addresses of the packet are
    /// needed to calculate the checksum.
    pub fn verify_checksum_v4(
        &self,
        source_ip: Ipv4Addr,
        dest_ip: Ipv4Addr,
    ) -> bool {
        let len = self.buffer.len();
        !0 == checksum::combine(&[
            checksum::pseudo_header_ipv4(source_ip, dest_ip, 17, len as u32),
            checksum::data(&self.buffer[..]),
        ])
    }
}

