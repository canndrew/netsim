use priv_prelude::*;
//use futures::future::Loop;

/// An ethernet frame.
#[derive(Clone, PartialEq, Eq)]
pub struct EtherFrame {
    data: Bytes,
}

impl fmt::Debug for EtherFrame {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f
        .debug_struct("EtherFrame")
        .field("source", &self.source())
        .field("destination", &self.destination())
        .field("payload", &self.payload())
        .finish()
    }
}

/// The payload of an ethernet frame.
#[derive(Debug)]
pub enum EtherPayload {
    /// IPv4
    Ipv4(Ipv4Packet<Bytes>),
    /// IPv6
    Ipv6(Ipv6Packet<Bytes>),
    /// ARP (Address Resolution Protocol)
    Arp(ArpPacket<Bytes>),
    /// Unknkown. The two bytes represent the Ethernet II EtherType of the packet. The `Bytes` is
    /// the payload.
    Unknown([u8; 2], Bytes),
}

impl EtherFrame {
    pub fn new() -> EtherFrame {
        EtherFrame {
            data: Bytes::from(&[0u8; 14][..]),
        }
    }

    /// Create an ethernet frame from a slice of bytes.
    pub fn from_bytes(data: Bytes) -> EtherFrame {
        EtherFrame {
            data,
        }
    }

    /// Return the frame as a slice of bytes.
    pub fn as_bytes(&self) -> &Bytes {
        &self.data
    }

    /// Get the source MAC address of the frame.
    pub fn source(&self) -> EthernetAddress {
        EthernetAddress::from_bytes(&self.data[6..12])
    }

    /// Get the destination MAC address of the frame.
    pub fn destination(&self) -> EthernetAddress {
        EthernetAddress::from_bytes(&self.data[0..6])
    }

    /// Get the payload of the frame.
    pub fn payload(&self) -> EtherPayload {
        match (self.data[12], self.data[13]) {
            (0x08, 0x00) => EtherPayload::Ipv4(Ipv4Packet::new(self.data.slice_from(14))),
            (0x86, 0xdd) => EtherPayload::Ipv6(Ipv6Packet::new(self.data.slice_from(14))),
            (0x08, 0x06) => EtherPayload::Arp(ArpPacket::new(self.data.slice_from(14))),
            (x, y) => EtherPayload::Unknown([x, y], self.data.slice_from(14)),
        }
    }

    /// Get the length of the frame, in bytes.
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Set the source MAC address of the frame.
    pub fn set_source(&mut self, addr: EthernetAddress) {
        let bytes = mem::replace(&mut self.data, Bytes::new());
        let mut bytes_mut = BytesMut::from(bytes);
        bytes_mut[6..12].clone_from_slice(&addr.as_bytes()[..]);
        self.data = bytes_mut.into();
    }

    /// Set the destination MAC address of the frame.
    pub fn set_destination(&mut self, addr: EthernetAddress) {
        let bytes = mem::replace(&mut self.data, Bytes::new());
        let mut bytes_mut = BytesMut::from(bytes);
        bytes_mut[0..6].clone_from_slice(&addr.as_bytes()[..]);
        self.data = bytes_mut.into();
    }

    /// Set the payload of the frame.
    pub fn set_payload(&mut self, payload: EtherPayload) {
        let mut bytes_mut = BytesMut::new();
        bytes_mut.extend(&self.data[..12]);
        match payload {
            EtherPayload::Ipv4(ipv4) => {
                bytes_mut.extend_from_slice(&[0x08, 0x00]);
                bytes_mut.extend_from_slice(&ipv4.into_inner());
            },
            EtherPayload::Ipv6(ipv6) => {
                bytes_mut.extend_from_slice(&[0x86, 0xdd]);
                bytes_mut.extend_from_slice(&ipv6.into_inner());
            },
            EtherPayload::Arp(arp) => {
                bytes_mut.extend_from_slice(&[0x08, 0x06]);
                bytes_mut.extend_from_slice(&arp.into_inner());
            },
            EtherPayload::Unknown(xy, payload) => {
                bytes_mut.extend_from_slice(&xy);
                bytes_mut.extend_from_slice(&payload);
            },
        }
        self.data = bytes_mut.into();
    }
}

/// Convenience type alias for a boxed stream/sink of ethernet frames.
pub type EtherBox = Box<EtherChannel<
    Item = EtherFrame,
    Error = io::Error,
    SinkItem = EtherFrame,
    SinkError = io::Error,
> + 'static>;

/// Trait alias (or at least will be when trait aliases are stable) representing a `Stream`/`Sink`
/// of ethernet frames.
pub trait EtherChannel: Stream<Item=EtherFrame, Error=io::Error>
                      + Sink<SinkItem=EtherFrame, SinkError=io::Error>
{
}

impl<T> EtherChannel for T
where
    T: Stream<Item=EtherFrame, Error=io::Error>
       + Sink<SinkItem=EtherFrame, SinkError=io::Error>
       + Sized,
{
}

// TODO: make this a method
/*
pub fn respond_to_arp(
    this: EtherBox,
    ip_addr: Ipv4Addr,
    mac_addr: EthernetAddress,
) -> BoxFuture<Option<EtherBox>, io::Error> {
    future::loop_fn(this, move |this| {
        this
        .into_future()
        .map_err(|(e, _)| e)
        .and_then(move |(frame_opt, this)| {
            match frame_opt {
                Some(mut frame) => {
                    if let EtherPayload::Arp(arp) = frame.payload() {
                        if {
                            arp.operation() == ArpOperation::Request &&
                            arp.destination_ip() == ip_addr
                        } {
                            let arp = arp.response(mac_addr);
                            frame.set_source(arp.source_mac());
                            frame.set_destination(arp.destination_mac());
                            frame.set_payload(EtherPayload::Arp(arp));

                            return {
                                this
                                .send(frame)
                                .map(|this| Loop::Break(Some(this)))
                                .into_boxed()
                            };
                        }
                    }
                    future::ok(Loop::Continue(this)).into_boxed()
                },
                None => future::ok(Loop::Break(None)).into_boxed()
            }
        })
    })
    .into_boxed()
}
*/

