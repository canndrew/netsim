use priv_prelude::*;

/// An ARP packet
#[derive(Clone, PartialEq)]
pub struct ArpPacket {
    buffer: Bytes,
}

impl fmt::Debug for ArpPacket {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.fields() {
            ArpFields::Request { .. } => {
                f
                .debug_struct("ArpPacket::Request")
                .field("source_mac", &self.source_mac())
                .field("source_ip", &self.source_ip())
                .field("dest_mac", &self.dest_mac())
                .field("dest_ip", &self.dest_ip())
                .finish()
            },
            ArpFields::Response { .. } => {
                f
                .debug_struct("ArpPacket::Response")
                .field("source_mac", &self.source_mac())
                .field("source_ip", &self.source_ip())
                .field("dest_mac", &self.dest_mac())
                .field("dest_ip", &self.dest_ip())
                .finish()
            },
        }
    }
}

/// The fields of an ARP packet.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ArpFields {
    /// An ARP request
    Request {
        /// The MAC address of the sender.
        source_mac: MacAddr,
        /// The Ipv4 address of the sender.
        source_ip: Ipv4Addr,
        /// The Ipv4 address of the peer whose MAC address we are requesting.
        dest_ip: Ipv4Addr,
    },
    /// An ARP response
    Response {
        /// The MAC address of the sender. 
        source_mac: MacAddr,
        /// The Ipv4 address of the sender.
        source_ip: Ipv4Addr,
        /// The MAC address of the receiver.
        dest_mac: MacAddr,
        /// The Ipv4 address of the receiver.
        dest_ip: Ipv4Addr,
    },
}

fn set_fields(buffer: &mut [u8], fields: ArpFields) {
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
            buffer[6..8].clone_from_slice(&[0x00, 0x02]);
            buffer[8..14].clone_from_slice(source_mac.as_bytes());
            buffer[14..18].clone_from_slice(&source_ip.octets());
            buffer[18..24].clone_from_slice(dest_mac.as_bytes());
            buffer[24..28].clone_from_slice(&dest_ip.octets());
        },
    }
}

impl ArpPacket {
    /// Create a new `ArpPacket` given the description provided by `fields`.
    pub fn new_from_fields(fields: ArpFields) -> ArpPacket {
        let mut buffer = unsafe { BytesMut::uninit(28) };
        set_fields(&mut buffer, fields);
        ArpPacket {
            buffer: buffer.freeze(),
        }
    }

    /// Write an ARP packet described by `fields` to the given buffer.
    pub fn write_to_buffer(
        buffer: &mut [u8],
        fields: ArpFields,
    ) {
        set_fields(buffer, fields);
    }

    /// Parse an ARP packet from the given buffer.
    pub fn from_bytes(buffer: Bytes) -> ArpPacket {
        ArpPacket {
            buffer,
        }
    }

    /// Parse the ARP packet into an `ArpFields`.
    pub fn fields(&self) -> ArpFields {
        match NetworkEndian::read_u16(&self.buffer[6..8]) {
            0x0001 => ArpFields::Request {
                source_mac: self.source_mac(),
                source_ip: self.source_ip(),
                dest_ip: self.dest_ip(),
            },
            0x0002 => ArpFields::Response {
                source_mac: self.source_mac(),
                source_ip: self.source_ip(),
                dest_mac: self.dest_mac(),
                dest_ip: self.dest_ip(),
            },
            x => panic!("unexpected ARP operation type (0x{:04x})", x),
        }
    }

    /// Get the MAC address of the sender.
    pub fn source_mac(&self) -> MacAddr {
        MacAddr::from_bytes(&self.buffer[8..14])
    }

    /// Get the IP address of the sender.
    pub fn source_ip(&self) -> Ipv4Addr {
        Ipv4Addr::from(slice_assert_len!(4, &self.buffer[14..18]))
    }

    /// Get the MAC address of the destination.
    pub fn dest_mac(&self) -> MacAddr {
        MacAddr::from_bytes(&self.buffer[18..24])
    }

    /// Get the IP address of the destination.
    pub fn dest_ip(&self) -> Ipv4Addr {
        Ipv4Addr::from(slice_assert_len!(4, &self.buffer[24..28]))
    }

    /// Return the underlying byte buffer of this packet.
    pub fn as_bytes(&self) -> &Bytes {
        &self.buffer
    }
}

