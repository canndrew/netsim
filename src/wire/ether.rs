use priv_prelude::*;
use super::*;

#[derive(Clone)]
pub struct EtherFrame {
    buffer: Bytes,
}

impl fmt::Debug for EtherFrame {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let payload = self.payload();

        f
        .debug_struct("EtherFrame")
        .field("source_mac", &self.source_mac())
        .field("dest_mac", &self.dest_mac())
        .field("payload", match payload {
            EtherPayload::Arp(ref arp) => arp,
            EtherPayload::Ipv4(ref ipv4) => ipv4,
            EtherPayload::Unknown { .. } => &payload,
        })
        .finish()
    }
}

#[derive(Clone, Debug)]
pub struct EtherFields {
    pub source_mac: MacAddr,
    pub dest_mac: MacAddr,
}

#[derive(Clone, Debug)]
pub enum EtherPayload {
    Arp(ArpPacket),
    Ipv4(Ipv4Packet),
    Unknown {
        ethertype: u16, 
        payload: Bytes,
    },
}

fn set_fields(buffer: &mut BytesMut, fields: EtherFields) {
    buffer[0..6].clone_from_slice(fields.source_mac.as_bytes());
    buffer[6..12].clone_from_slice(fields.dest_mac.as_bytes());
}

impl EtherFrame {
    pub fn new_from_fields(
        fields: EtherFields,
        payload: &EtherPayload,
    ) -> EtherFrame {
        let len = 18 + match *payload {
            EtherPayload::Arp(ref arp) => arp.as_bytes().len(),
            EtherPayload::Ipv4(ref ipv4) => ipv4.as_bytes().len(),
            EtherPayload::Unknown { ref payload, .. } => payload.len(),
        };
        let mut buffer = unsafe { BytesMut::uninit(len) };
        set_fields(&mut buffer, fields);
        let ethertype = match *payload {
            EtherPayload::Arp(..) => 0x0806,
            EtherPayload::Ipv4(..) => 0x0800,
            EtherPayload::Unknown { ethertype, .. } => ethertype,
        };
        NetworkEndian::write_u16(&mut buffer[12..14], ethertype);
        buffer[14..(len - 4)].clone_from_slice(match *payload {
            EtherPayload::Arp(ref arp) => arp.as_bytes(),
            EtherPayload::Ipv4(ref ipv4) => ipv4.as_bytes(),
            EtherPayload::Unknown { ref payload, .. } => &payload,
        });
        // TODO: correctly set checksum
        buffer[(len - 4)..].clone_from_slice(&[0x00, 0x00, 0x00, 0x00]);

        EtherFrame {
            buffer: buffer.freeze(),
        }
    }

    pub fn set_fields(&mut self, fields: EtherFields) {
        let buffer = mem::replace(&mut self.buffer, Bytes::new());
        let mut buffer = BytesMut::from(buffer);
        set_fields(&mut buffer, fields);
        self.buffer = buffer.freeze();
    }

    pub fn from_bytes(buffer: Bytes) -> EtherFrame {
        EtherFrame {
            buffer,
        }
    }

    pub fn source_mac(&self) -> MacAddr {
        MacAddr::from_bytes(&self.buffer[0..6])
    }

    pub fn dest_mac(&self) -> MacAddr {
        MacAddr::from_bytes(&self.buffer[6..12])
    }

    pub fn payload(&self) -> EtherPayload {
        let end = self.buffer.len() - 4;
        match NetworkEndian::read_u16(&self.buffer[12..14]) {
            0x0806 => EtherPayload::Arp(ArpPacket::from_bytes(self.buffer.slice(14, end))),
            0x0800 => EtherPayload::Ipv4(Ipv4Packet::from_bytes(self.buffer.slice(14, end))),
            p => EtherPayload::Unknown {
                ethertype: p,
                payload: self.buffer.slice(14, end),
            },
        }
    }

    pub fn as_bytes(&self) -> &Bytes {
        &self.buffer
    }
}

