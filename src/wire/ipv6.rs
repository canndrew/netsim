use priv_prelude::*;

/// An IPv6 packet
#[derive(Clone, PartialEq)]
pub struct Ipv6Packet {
    buffer: Bytes,
}

impl fmt::Debug for Ipv6Packet {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let payload = self.payload();

        f
        .debug_struct("Ipv6Packet")
        .field("source_ip", &self.source_ip())
        .field("dest_ip", &self.dest_ip())
        .field("hop_limit", &self.hop_limit())
        .field("payload", match payload {
            Ipv6Payload::Udp(ref udp) => {
                if udp.verify_checksum_v6(self.source_ip(), self.dest_ip()) {
                    udp
                } else {
                    &"INVALID UDP packet"
                }
            },
            Ipv6Payload::Tcp(ref tcp) => {
                if tcp.verify_checksum_v6(self.source_ip(), self.dest_ip()) {
                    tcp
                } else {
                    &"INVALID TCP packet"
                }
            },
            Ipv6Payload::Unknown { .. } => &payload,
        })
        .finish()
    }
}

/// The header fields of an IPv6 packet
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ipv6Fields {
    /// The packet source IP
    pub source_ip: Ipv6Addr,
    /// The packet destination IP
    pub dest_ip: Ipv6Addr,
    /// The packet hop limit (ie. TTL)
    pub hop_limit: u8,
}

/// The payload of an IPv6 packet
#[derive(Debug, Clone)]
pub enum Ipv6Payload {
    /// A UDP payload
    Udp(UdpPacket),
    /// A TCP payload
    Tcp(TcpPacket),
    /// Payload of some unrecognised protocol
    Unknown {
        /// The payload's protocol number
        protocol: u8,
        /// The payload data
        payload: Bytes,
    }
}

/// The payload of an IPv6 packet. Can be used to construct an IPv6 packet and its contents
/// simultaneously.
#[derive(Debug, Clone)]
pub enum Ipv6PayloadFields {
    /// A UDP packet
    Udp {
        /// The header fields of the UDP packet.
        fields: UdpFields,
        /// The UDP payload data.
        payload: Bytes,
    },
    /// A TCP packet
    Tcp {
        /// The header fields of the TCP packet.
        fields: TcpFields,
        /// The TCP payload data.
        payload: Bytes,
    },
}

impl Ipv6Packet {
    /// Parse an IPv6 packet from a byte buffer
    pub fn from_bytes(buffer: Bytes) -> Ipv6Packet {
        Ipv6Packet {
            buffer,
        }
    }

    /// Get the source IP of the packet.
    pub fn source_ip(&self) -> Ipv6Addr {
        Ipv6Addr::from(slice_assert_len!(16, &self.buffer[8..24]))
    }

    /// Get the destination IP of the packet.
    pub fn dest_ip(&self) -> Ipv6Addr {
        Ipv6Addr::from(slice_assert_len!(16, &self.buffer[24..40]))
    }

    /// Get the hop limit of the packet
    pub fn hop_limit(&self) -> u8 {
        self.buffer[7]
    }

    /// Get the packet's payload
    pub fn payload(&self) -> Ipv6Payload {
        match self.buffer[6] {
            17 => Ipv6Payload::Udp(UdpPacket::from_bytes(self.buffer.slice_from(40))),
            6 => Ipv6Payload::Tcp(TcpPacket::from_bytes(self.buffer.slice_from(40))),
            p => Ipv6Payload::Unknown {
                protocol: p,
                payload: self.buffer.slice_from(40),
            },
        }
    }

    /// Get a reference to the packets internal byte buffer
    pub fn as_bytes(&self) -> &Bytes {
        &self.buffer
    }

    /// Consume the packet and return the underlying buffer
    pub fn into_bytes(self) -> Bytes {
        self.buffer
    }
}

