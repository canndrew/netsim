use priv_prelude::*;

/// A MAC address, the hardware address of an ethernet interface.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Rand)]
pub struct MacAddr {
    bytes: [u8; 6],
}

impl MacAddr {
    /// Create a mac address from a slice of 6 bytes.
    ///
    /// # Panic
    ///
    /// If the slice is not exactly 6 bytes.
    pub fn from_bytes(b: &[u8]) -> MacAddr {
        let mut bytes = [0u8; 6];
        bytes[..].clone_from_slice(b);
        MacAddr {
            bytes,
        }
    }

    /// Returns the mac address as an array of 6 bytes.
    pub fn as_bytes(&self) -> [u8; 6] {
        self.bytes
    }

    /// Returns the broadcast MAC address FF:FF:FF:FF:FF:FF
    pub fn broadcast() -> MacAddr {
        MacAddr {
            bytes: [0xff; 6],
        }
    }

    /// Check whether a frame with the MAC address should be received by an interface with address `iface`. Returns `true` if either `self` is the broadcast address or the addresses are equal
    pub fn matches(self, iface: MacAddr) -> bool {
        self == MacAddr::broadcast() || self == iface
    }
}

impl fmt::Debug for MacAddr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
                  self.bytes[0], self.bytes[1], self.bytes[2],
                  self.bytes[3], self.bytes[4], self.bytes[5])
    }
}

impl fmt::Display for MacAddr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

/// An ethernet frame.
#[derive(Clone)]
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
    Ipv4(Ipv4Packet),
    /// IPv6
    Ipv6(Ipv6Packet),
    /// ARP (Address Resolution Protocol)
    Arp(ArpPacket),
    /// Unknkown. The two bytes represent the Ethernet II EtherType of the packet. The `Bytes` is
    /// the payload.
    Unknown([u8; 2], Bytes),
}

impl EtherFrame {
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
    pub fn source(&self) -> MacAddr {
        MacAddr::from_bytes(&self.data[6..12])
    }

    /// Get the destination MAC address of the frame.
    pub fn destination(&self) -> MacAddr {
        MacAddr::from_bytes(&self.data[0..6])
    }

    /// Get the payload of the frame.
    pub fn payload(&self) -> EtherPayload {
        match (self.data[12], self.data[13]) {
            (0x08, 0x00) => EtherPayload::Ipv4(Ipv4Packet::from_bytes(self.data.slice_from(14))),
            (0x86, 0xdd) => EtherPayload::Ipv6(Ipv6Packet::from_bytes(self.data.slice_from(14))),
            (0x08, 0x06) => EtherPayload::Arp(ArpPacket::from_bytes(self.data.slice_from(14))),
            (x, y) => EtherPayload::Unknown([x, y], self.data.slice_from(14)),
        }
    }

    /// Get the length of the frame, in bytes.
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Set the source MAC address of the frame.
    pub fn set_source(&mut self, addr: MacAddr) {
        let bytes = mem::replace(&mut self.data, Bytes::new());
        let mut bytes_mut = BytesMut::from(bytes);
        bytes_mut[6..12].clone_from_slice(&addr.bytes[..]);
        self.data = bytes_mut.into();
    }

    /// Set the destination MAC address of the frame.
    pub fn set_destination(&mut self, addr: MacAddr) {
        let bytes = mem::replace(&mut self.data, Bytes::new());
        let mut bytes_mut = BytesMut::from(bytes);
        bytes_mut[0..6].clone_from_slice(&addr.bytes[..]);
        self.data = bytes_mut.into();
    }

    /// Set the payload of the frame.
    pub fn set_payload(&mut self, payload: EtherPayload) {
        let mut bytes_mut = BytesMut::new();
        bytes_mut.extend(&self.data[..12]);
        match payload {
            EtherPayload::Ipv4(ipv4) => {
                bytes_mut.extend_from_slice(&[0x08, 0x00]);
                bytes_mut.extend_from_slice(ipv4.as_bytes());
            },
            EtherPayload::Ipv6(ipv6) => {
                bytes_mut.extend_from_slice(&[0x86, 0xdd]);
                bytes_mut.extend_from_slice(ipv6.as_bytes());
            },
            EtherPayload::Arp(arp) => {
                bytes_mut.extend_from_slice(&[0x08, 0x06]);
                bytes_mut.extend_from_slice(arp.as_bytes());
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
       + Sink<SinkItem=EtherFrame, SinkError=io::Error>,
{
}

