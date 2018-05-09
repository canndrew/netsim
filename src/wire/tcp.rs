use priv_prelude::*;
use super::*;

/// A TCP packet
#[derive(Clone, PartialEq)]
pub struct TcpPacket {
    buffer: Bytes,
}

impl fmt::Debug for TcpPacket {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {

        struct Kind(pub u8);
        impl fmt::Debug for Kind {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                let Kind(b) = *self;
                let s = match b & 0x17 {
                    0x00    => "-",
                    0x01    => "FIN",
                    0x02    => "SYN",
                    0x03    => "SYN | FIN",
                    0x04    => "RST",
                    0x05    => "RST | FIN",
                    0x06    => "RST | SYN",
                    0x07    => "RST | SYN | FIN",
                    0x10    => "ACK",
                    0x11    => "ACK | FIN",
                    0x12    => "ACK | SYN",
                    0x13    => "ACK | SYN | FIN",
                    0x14    => "ACK | RST",
                    0x15    => "ACK | RST | FIN",
                    0x16    => "ACK | RST | SYN",
                    0x17    => "ACK | RST | SYN | FIN",
                    _ => unreachable!(),
                };
                write!(f, "{}", s)
            }
        }

        let payload = self.payload();

        f
        .debug_struct("TcpPacket")
        .field("source_port", &self.source_port())
        .field("dest_port", &self.dest_port())
        .field("seq_num", &self.seq_num())
        .field("ack_num", &self.ack_num())
        .field("window_size", &self.window_size())
        .field("kind", &Kind(self.buffer[13]))
        .field("payload", &payload)
        .finish()
    }
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
    /// Is this a SYN packet?
    pub syn: bool,
    /// Is this an ACK packet?
    pub ack: bool,
    /// Is this a FIN packet?
    pub fin: bool,
    /// Is this an RST packet?
    pub rst: bool,
}

impl TcpFields {
    /// Get the length of the header described by this `TcpFields`
    pub fn header_len(&self) -> usize {
        20
    }
}

fn set_fields(buffer: &mut [u8], fields: TcpFields) {
    NetworkEndian::write_u16(&mut buffer[0..2], fields.source_port);
    NetworkEndian::write_u16(&mut buffer[2..4], fields.dest_port);
    NetworkEndian::write_u32(&mut buffer[4..8], fields.seq_num);
    NetworkEndian::write_u32(&mut buffer[8..12], fields.ack_num);
    buffer[12] = 0x50;
    buffer[13] = {
        (if fields.syn { 0x02 } else { 0 }) |
        (if fields.ack { 0x10 } else { 0 }) |
        (if fields.fin { 0x01 } else { 0 }) |
        (if fields.rst { 0x04 } else { 0 })
    };
    NetworkEndian::write_u16(&mut buffer[14..16], fields.window_size);
    NetworkEndian::write_u16(&mut buffer[16..18], 0);
    NetworkEndian::write_u16(&mut buffer[18..20], 0);
}

fn set_fields_v4(buffer: &mut [u8], fields: TcpFields, source_ip: Ipv4Addr, dest_ip: Ipv4Addr) {
    set_fields(buffer, fields);

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

fn set_fields_v6(buffer: &mut [u8], fields: TcpFields, source_ip: Ipv6Addr, dest_ip: Ipv6Addr) {
    set_fields(buffer, fields);

    let checksum = !checksum::combine(&[
        checksum::pseudo_header_ipv6(
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

    /// Allocate a new TCP packet from the given fields and payload
    pub fn new_from_fields_v6(
        fields: TcpFields,
        source_ip: Ipv6Addr,
        dest_ip: Ipv6Addr,
        payload: Bytes,
    ) -> TcpPacket {
        let len = 20 + payload.len();
        let mut buffer = unsafe { BytesMut::uninit(len) };
        TcpPacket::write_to_buffer_v6(&mut buffer, fields, source_ip, dest_ip, payload);
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

    /// Write a TCP packet to the given empty buffer. The buffer must have the exact correct size.
    pub fn write_to_buffer_v6(
        buffer: &mut [u8],
        fields: TcpFields,
        source_ip: Ipv6Addr,
        dest_ip: Ipv6Addr,
        payload: Bytes,
    ) {
        buffer[20..].clone_from_slice(&payload);
        set_fields_v6(buffer, fields, source_ip, dest_ip);
    }

    /// Get the fields of this packet
    pub fn fields(&self) -> TcpFields {
        TcpFields {
            source_port: self.source_port(),
            dest_port: self.dest_port(),
            seq_num: self.seq_num(),
            ack_num: self.ack_num(),
            window_size: self.window_size(),
            syn: self.is_syn(),
            ack: self.is_ack(),
            fin: self.is_fin(),
            rst: self.is_rst(),
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

    /// Set the header fields of a TCP packet
    pub fn set_fields_v6(&mut self, fields: TcpFields, source_ip: Ipv6Addr, dest_ip: Ipv6Addr) {
        let buffer = mem::replace(&mut self.buffer, Bytes::new());
        let mut buffer = BytesMut::from(buffer);
        set_fields_v6(&mut buffer, fields, source_ip, dest_ip);
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

    /// Check whether this is a SYN packet
    pub fn is_syn(&self) -> bool {
        self.buffer[13] & 0x02 != 0
    }

    /// Check whether this is an ACK packet
    pub fn is_ack(&self) -> bool {
        self.buffer[13] & 0x10 != 0
    }

    /// Check whether this is a FIN packet
    pub fn is_fin(&self) -> bool {
        self.buffer[13] & 0x01 != 0
    }

    /// Check whether this is an RST packet
    pub fn is_rst(&self) -> bool {
        self.buffer[13] & 0x04 != 0
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

    /// Verify the checksum of the packet. The source/destination IP addresses of the packet are
    /// needed to calculate the checksum.
    pub fn verify_checksum_v6(
        &self,
        source_ip: Ipv6Addr,
        dest_ip: Ipv6Addr,
    ) -> bool {
        let len = self.buffer.len();
        !0 == checksum::combine(&[
            checksum::pseudo_header_ipv6(source_ip, dest_ip, 17, len as u32),
            checksum::data(&self.buffer[..]),
        ])
    }
}

