use priv_prelude::*;
use checksum;

pub struct Ipv4Packet {
    buffer: Bytes,
}

pub struct Ipv4Fields {
    pub source_ip: Ipv4Addr,
    pub dest_ip: Ipv4Addr,
    pub ttl: u8,
}

pub enum Ipv4Payload {
    Unknown {
        protocol: u8,
        payload: Bytes,
    },
}

fn set_fields(buffer: &mut BytesMut, fields: Ipv4Fields) {
    buffer[0] = 0x45;
    buffer[1] = 0x00;
    buffer[4..6].clone_from_slice(&[0x00, 0x00]);
    buffer[6..8].clone_from_slice(&[0x00, 0x00]);
    buffer[8] = fields.ttl;
    buffer[10..12].clone_from_slice(&[0x00, 0x00]);
    buffer[12..16].clone_from_slice(&fields.source_ip.octets());
    buffer[16..20].clone_from_slice(&fields.dest_ip.octets());

    let checksum = checksum::data(&buffer[0..20]);
    NetworkEndian::write_u16(&mut buffer[10..12], checksum);
}

impl Ipv4Packet {
    pub fn new_from_fields(
        fields: Ipv4Fields,
        payload: &Ipv4Payload,
    ) -> Ipv4Packet {
        let len = 20 + match *payload {
            Ipv4Payload::Unknown { ref payload, .. } => payload.len(),
        };
        let mut buffer = unsafe { BytesMut::uninit(len) };
        NetworkEndian::write_u16(&mut buffer[2..4], len as u16);
        buffer[9] = match *payload {
            Ipv4Payload::Unknown { protocol, .. } => protocol,
        };

        set_fields(&mut buffer, fields);

        match *payload {
            Ipv4Payload::Unknown { ref payload, .. } => buffer[20..].clone_from_slice(&payload),
        }

        Ipv4Packet {
            buffer: buffer.freeze(),
        }
    }

    pub fn set_fields(&mut self, fields: Ipv4Fields) {
        let buffer = mem::replace(&mut self.buffer, Bytes::new());
        let mut buffer = BytesMut::from(buffer);
        set_fields(&mut buffer, fields);
        self.buffer = buffer.freeze();
    }

    pub fn source_ip(&self) -> Ipv4Addr {
        Ipv4Addr::from(slice_assert_len!(4, &self.buffer[12..16]))
    }

    pub fn dest_ip(&self) -> Ipv4Addr {
        Ipv4Addr::from(slice_assert_len!(4, &self.buffer[16..20]))
    }

    pub fn ttl(&self) -> u8 {
        self.buffer[8]
    }

    pub fn payload(&self) -> Ipv4Payload {
        match self.buffer[9] {
            p => Ipv4Payload::Unknown {
                protocol: p,
                payload: self.buffer.slice_from(20),
            },
        }
    }
}

