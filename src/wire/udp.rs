use priv_prelude::*;
use super::*;

/// A UDP packet
#[derive(Clone, PartialEq)]
pub struct UdpPacket {
    buffer: Bytes,
}

impl fmt::Debug for UdpPacket {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f
        .debug_struct("UdpPacket")
        .field("source_port", &self.source_port())
        .field("dest_port", &self.dest_port())
        .field("payload", &self.payload())
        .finish()
    }
}

/// Represents the header fields of a UDP packet.
#[derive(Debug, Clone, Copy)]
pub struct UdpFields {
    /// The source port
    pub source_port: u16,
    /// The destination port
    pub dest_port: u16,
}

fn set_fields(buffer: &mut [u8], fields: UdpFields) {
    NetworkEndian::write_u16(&mut buffer[0..2], fields.source_port);
    NetworkEndian::write_u16(&mut buffer[2..4], fields.dest_port);
    let len = buffer.len() as u16;
    NetworkEndian::write_u16(&mut buffer[4..6], len);
    NetworkEndian::write_u16(&mut buffer[6..8], 0);
}

fn set_fields_v4(buffer: &mut [u8], fields: UdpFields, source_ip: Ipv4Addr, dest_ip: Ipv4Addr) {
    set_fields(buffer, fields);

    let checksum = !checksum::combine(&[
        checksum::pseudo_header_ipv4(
            source_ip,
            dest_ip,
            17,
            buffer.len() as u32,
        ),
        checksum::data(&buffer[..]),
    ]);
    let checksum = if checksum == 0x0000 { 0xffff } else { checksum };
    NetworkEndian::write_u16(&mut buffer[6..8], checksum);
}

fn set_fields_v6(buffer: &mut [u8], fields: UdpFields, source_ip: Ipv6Addr, dest_ip: Ipv6Addr) {
    set_fields(buffer, fields);

    let checksum = !checksum::combine(&[
        checksum::pseudo_header_ipv6(
            source_ip,
            dest_ip,
            17,
            buffer.len() as u32,
        ),
        checksum::data(&buffer[..]),
    ]);
    let checksum = if checksum == 0x0000 { 0xffff } else { checksum };
    NetworkEndian::write_u16(&mut buffer[6..8], checksum);
}

impl UdpPacket {
    /// Create a new UDP packet from the given header fields and payload.
    pub fn new_from_fields_v4(
        fields: UdpFields,
        source_ip: Ipv4Addr,
        dest_ip: Ipv4Addr,
        payload: Bytes,
    ) -> UdpPacket {
        let len = 8 + payload.len();
        let mut buffer = unsafe { BytesMut::uninit(len) };
        UdpPacket::write_to_buffer_v4(&mut buffer, fields, source_ip, dest_ip, payload);
        UdpPacket {
            buffer: buffer.freeze(),
        }
    }

    /// Create a new UDP packet from the given header fields and payload.
    pub fn new_from_fields_v6(
        fields: UdpFields,
        source_ip: Ipv6Addr,
        dest_ip: Ipv6Addr,
        payload: Bytes,
    ) -> UdpPacket {
        let len = 8 + payload.len();
        let mut buffer = unsafe { BytesMut::uninit(len) };
        UdpPacket::write_to_buffer_v6(&mut buffer, fields, source_ip, dest_ip, payload);
        UdpPacket {
            buffer: buffer.freeze(),
        }
    }

    /// Write a UDP packet to the given empty buffer.
    pub fn write_to_buffer_v4(
        buffer: &mut [u8],
        fields: UdpFields,
        source_ip: Ipv4Addr,
        dest_ip: Ipv4Addr,
        payload: Bytes,
    ) {
        buffer[8..].clone_from_slice(&payload);
        set_fields_v4(buffer, fields, source_ip, dest_ip);
    }

    /// Write a UDP packet to the given empty buffer.
    pub fn write_to_buffer_v6(
        buffer: &mut [u8],
        fields: UdpFields,
        source_ip: Ipv6Addr,
        dest_ip: Ipv6Addr,
        payload: Bytes,
    ) {
        buffer[8..].clone_from_slice(&payload);
        set_fields_v6(buffer, fields, source_ip, dest_ip);
    }

    /// Get the UDP header fields
    pub fn fields(&self) -> UdpFields {
        UdpFields {
            source_port: self.source_port(),
            dest_port: self.dest_port(),
        }
    }

    /// Parse a UDP packet from the given buffer.
    pub fn from_bytes(buffer: Bytes) -> UdpPacket {
        UdpPacket {
            buffer,
        }
    }

    /// Set the header fields of this UDP packet.
    pub fn set_fields_v4(&mut self, fields: UdpFields, source_ip: Ipv4Addr, dest_ip: Ipv4Addr) {
        let buffer = mem::replace(&mut self.buffer, Bytes::new());
        let mut buffer = BytesMut::from(buffer);
        set_fields_v4(&mut buffer, fields, source_ip, dest_ip);
        self.buffer = buffer.freeze();
    }

    /// Set the header fields of this UDP packet.
    pub fn set_fields_v6(&mut self, fields: UdpFields, source_ip: Ipv6Addr, dest_ip: Ipv6Addr) {
        let buffer = mem::replace(&mut self.buffer, Bytes::new());
        let mut buffer = BytesMut::from(buffer);
        set_fields_v6(&mut buffer, fields, source_ip, dest_ip);
        self.buffer = buffer.freeze();
    }

    /// Get the packet's source port.
    pub fn source_port(&self) -> u16 {
        NetworkEndian::read_u16(&self.buffer[0..2])
    }

    /// Get the packet's destination port.
    pub fn dest_port(&self) -> u16 {
        NetworkEndian::read_u16(&self.buffer[2..4])
    }

    /// Get the packet's payload data.
    pub fn payload(&self) -> Bytes {
        self.buffer.slice_from(8)
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
        let len = NetworkEndian::read_u16(&self.buffer[4..6]);
        !0 == checksum::combine(&[
            checksum::pseudo_header_ipv4(source_ip, dest_ip, 17, u32::from(len)),
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
        let len = NetworkEndian::read_u16(&self.buffer[4..6]);
        !0 == checksum::combine(&[
            checksum::pseudo_header_ipv6(source_ip, dest_ip, 17, u32::from(len)),
            checksum::data(&self.buffer[..]),
        ])
    }
}

