use priv_prelude::*;
use super::*;

/// A TCP packet
#[derive(Clone, PartialEq)]
pub struct TcpPacket {
    buffer: Bytes,
}

#[derive(Clone, Debug, PartialEq)]
/// A selective ACK block of a TCP packet
pub struct SelectiveAck {
    /// The initial sequence number that this SACK block is acknowledging
    pub start_seq_num: u32,
    /// The final sequence number that this SACK block is acknowledging
    pub end_seq_num: u32,
}

impl fmt::Debug for TcpPacket {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {

        struct Kind(pub u16);
        impl fmt::Debug for Kind {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                let Kind(b) = *self;

                let mut written = false;
                for &(mask, name) in &[
                    (0x0010, "FIN"),
                    (0x0002, "SYN"),
                    (0x0004, "RST"),
                    (0x0008, "PSH"),
                    (0x0010, "ACK"),
                    (0x0040, "ECE"),
                    (0x0080, "CWR"),
                    (0x0100, "NS"),
                ] {
                    if mask & b != 0 {
                        if written {
                            write!(f, " | {}", name)?;
                        } else {
                            write!(f, "{}", name)?;
                            written = true;
                        }
                    }
                }

                if !written {
                    write!(f, "-")?;
                }

                Ok(())
            }
        }

        let payload = self.payload();
        let kind = Kind(NetworkEndian::read_u16(&self.buffer[12..14]) & 0x01df);

        let mut ds = f.debug_struct("TcpPacket");

        ds
        .field("source_port", &self.source_port())
        .field("dest_port", &self.dest_port())
        .field("seq_num", &self.seq_num())
        .field("ack_num", &self.ack_num())
        .field("window_size", &self.window_size())
        .field("kind", &kind);
        if let Some(ptr) = self.urgent() {
            ds.field("urgent", &ptr);
        }
        if let Some(mss) = self.mss() {
            ds.field("mss", &mss);
        }
        if let Some(window_scale) = self.window_scale() {
            ds.field("window_scale", &window_scale);
        }
        if self.selective_ack_permitted() {
            ds.field("selective_ack_permitted", &());
        }

        let selective_acks = self.selective_acks();
        if let Some(selective_acks) = selective_acks {
            ds.field("selective_acks", &&selective_acks[..]);
        }
        if let Some((timestamp, echo_timestamp)) = self.timestamps() {
            ds.field("timestamp", &timestamp);
            ds.field("echo_timestamp", &echo_timestamp);
        }

        ds
        .field("payload", &payload)
        .finish()
    }
}

/// The fields of a TCP header
#[derive(Debug, Clone)]
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
    /// Is the NS flag set?
    pub ns: bool,
    /// Is the CWR flag set?
    pub cwr: bool,
    /// Is the ECE flag set?
    pub ece: bool,
    /// Is the PSH flag set?
    pub psh: bool,
    /// The urgent field
    pub urgent: Option<u16>,
    /// The maximum segment size
    pub mss: Option<u16>,
    /// The window scale
    pub window_scale: Option<u8>,
    /// Extension field indicating whether selective acks are permitted
    pub selective_ack_permitted: bool,
    /// Selective ack blocks
    pub selective_acks: Option<Vec<SelectiveAck>>,
    /// Packet timestamps, as `(timestamp, echo_timestamp)`
    pub timestamps: Option<(u32, u32)>,
}

impl TcpFields {
    /// Get the length of the header described by this `TcpFields`
    pub fn header_len(&self) -> usize {
        (if self.mss.is_some() { 4 } else { 0 }) +
        (if self.window_scale.is_some() { 3 } else { 0 }) +
        (if self.selective_ack_permitted { 2 } else { 0 }) +
        (if let Some(ref selective_acks) = self.selective_acks {
            2 + selective_acks.len() * 8
        } else {
            0
        }) +
        (if self.timestamps.is_some() { 10 } else { 0 }) +
        20
    }
}

