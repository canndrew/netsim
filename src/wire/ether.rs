use priv_prelude::*;
use future_utils;

#[derive(Clone, PartialEq)]
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

#[derive(Clone, Copy, Debug, PartialEq)]
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

pub enum EtherPayloadFields {
    Arp {
        fields: ArpFields,
    },
    Ipv4 {
        fields: Ipv4Fields,
        payload_fields: Ipv4PayloadFields,
    },
}

impl EtherPayloadFields {
    pub fn total_frame_len(&self) -> usize {
        14 + match *self {
            EtherPayloadFields::Arp { .. } => 28,
            EtherPayloadFields::Ipv4 { ref payload_fields, .. } => {
                payload_fields.total_packet_len()
            },
        }
    }
}

fn set_fields(buffer: &mut [u8], fields: EtherFields) {
    buffer[0..6].clone_from_slice(fields.dest_mac.as_bytes());
    buffer[6..12].clone_from_slice(fields.source_mac.as_bytes());
}

impl EtherFrame {
    pub fn new_from_fields(
        fields: EtherFields,
        payload: EtherPayload,
    ) -> EtherFrame {
        let len = 14 + match payload {
            EtherPayload::Arp(ref arp) => arp.as_bytes().len(),
            EtherPayload::Ipv4(ref ipv4) => ipv4.as_bytes().len(),
            EtherPayload::Unknown { ref payload, .. } => payload.len(),
        };
        let mut buffer = unsafe { BytesMut::uninit(len) };
        set_fields(&mut buffer, fields);
        let ethertype = match payload {
            EtherPayload::Arp(..) => 0x0806,
            EtherPayload::Ipv4(..) => 0x0800,
            EtherPayload::Unknown { ethertype, .. } => ethertype,
        };
        NetworkEndian::write_u16(&mut buffer[12..14], ethertype);
        buffer[14..].clone_from_slice(match payload {
            EtherPayload::Arp(ref arp) => arp.as_bytes(),
            EtherPayload::Ipv4(ref ipv4) => ipv4.as_bytes(),
            EtherPayload::Unknown { ref payload, .. } => &payload,
        });

        EtherFrame {
            buffer: buffer.freeze(),
        }
    }

    pub fn new_from_fields_recursive(
        fields: EtherFields,
        payload_fields: EtherPayloadFields,
    ) -> EtherFrame {
        let len = payload_fields.total_frame_len();
        let mut buffer = unsafe { BytesMut::uninit(len) };
        
        EtherFrame::write_to_buffer(&mut buffer, fields, payload_fields);
        EtherFrame {
            buffer: buffer.freeze(),
        }
    }

    pub fn write_to_buffer(
        buffer: &mut [u8],
        fields: EtherFields,
        payload_fields: EtherPayloadFields,
    ) {
        let ethertype = match payload_fields {
            EtherPayloadFields::Arp { .. } => 0x0806,
            EtherPayloadFields::Ipv4 { .. } => 0x0800,
        };
        NetworkEndian::write_u16(&mut buffer[12..14], ethertype);

        set_fields(buffer, fields);

        match payload_fields {
            EtherPayloadFields::Arp { fields } => {
                ArpPacket::write_to_buffer(&mut buffer[14..], fields);
            },
            EtherPayloadFields::Ipv4 { fields, payload_fields } => {
                Ipv4Packet::write_to_buffer(&mut buffer[14..], fields, payload_fields);
            },
        }
    }

    pub fn fields(&self) -> EtherFields {
        EtherFields {
            source_mac: self.source_mac(),
            dest_mac: self.dest_mac(),
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
        MacAddr::from_bytes(&self.buffer[6..12])
    }

    pub fn dest_mac(&self) -> MacAddr {
        MacAddr::from_bytes(&self.buffer[0..6])
    }

    pub fn payload(&self) -> EtherPayload {
        match NetworkEndian::read_u16(&self.buffer[12..14]) {
            0x0806 => EtherPayload::Arp(ArpPacket::from_bytes(self.buffer.slice_from(14))),
            0x0800 => EtherPayload::Ipv4(Ipv4Packet::from_bytes(self.buffer.slice_from(14))),
            p => EtherPayload::Unknown {
                ethertype: p,
                payload: self.buffer.slice_from(14),
            },
        }
    }

    pub fn as_bytes(&self) -> &Bytes {
        &self.buffer
    }
}

pub struct EtherPlug {
    pub tx: UnboundedSender<EtherFrame>,
    pub rx: UnboundedReceiver<EtherFrame>,
}

impl EtherPlug {
    pub fn new_wire() -> (EtherPlug, EtherPlug) {
        let (a_tx, b_rx) = future_utils::mpsc::unbounded();
        let (b_tx, a_rx) = future_utils::mpsc::unbounded();
        let a = EtherPlug {
            tx: a_tx,
            rx: a_rx,
        };
        let b = EtherPlug {
            tx: b_tx,
            rx: b_rx,
        };
        (a, b)
    }
}

