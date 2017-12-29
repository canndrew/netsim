use bytes::Bytes;

#[derive(Debug)]
pub struct UdpPacket {
    data: Bytes,
}

impl UdpPacket {
    pub fn new(data: Bytes) -> UdpPacket {
        UdpPacket {
            data,
        }
    }

    pub fn source_port(&self) -> u16 {
        ((self.data[0] as u16) << 8) | (self.data[1] as u16)
    }

    pub fn destination_port(&self) -> u16 {
        ((self.data[2] as u16) << 8) | (self.data[3] as u16)
    }

    pub fn payload(&self) -> Bytes {
        self.data.slice_from(8)
    }
}