fn set_fields(buffer: &mut [u8], fields: TcpFields) {
    NetworkEndian::write_u16(&mut buffer[0..2], fields.source_port);
    NetworkEndian::write_u16(&mut buffer[2..4], fields.dest_port);
    NetworkEndian::write_u32(&mut buffer[4..8], fields.seq_num);
    NetworkEndian::write_u32(&mut buffer[8..12], fields.ack_num);
    buffer[12] = {
        (((fields.header_len() / 4) as u8) << 4) |
        if fields.ns { 0x01 } else { 0 }
    };
    buffer[13] = {
        (if fields.fin { 0x01 } else { 0 }) |
        (if fields.syn { 0x02 } else { 0 }) |
        (if fields.rst { 0x04 } else { 0 }) |
        (if fields.psh { 0x08 } else { 0 }) |
        (if fields.ack { 0x10 } else { 0 }) |
        (if fields.urgent.is_some() { 0x20 } else { 0 }) |
        (if fields.ece { 0x40 } else { 0 }) |
        (if fields.cwr { 0x80 } else { 0 })
    };
    NetworkEndian::write_u16(&mut buffer[14..16], fields.window_size);
    NetworkEndian::write_u16(&mut buffer[16..18], 0);
    NetworkEndian::write_u16(&mut buffer[18..20], fields.urgent.unwrap_or(0));

    let mut pos = 20;
    if let Some(mss) = fields.mss {
        buffer[pos] = 2;
        buffer[pos + 1] = 4;
        NetworkEndian::write_u16(&mut buffer[(pos + 2)..(pos + 4)], mss);
        pos += 4;
    }

    if let Some(window_scale) = fields.window_scale {
        buffer[pos] = 3;
        buffer[pos + 1] = 3;
        buffer[pos + 2] = window_scale;
        pos += 3;
    }

    if fields.selective_ack_permitted {
        buffer[pos] = 4;
        buffer[pos + 1] = 2;
        pos += 2;
    }

    if let Some(ref selective_acks) = fields.selective_acks {
        buffer[pos] = 5;
        buffer[pos + 1] = 2 + (selective_acks.len() * 8) as u8;
        pos += 2;
        for ack in selective_acks {
            NetworkEndian::write_u32(&mut buffer[pos..(pos + 4)], ack.start_seq_num);
            NetworkEndian::write_u32(&mut buffer[(pos + 4)..(pos + 8)], ack.end_seq_num);
            pos += 8;
        }
    }

    if let Some((timestamp, echo_timestamp)) = fields.timestamps {
        buffer[pos] = 8;
        buffer[pos + 1] = 10;
        NetworkEndian::write_u32(&mut buffer[(pos + 2)..(pos + 6)], timestamp);
        NetworkEndian::write_u32(&mut buffer[(pos + 6)..(pos + 10)], echo_timestamp);
        pos += 10;
    }

    while pos < buffer.len() {
        buffer[pos] = 0;
    }
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
        let len = fields.header_len() + payload.len();
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
        let len = fields.header_len() + payload.len();
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
        let start = fields.header_len();
        buffer[start..].clone_from_slice(&payload);
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
        let start = fields.header_len();
        buffer[start..].clone_from_slice(&payload);
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
            ns: self.is_ns(),
            cwr: self.is_cwr(),
            ece: self.is_ece(),
            psh: self.is_psh(),
            urgent: self.urgent(),
            mss: self.mss(),
            window_scale: self.window_scale(),
            selective_ack_permitted: self.selective_ack_permitted(),
            selective_acks: self.selective_acks(),
            timestamps: self.timestamps(),
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

    /// Get the *unscaled* window size of the packet in window size units.
    pub fn window_size(&self) -> u16 {
        NetworkEndian::read_u16(&self.buffer[14..16])
    }

    /// Get the value of the window scale field, if it is present.
    pub fn window_scale(&self) -> Option<u8> {
        let mut pos = 20;
        while pos < self.header_len() {
            match self.buffer[pos] {
                0 | 1 => pos += 1,
                3 => {
                    return Some(self.buffer[pos + 2]);
                }
                _ => pos += self.buffer[pos + 1] as usize,
            }
        }
        None
    }

    /// Check whether the selective-ack-permitted option is present
    pub fn selective_ack_permitted(&self) -> bool {
        let mut pos = 20;
        while pos < self.header_len() {
            match self.buffer[pos] {
                0 | 1 => pos += 1,
                4 => return true,
                _ => pos += self.buffer[pos + 1] as usize,
            }
        }
        false
    }

    /// Get the selective ACK blocks stored in the options field of this packet.
    pub fn selective_acks(&self) -> Option<Vec<SelectiveAck>> {
        let mut pos = 20;
        while pos < self.header_len() {
            match self.buffer[pos] {
                0 | 1 => pos += 1,
                5 => {
                    let mut ret = Vec::new();
                    let num_sacks = (self.buffer[pos + 1] - 2) / 4;
                    pos += 2;
                    for _ in 0..num_sacks {
                        let start = NetworkEndian::read_u32(&self.buffer[pos..(pos + 4)]);
                        let end = NetworkEndian::read_u32(&self.buffer[(pos + 4)..(pos + 8)]);
                        ret.push(SelectiveAck {
                            start_seq_num: start,
                            end_seq_num: end,
                        });
                        pos += 8;
                    }
                    return Some(ret);
                },
                _ => pos += self.buffer[pos + 1] as usize,
            }
        }
        None
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

    /// Check whether the packet's NS flag is set
    pub fn is_ns(&self) -> bool {
        self.buffer[12] & 0x01 != 0
    }

    /// Check whether the packet's CWR flag is set
    pub fn is_cwr(&self) -> bool {
        self.buffer[13] & 0x80 != 0
    }

    /// Check whether the packet's ECE flag is set
    pub fn is_ece(&self) -> bool {
        self.buffer[13] & 0x40 != 0
    }

    /// Get the packet's urgent pointer, if URG is set
    pub fn urgent(&self) -> Option<u16> {
        if self.buffer[13] & 0x20 != 0 {
            Some(NetworkEndian::read_u16(&self.buffer[18..20]))
        } else {
            None
        }
    }

    /// Check whether the packet's PSH flag is set
    pub fn is_psh(&self) -> bool {
        self.buffer[13] & 0x80 != 0
    }

    /// Get the maximum segment size field (if it is present)
    pub fn mss(&self) -> Option<u16> {
        let mut pos = 20;
        while pos < self.header_len() {
            match self.buffer[pos] {
                0 | 1 => pos += 1,
                2 => {
                    let mss = NetworkEndian::read_u16(&self.buffer[(pos + 2)..(pos + 4)]);
                    return Some(mss);
                }
                _ => pos += self.buffer[pos + 1] as usize,
            }
        }
        None
    }

    /// Returns the `(timestamp, echo_timestamp)` of the packet, if present.
    pub fn timestamps(&self) -> Option<(u32, u32)> {
        let mut pos = 20;
        while pos < self.header_len() {
            match self.buffer[pos] {
                0 | 1 => pos += 1,
                8 => {
                    let t = NetworkEndian::read_u32(&self.buffer[(pos + 2)..(pos + 6)]);
                    let e = NetworkEndian::read_u32(&self.buffer[(pos + 6)..(pos + 10)]);
                    return Some((t, e));
                }
                _ => pos += self.buffer[pos + 1] as usize,
            }
        }
        None
    }

    /// Get the length of the TCP header, in bytes
    pub fn header_len(&self) -> usize {
        4 * (self.buffer[12] >> 4) as usize
    }

    /// Get the packet's payload data
    pub fn payload(&self) -> Bytes {
        let header_len = self.header_len();
        self.buffer.slice_from(header_len)
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

