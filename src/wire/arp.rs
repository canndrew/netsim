use priv_prelude::*;
use super::*;

#[derive(Clone)]
pub struct ArpPacket {
    buffer: Bytes,
}

impl fmt::Debug for ArpPacket {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.fields() {
            ArpFields::Request { .. } => {
                f
                .debug_struct("ArpPacket::Request")
                .field("source_mac", &self.source_mac())
                .field("source_ip", &self.source_ip())
                .field("dest_mac", &self.dest_mac())
                .field("dest_ip", &self.dest_ip())
                .finish()
            },
            ArpFields::Response { .. } => {
                f
                .debug_struct("ArpPacket::Response")
                .field("source_mac", &self.source_mac())
                .field("source_ip", &self.source_ip())
                .field("dest_mac", &self.dest_mac())
                .field("dest_ip", &self.dest_ip())
                .finish()
            },
        }
    }
}

#[derive(Clone, Copy)]
pub enum ArpFields {
    Request {
        source_mac: MacAddr,
        source_ip: Ipv4Addr,
        dest_ip: Ipv4Addr,
    },
    Response {
        source_mac: MacAddr,
        source_ip: Ipv4Addr,
        dest_mac: MacAddr,
        dest_ip: Ipv4Addr,
    },
}

impl ArpPacket {
    pub fn new_from_fields(fields: ArpFields) -> ArpPacket {
        let mut buffer = unsafe { BytesMut::uninit(28) };
        buffer[0..6].clone_from_slice(&[
            0x00, 0x01,
            0x08, 0x00,
            0x06, 0x04,
        ]);
        match fields {
            ArpFields::Request {
                source_mac,
                source_ip,
                dest_ip,
            } => {
                buffer[6..8].clone_from_slice(&[0x00, 0x01]);
                buffer[8..14].clone_from_slice(source_mac.as_bytes());
                buffer[14..18].clone_from_slice(&source_ip.octets());
                buffer[18..24].clone_from_slice(&[0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);
                buffer[24..28].clone_from_slice(&dest_ip.octets());
            },
            ArpFields::Response {
                source_mac,
                source_ip,
                dest_mac,
                dest_ip,
            } => {
                buffer[6..8].clone_from_slice(&[0x00, 0x02]);
                buffer[8..14].clone_from_slice(source_mac.as_bytes());
                buffer[14..18].clone_from_slice(&source_ip.octets());
                buffer[18..24].clone_from_slice(dest_mac.as_bytes());
                buffer[24..28].clone_from_slice(&dest_ip.octets());
            },
        }
        ArpPacket {
            buffer: buffer.freeze(),
        }
    }

    pub fn from_bytes(buffer: Bytes) -> ArpPacket {
        ArpPacket {
            buffer,
        }
    }

    pub fn fields(&self) -> ArpFields {
        match NetworkEndian::read_u16(&self.buffer[6..8]) {
            0x0001 => ArpFields::Request {
                source_mac: self.source_mac(),
                source_ip: self.source_ip(),
                dest_ip: self.dest_ip(),
            },
            0x0002 => ArpFields::Response {
                source_mac: self.source_mac(),
                source_ip: self.source_ip(),
                dest_mac: self.dest_mac(),
                dest_ip: self.dest_ip(),
            },
            x => panic!("unexpected ARP operation type (0x{:04x})", x),
        }
    }

    pub fn source_mac(&self) -> MacAddr {
        MacAddr::from_bytes(&self.buffer[8..14])
    }

    pub fn source_ip(&self) -> Ipv4Addr {
        Ipv4Addr::from(slice_assert_len!(4, &self.buffer[14..18]))
    }

    pub fn dest_mac(&self) -> MacAddr {
        MacAddr::from_bytes(&self.buffer[18..24])
    }

    pub fn dest_ip(&self) -> Ipv4Addr {
        Ipv4Addr::from(slice_assert_len!(4, &self.buffer[24..28]))
    }

    pub fn as_bytes(&self) -> &Bytes {
        &self.buffer
    }
}

