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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TcpPacketKind {
    /// A SYN packet
    Syn,
    /// A SYN-ACK packet
    SynAck,
    /// An ACK packet
    Ack,
    /// A FIN packet
    Fin,
    /// An RST packet
    Rst,
}

/// The fields of a TCP header
#[derive(Debug, Clone, Copy)]
pub struct TcpFields {
    /// The source port
    pub source_port: u16,
    /// The destination port
    pub dest_port: u16,
    /// The sequence number
    pub seq_num: u32,
    /// The ACK number
    pub ack_num: u32,
    /// The window size
    pub window_size: u16,
    /// The kind of packet, as specified by the control flags
    pub kind: TcpPacketKind,
}

impl TcpFields {
    /// Get the length of the header described by this `TcpFields`
    pub fn header_len(&self) -> usize {
        20
    }
}

fn set_fields_v4(buffer: &mut [u8], fields: TcpFields, source_ip: Ipv4Addr, dest_ip: Ipv4Addr) {
    NetworkEndian::write_u16(&mut buffer[0..2], fields.source_port);
    NetworkEndian::write_u16(&mut buffer[2..4], fields.dest_port);
    NetworkEndian::write_u32(&mut buffer[4..8], fields.seq_num);
    NetworkEndian::write_u32(&mut buffer[8..12], fields.ack_num);
    buffer[12] = 0x50;
    buffer[13] = match fields.kind {
        TcpPacketKind::Syn { .. } => 0x02,
        TcpPacketKind::SynAck { .. } => 0x12,
        TcpPacketKind::Ack { .. } => 0x10,
        TcpPacketKind::Fin { .. } => 0x11,
        TcpPacketKind::Rst => 0x40,
    };
    NetworkEndian::write_u16(&mut buffer[14..16], fields.window_size);
    NetworkEndian::write_u16(&mut buffer[16..18], 0);
    NetworkEndian::write_u16(&mut buffer[18..20], 0);

    let checksum = !checksum::combine(&[
        checksum::pseudo_header_ipv4(
            source_ip,
            dest_ip,
            6,
            buffer.len() as u32,
        ),
        checksum::data(&buffer[..]),
    ]);
    NetworkEndian::write_u16(&mut buffer[16..18], checksum);
}

impl TcpPacket {
    /// Allocate a new TCP packet from the given fields and payload
    pub fn new_from_fields_v4(
        fields: TcpFields,
        source_ip: Ipv4Addr,
        dest_ip: Ipv4Addr,
        payload: Bytes,
    ) -> TcpPacket {
        // NOTE: this will break when TCP options are added
        let len = 20 + payload.len();
        let mut buffer = unsafe { BytesMut::uninit(len) };
        TcpPacket::write_to_buffer_v4(&mut buffer, fields, source_ip, dest_ip, payload);
        TcpPacket {
            buffer: buffer.freeze(),
        }
    }

    /// Write a TCP packet to the given empty buffer. The buffer must have the exact correct size.
    pub fn write_to_buffer_v4(
        buffer: &mut [u8],
        fields: TcpFields,
        source_ip: Ipv4Addr,
        dest_ip: Ipv4Addr,
        payload: Bytes,
    ) {
        // NOTE: this will break when TCP options are added
        buffer[20..].clone_from_slice(&payload);
        set_fields_v4(buffer, fields, source_ip, dest_ip);
    }

    /// Get the fields of this packet
    pub fn fields(&self) -> TcpFields {
        TcpFields {
            source_port: self.source_port(),
            dest_port: self.dest_port(),
            seq_num: self.seq_num(),
            ack_num: self.ack_num(),
            window_size: self.window_size(),
            kind: self.kind(),
        }
    }

    /// Parse a TCP packet from a byte buffer
    pub fn from_bytes(buffer: Bytes) -> TcpPacket {
        TcpPacket {
            buffer,
        }
    }

    /// Set the header fields of a TCP packet
    pub fn set_fields_v4(&mut self, fields: TcpFields, source_ip: Ipv4Addr, dest_ip: Ipv4Addr) {
        let buffer = mem::replace(&mut self.buffer, Bytes::new());
        let mut buffer = BytesMut::from(buffer);
        set_fields_v4(&mut buffer, fields, source_ip, dest_ip);
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
            0x12 => TcpPacketKind::SynAck,
            0x10 => TcpPacketKind::Ack,
            0x11 => TcpPacketKind::Fin,
            0x14 => TcpPacketKind::Rst,
            f => panic!("invalid tcp header flags: {:02x}", f),
        }
    }

    /// Get the packet's payload data
    pub fn payload(&self) -> Bytes {
        let data_offset = 4 * (self.buffer[12] >> 4) as usize;
        self.buffer.slice_from(data_offset)
    }

    /// Get the entire packet as a raw byte buffer.
    pub fn as_bytes(&self) -> &Bytes {
        &self.buffer
    }

    /// Consume the packet and return the underlying buffer
    pub fn into_bytes(self) -> Bytes {
        self.buffer
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

