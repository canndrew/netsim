use priv_prelude::*;
use checksum;

#[derive(Clone)]
pub struct Ipv4Packet {
    buffer: Bytes,
}

impl fmt::Debug for Ipv4Packet {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let payload = self.payload();

        f
        .debug_struct("Ipv4Packet")
        .field("source_ip", &self.source_ip())
        .field("dest_ip", &self.dest_ip())
        .field("ttl", &self.ttl())
        .field("payload", match payload {
            Ipv4Payload::Udp(ref udp) => udp,
            Ipv4Payload::Unknown { .. } => &payload,
        })
        .finish()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Ipv4Fields {
    pub source_ip: Ipv4Addr,
    pub dest_ip: Ipv4Addr,
    pub ttl: u8,
}

#[derive(Debug, Clone)]
pub enum Ipv4Payload {
    Udp(UdpPacket),
    Unknown {
        protocol: u8,
        payload: Bytes,
    },
}

fn set_fields(buffer: &mut BytesMut, fields: Ipv4Fields) {
    buffer[0] = 0x45;
    buffer[1] = 0x00;
    let len = buffer.len() as u16;
    NetworkEndian::write_u16(&mut buffer[2..4], len);
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
            Ipv4Payload::Udp(ref udp) => udp.as_bytes().len(),
            Ipv4Payload::Unknown { ref payload, .. } => payload.len(),
        };
        let mut buffer = unsafe { BytesMut::uninit(len) };
        buffer[9] = match *payload {
            Ipv4Payload::Udp(..) => 17,
            Ipv4Payload::Unknown { protocol, .. } => protocol,
        };

        set_fields(&mut buffer, fields);

        match *payload {
            Ipv4Payload::Udp(ref udp) => buffer[20..].clone_from_slice(udp.as_bytes()),
            Ipv4Payload::Unknown { ref payload, .. } => buffer[20..].clone_from_slice(&payload),
        }

        Ipv4Packet {
            buffer: buffer.freeze(),
        }
    }

    pub fn from_bytes(buffer: Bytes) -> Ipv4Packet {
        Ipv4Packet {
            buffer,
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

    pub fn as_bytes(&self) -> &Bytes {
        &self.buffer
    }
}

