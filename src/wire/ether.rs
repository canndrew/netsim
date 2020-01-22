use crate::priv_prelude::*;
use futures::sync::mpsc::SendError;

#[derive(Clone, PartialEq)]
/// Represents an ethernet frame.
pub struct EtherFrame {
    buffer: Bytes,
}

impl fmt::Debug for EtherFrame {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let payload = self.payload();

        f.debug_struct("EtherFrame")
            .field("source_mac", &self.source_mac())
            .field("dest_mac", &self.dest_mac())
            .field(
                "payload",
                match payload {
                    EtherPayload::Arp(ref arp) => arp,
                    EtherPayload::Ipv4(ref ipv4) => ipv4,
                    EtherPayload::Unknown { .. } => &payload,
                },
            )
            .finish()
    }
}

/// The header fields of an ethernet packet.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EtherFields {
    /// The frame's source MAC address.
    pub source_mac: MacAddr,
    /// The frame's destination MAC address.
    pub dest_mac: MacAddr,
}

#[derive(Clone, Debug)]
/// The payload of an ethernet frame.
pub enum EtherPayload {
    /// An ARP packet
    Arp(ArpPacket),
    /// An Ipv4 packet
    Ipv4(Ipv4Packet),
    /// A packet with an unrecognised protocol.
    Unknown {
        /// The ethertype of the protocol.
        ethertype: u16,
        /// The packet's payload data.
        payload: Bytes,
    },
}

/// The fields of the payload of an ethernet frame. Can be used along with `EtherFields` to
/// describe/construct an ethernet frame and its contents.
pub enum EtherPayloadFields {
    /// An ARP packet
    Arp {
        /// The ARP packet's fields.
        fields: ArpFields,
    },
    /// An Ipv4 packet
    Ipv4 {
        /// The Ipv4 packet's header fields.
        fields: Ipv4Fields,
        /// The Ipv4 packet's payload
        payload_fields: Ipv4PayloadFields,
    },
}

impl EtherPayloadFields {
    /// The total length of an ethernet frame with this payload
    pub fn payload_len(&self) -> usize {
        match *self {
            EtherPayloadFields::Arp { .. } => 28,
            EtherPayloadFields::Ipv4 {
                ref fields,
                ref payload_fields,
            } => fields.header_len() + payload_fields.payload_len(),
        }
    }
}

fn set_fields(buffer: &mut [u8], fields: EtherFields) {
    buffer[0..6].clone_from_slice(fields.dest_mac.as_bytes());
    buffer[6..12].clone_from_slice(fields.source_mac.as_bytes());
}

impl EtherFrame {
    /// Construct a new `EthernetFrame`. Using `new_from_fields_recursive` can avoid an extra
    /// allocation if you are also constructing the frame's payload.
    pub fn new_from_fields(fields: EtherFields, payload: &EtherPayload) -> EtherFrame {
        let len = 14
            + match payload {
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
        buffer[14..].clone_from_slice(match payload {
            EtherPayload::Arp(ref arp) => arp.as_bytes(),
            EtherPayload::Ipv4(ref ipv4) => ipv4.as_bytes(),
            EtherPayload::Unknown { ref payload, .. } => payload,
        });

        EtherFrame {
            buffer: buffer.freeze(),
        }
    }

    /// Construct a new `EthernetFrame`.
    pub fn new_from_fields_recursive(
        fields: EtherFields,
        payload_fields: EtherPayloadFields,
    ) -> EtherFrame {
        let len = 14 + payload_fields.payload_len();
        let mut buffer = unsafe { BytesMut::uninit(len) };

        EtherFrame::write_to_buffer(&mut buffer, fields, payload_fields);
        EtherFrame {
            buffer: buffer.freeze(),
        }
    }

    /// Create a new ethernet frame, writing it to the given buffer.
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
            }
            EtherPayloadFields::Ipv4 {
                fields,
                payload_fields,
            } => {
                Ipv4Packet::write_to_buffer(&mut buffer[14..], fields, payload_fields);
            }
        }
    }

    /// Get the fields of this ethernet frame.
    pub fn fields(&self) -> EtherFields {
        EtherFields {
            source_mac: self.source_mac(),
            dest_mac: self.dest_mac(),
        }
    }

    /// Set the fields of this ethernet frame.
    pub fn set_fields(&mut self, fields: EtherFields) {
        let buffer = mem::replace(&mut self.buffer, Bytes::new());
        let mut buffer = BytesMut::from(buffer);
        set_fields(&mut buffer, fields);
        self.buffer = buffer.freeze();
    }

    /// Construct a new ethernet frame from the given buffer.
    pub fn from_bytes(buffer: Bytes) -> EtherFrame {
        EtherFrame { buffer }
    }

    /// Get the frame's sender MAC address.
    pub fn source_mac(&self) -> MacAddr {
        MacAddr::from_bytes(&self.buffer[6..12])
    }

    /// Get the frame's destination MAC address.
    pub fn dest_mac(&self) -> MacAddr {
        MacAddr::from_bytes(&self.buffer[0..6])
    }

    /// Get the frame's payload
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

    /// Returns the underlying buffer.
    pub fn as_bytes(&self) -> &Bytes {
        &self.buffer
    }

    /// Consume the frame and return the underlying buffer
    pub fn into_bytes(self) -> Bytes {
        self.buffer
    }
}

#[derive(Debug)]
/// An ethernet plug
pub struct EtherPlug {
    inner: Plug<EtherFrame>,
}

impl EtherPlug {
    /// Create a connected pair of plugs
    pub fn new_pair() -> (EtherPlug, EtherPlug) {
        let (plug_a, plug_b) = Plug::new_pair();
        let plug_a = EtherPlug { inner: plug_a };
        let plug_b = EtherPlug { inner: plug_b };
        (plug_a, plug_b)
    }

    /// Add latency to a connection
    pub fn with_latency(
        self,
        handle: &NetworkHandle,
        min_latency: Duration,
        mean_additional_latency: Duration,
    ) -> EtherPlug {
        EtherPlug {
            inner: self
                .inner
                .with_latency(handle, min_latency, mean_additional_latency),
        }
    }

    /// Add packet loss to a connection
    pub fn with_packet_loss(
        self,
        handle: &NetworkHandle,
        loss_rate: f64,
        mean_loss_duration: Duration,
    ) -> EtherPlug {
        EtherPlug {
            inner: self
                .inner
                .with_packet_loss(handle, loss_rate, mean_loss_duration),
        }
    }

    /// Split into sending and receiving halves
    pub fn split(self) -> (UnboundedSender<EtherFrame>, UnboundedReceiver<EtherFrame>) {
        self.inner.split()
    }

    /// Poll for incoming frames
    pub fn poll_incoming(&mut self) -> Async<Option<EtherFrame>> {
        self.inner.rx.poll().void_unwrap()
    }

    /// Send a frame
    pub fn unbounded_send(&mut self, frame: EtherFrame) -> Result<(), SendError<EtherFrame>> {
        self.inner.tx.unbounded_send(frame)
    }
}

impl From<EtherPlug> for Plug<EtherFrame> {
    fn from(plug: EtherPlug) -> Plug<EtherFrame> {
        plug.inner
    }
}
