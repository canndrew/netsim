use priv_prelude::*;

/// A IPv4 UDP packet
#[derive(Clone)]
pub struct UdpPacket {
    data: Bytes,
}

impl fmt::Debug for UdpPacket {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f
        .debug_struct("UdpPacket")
        .field("source_port", &self.source_port())
        .field("destination_port", &self.destination_port())
        .finish()
    }
}

impl UdpPacket {
    /// Create a UDP packet from raw UDP data. This contains the UDP header but not the IPv4
    /// header.
    pub fn from_bytes(data: Bytes) -> UdpPacket {
        UdpPacket {
            data,
        }
    }

    /// Get the raw packet data. This contains the UDP header but not the IPv4 header.
    pub fn as_bytes(&self) -> &Bytes {
        &self.data
    }

    /// Get the source port.
    pub fn source_port(&self) -> u16 {
        ((self.data[0] as u16) << 8) | (self.data[1] as u16)
    }

    /// Get the destination port.
    pub fn destination_port(&self) -> u16 {
        ((self.data[2] as u16) << 8) | (self.data[3] as u16)
    }

    /// The payload data of the packet.
    pub fn payload(&self) -> Bytes {
        self.data.slice_from(8)
    }

    /// Set the source port
    pub fn set_source_port(&mut self, port: u16) {
        let bytes = mem::replace(&mut self.data, Bytes::new());
        let mut bytes_mut = BytesMut::from(bytes);
        bytes_mut[0] = (port >> 8) as u8;
        bytes_mut[1] = (port & 0xff) as u8;
        self.data = bytes_mut.into();
    }

    /// Set the destination port
    pub fn set_destination_port(&mut self, port: u16) {
        let bytes = mem::replace(&mut self.data, Bytes::new());
        let mut bytes_mut = BytesMut::from(bytes);
        bytes_mut[2] = (port >> 8) as u8;
        bytes_mut[3] = (port & 0xff) as u8;
        self.data = bytes_mut.into();
    }

    /// Set the packet payload
    pub fn set_payload(&mut self, payload: Bytes) {
        let mut bytes_mut = BytesMut::with_capacity(payload.len() + 8);
        bytes_mut.extend_from_slice(&self.data[..8]);
        bytes_mut.extend_from_slice(&payload);
        self.data = bytes_mut.into();
    }
}


