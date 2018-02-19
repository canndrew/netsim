use priv_prelude::*;

pub struct ArpPacket {
    buffer: Bytes,
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
                buffer[6..8].clone_from_slice(&[0x00, 0x01]);
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

    pub fn as_bytes(&self) -> &[u8] {
        &self.buffer
    }
}

