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

/// Represents the header fields of a UDP packet. Also includes IP addresses as these are needed to
/// calculate/verify the packet checksum.
#[derive(Debug, Clone)]
pub enum UdpFields {
    /// A UDP packet stored in an Ipv4 packet.
    V4 {
        /// Source IP and port of the packet.
        source_addr: SocketAddrV4,
        /// Destination IP and port of the packet.
        dest_addr: SocketAddrV4,
    },
    /// A UDP packet stored in an Ipv6 packet.
    V6 {
        /// Source IP and port of the packet.
        source_addr: SocketAddrV6,
        /// Destination IP and port of the packet.
        dest_addr: SocketAddrV6,
    },
}

fn set_fields(buffer: &mut [u8], fields: UdpFields) {
    match fields {
        UdpFields::V4 {
            source_addr,
            dest_addr,
        } => {
            NetworkEndian::write_u16(&mut buffer[0..2], source_addr.port());
            NetworkEndian::write_u16(&mut buffer[2..4], dest_addr.port());
            let len = buffer.len() as u16;
            NetworkEndian::write_u16(&mut buffer[4..6], len);
            NetworkEndian::write_u16(&mut buffer[6..8], 0);

            let checksum = !checksum::combine(&[
                checksum::pseudo_header_ipv4(
                    *source_addr.ip(),
                    *dest_addr.ip(),
                    17,
                    len as u32,
                ),
                checksum::data(&buffer[..]),
            ]);
            let checksum = if checksum == 0x0000 { 0xffff } else { 0x0000 };
            NetworkEndian::write_u16(&mut buffer[6..8], checksum);
        },
        UdpFields::V6 {
            source_addr,
            dest_addr,
        } => {
            NetworkEndian::write_u16(&mut buffer[0..2], source_addr.port());
            NetworkEndian::write_u16(&mut buffer[2..4], dest_addr.port());
            let len = buffer.len() as u16;
            NetworkEndian::write_u16(&mut buffer[4..6], len);
            NetworkEndian::write_u16(&mut buffer[6..8], 0);

            let checksum = !checksum::combine(&[
                checksum::pseudo_header_ipv6(
                    *source_addr.ip(),
                    *dest_addr.ip(),
                    17,
                    len as u32,
                ),
                checksum::data(&buffer[..]),
            ]);
            let checksum = if checksum == 0x0000 { 0xffff } else { 0x0000 };
            NetworkEndian::write_u16(&mut buffer[6..8], checksum);
        },
    }
}

impl UdpPacket {
    /// Create a new UDP packet from the given header fields and payload.
    pub fn new_from_fields(
        fields: UdpFields,
        payload: Bytes,
    ) -> UdpPacket {
        let len = 8 + payload.len();
        let mut buffer = unsafe { BytesMut::uninit(len) };
        UdpPacket::write_to_buffer(&mut buffer, fields, payload);
        UdpPacket {
            buffer: buffer.freeze(),
        }
    }

    /// Write a UDP packet to the given empty buffer.
    pub fn write_to_buffer(
        buffer: &mut [u8],
        fields: UdpFields,
        payload: Bytes,
    ) {
        buffer[8..].clone_from_slice(&payload);
        set_fields(buffer, fields);
    }

    /// Parse a UDP packet from the given buffer.
    pub fn from_bytes(buffer: Bytes) -> UdpPacket {
        UdpPacket {
            buffer,
        }
    }

    /// Set the header fields of this UDP packet.
    pub fn set_fields(&mut self, fields: UdpFields) {
        let buffer = mem::replace(&mut self.buffer, Bytes::new());
        let mut buffer = BytesMut::from(buffer);
        set_fields(&mut buffer, fields);
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

    /// Verify the checksum of the packet. The source/destination IP addresses of the packet are
    /// needed to calculate the checksum.
    pub fn verify_checksum_v4(
        &self,
        source_ip: Ipv4Addr,
        dest_ip: Ipv4Addr,
    ) -> bool {
        let len = NetworkEndian::read_u16(&self.buffer[4..6]);
        !0 == checksum::combine(&[
            checksum::pseudo_header_ipv4(source_ip, dest_ip, 17, len as u32),
            checksum::data(&self.buffer[..]),
        ])
    }
}

