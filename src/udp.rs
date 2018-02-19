use priv_prelude::*;
use checksum;

#[derive(Clone)]
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

#[derive(Debug, Clone)]
pub enum UdpFields {
    V4 {
        source_addr: SocketAddrV4,
        dest_addr: SocketAddrV4,
    },
    V6 {
        source_addr: SocketAddrV6,
        dest_addr: SocketAddrV6,
    },
}

fn set_fields(buffer: &mut BytesMut, fields: UdpFields) {
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
    pub fn new_from_fields(
        fields: UdpFields,
        payload: Bytes,
    ) -> UdpPacket {
        let len = 8 + payload.len();
        let mut buffer = unsafe { BytesMut::uninit(len) };
        buffer[8..].clone_from_slice(&payload);
        set_fields(&mut buffer, fields);
        UdpPacket {
            buffer: buffer.freeze(),
        }
    }

    pub fn from_bytes(buffer: Bytes) -> UdpPacket {
        UdpPacket {
            buffer,
        }
    }

    pub fn set_fields(&mut self, fields: UdpFields) {
        let buffer = mem::replace(&mut self.buffer, Bytes::new());
        let mut buffer = BytesMut::from(buffer);
        set_fields(&mut buffer, fields);
        self.buffer = buffer.freeze();
    }

    pub fn source_port(&self) -> u16 {
        NetworkEndian::read_u16(&self.buffer[0..2])
    }

    pub fn dest_port(&self) -> u16 {
        NetworkEndian::read_u16(&self.buffer[2..4])
    }

    pub fn payload(&self) -> Bytes {
        self.buffer.slice_from(8)
    }

    pub fn as_bytes(&self) -> &Bytes {
        &self.buffer
    }
}

