use priv_prelude::*;
use super::*;
use future_utils;

/// An Ipv4 packet.
#[derive(Clone, PartialEq)]
pub struct Ipv4Packet {
    buffer: Bytes,
}

impl fmt::Debug for Ipv4Packet {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let payload = self.payload();

        if self.verify_checksum() {
            f
            .debug_struct("Ipv4Packet")
            .field("source_ip", &self.source_ip())
            .field("dest_ip", &self.dest_ip())
            .field("ttl", &self.ttl())
            .field("payload", match payload {
                Ipv4Payload::Udp(ref udp) => {
                    if udp.verify_checksum_v4(self.source_ip(), self.dest_ip()) {
                        udp
                    } else {
                        &"INVALID UDP packet"
                    }
                },
                Ipv4Payload::Tcp(ref tcp) => {
                    if tcp.verify_checksum_v4(self.source_ip(), self.dest_ip()) {
                        tcp
                    } else {
                        &"INVALID TCP packet"
                    }
                },
                Ipv4Payload::Icmp(ref icmp) => icmp,
                Ipv4Payload::Unknown { .. } => &payload,
            })
            .finish()
        } else {
            write!(f, "INVALID Ipv4Packet")
        }
    }
}

/// The header fields of an Ipv4 packet.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ipv4Fields {
    /// IP address of the sender.
    pub source_ip: Ipv4Addr,
    /// IP address of the destination.
    pub dest_ip: Ipv4Addr,
    /// Packet's time-to-live field. ie. hop count.
    pub ttl: u8,
}

impl Ipv4Fields {
    /// Parse an IPv4 header from a byte buffer
    pub fn from_bytes(buffer: &[u8]) -> Ipv4Fields {
        let packet = Ipv4Packet { buffer: Bytes::from(&buffer[..20]) };
        packet.fields()
    }

    /// Get the size of the IPv4 header represented by this `Ipv4Fields`
    pub fn header_len(&self) -> usize {
        20
    }
}

/// The payload of an Ipv4 packet
#[derive(Debug, Clone)]
pub enum Ipv4Payload {
    /// A UDP payload
    Udp(UdpPacket),
    /// A TCP payload
    Tcp(TcpPacket),
    /// An ICMP payload
    Icmp(Icmpv4Packet),
    /// Payload of some unrecognised protocol.
    Unknown {
        /// The payload's protocol number.
        protocol: u8,
        /// The payload data.
        payload: Bytes,
    },
}

impl Ipv4Payload {
    /// Get the length of the payload, in bytes
    pub fn payload_len(&self) -> usize {
        match *self {
            Ipv4Payload::Udp(ref udp) => udp.as_bytes().len(),
            Ipv4Payload::Tcp(ref tcp) => tcp.as_bytes().len(),
            Ipv4Payload::Icmp(ref icmp) => icmp.as_bytes().len(),
            Ipv4Payload::Unknown { ref payload, .. } => payload.len(),
        }
    }
}

/// The payload of an Ipv4 packet. Can be used to construct an Ipv4 packet and its contents
/// simultaneously.
#[derive(Debug, Clone)]
pub enum Ipv4PayloadFields {
    /// A UDP packet
    Udp {
        /// The header fields of the UDP packet
        fields: UdpFields,
        /// The UDP payload data.
        payload: Bytes,
    },
    /// A TCP packet
    Tcp {
        /// The header fields of the TCP packet
        fields: TcpFields,
        /// The TCP payload data.
        payload: Bytes,
    },
    /// An ICMP packet
    Icmp {
        /// The kind of ICMP packet
        kind: Icmpv4PacketKind,
    }
}

impl Ipv4PayloadFields {
    /// Calculate the total length of an Ipv4 packet with the given fields.
    pub fn payload_len(&self) -> usize {
        match *self {
            Ipv4PayloadFields::Udp { ref payload, .. } => 8 + payload.len(),
            Ipv4PayloadFields::Tcp { ref payload, ref fields } => {
                fields.header_len() + payload.len()
            },
            Ipv4PayloadFields::Icmp { ref kind } => kind.buffer_len(),
        }
    }
}

pub fn set_fields(buffer: &mut [u8], fields: Ipv4Fields) {
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

    let checksum = !checksum::data(&buffer[0..20]);
    NetworkEndian::write_u16(&mut buffer[10..12], checksum);
}

impl Ipv4Packet {
    /// Create a new `Ipv4Packet` with the given header fields and payload. If you are also
    /// creating the packet's payload data it can be more efficient to use
    /// `new_from_fields_recursive` and save an allocation/copy.
    pub fn new_from_fields(
        fields: Ipv4Fields,
        payload: &Ipv4Payload,
    ) -> Ipv4Packet {
        let header_len = fields.header_len();
        let len = header_len + payload.payload_len();
        let mut buffer = unsafe { BytesMut::uninit(len) };
        buffer[9] = match *payload {
            Ipv4Payload::Udp(..) => 17,
            Ipv4Payload::Tcp(..) => 6,
            Ipv4Payload::Icmp(..) => 1,
            Ipv4Payload::Unknown { protocol, .. } => protocol,
        };

        set_fields(&mut buffer, fields);

        match *payload {
            Ipv4Payload::Udp(ref udp) => buffer[header_len..].clone_from_slice(udp.as_bytes()),
            Ipv4Payload::Tcp(ref tcp) => buffer[header_len..].clone_from_slice(tcp.as_bytes()),
            Ipv4Payload::Icmp(ref icmp) => buffer[header_len..].clone_from_slice(icmp.as_bytes()),
            Ipv4Payload::Unknown { ref payload, .. } => buffer[header_len..].clone_from_slice(payload),
        }

        Ipv4Packet {
            buffer: buffer.freeze(),
        }
    }
    
    /// Create a new `Ipv4Packet` with the given header fields and payload fields.
    pub fn new_from_fields_recursive(
        fields: Ipv4Fields,
        payload_fields: Ipv4PayloadFields,
    ) -> Ipv4Packet {
        let len = fields.header_len() + payload_fields.payload_len();
        let mut buffer = unsafe { BytesMut::uninit(len) };
        Ipv4Packet::write_to_buffer(&mut buffer, fields, payload_fields);
        Ipv4Packet {
            buffer: buffer.freeze()
        }
    }

    /// Create a new Ipv4 packet by writing it to the given empty buffer.
    pub fn write_to_buffer(
        buffer: &mut [u8],
        fields: Ipv4Fields,
        payload_fields: Ipv4PayloadFields,
    ) {
        let header_len = fields.header_len();

        buffer[9] = match payload_fields {
            Ipv4PayloadFields::Udp { .. } => 17,
            Ipv4PayloadFields::Tcp { .. } => 6,
            Ipv4PayloadFields::Icmp { .. } => 1,
        };

        set_fields(buffer, fields);

        match payload_fields {
            Ipv4PayloadFields::Udp { fields: udp_fields, payload } => {
                UdpPacket::write_to_buffer_v4(
                    &mut buffer[header_len..],
                    udp_fields,
                    fields.source_ip,
                    fields.dest_ip,
                    payload,
                );
            },
            Ipv4PayloadFields::Tcp { fields: tcp_fields, payload } => {
                TcpPacket::write_to_buffer_v4(
                    &mut buffer[header_len..],
                    tcp_fields,
                    fields.source_ip,
                    fields.dest_ip,
                    payload,
                );
            },
            Ipv4PayloadFields::Icmp { kind } => {
                Icmpv4Packet::write_to_buffer(
                    &mut buffer[header_len..],
                    kind,
                );
            },
        }
    }

    /// Parse an Ipv4 packet from the given buffer.
    pub fn from_bytes(buffer: Bytes) -> Ipv4Packet {
        Ipv4Packet {
            buffer,
        }
    }

    /// Get the header of fields of this packet.
    pub fn fields(&self) -> Ipv4Fields {
        Ipv4Fields {
            source_ip: self.source_ip(),
            dest_ip: self.dest_ip(),
            ttl: self.ttl(),
        }
    }

    /// Set the packet's header fields.
    pub fn set_fields(&mut self, fields: Ipv4Fields) {
        let buffer = mem::replace(&mut self.buffer, Bytes::new());
        let mut buffer = BytesMut::from(buffer);
        set_fields(&mut buffer, fields);
        self.buffer = buffer.freeze();
    }

    /// Get the source Ipv4 address.
    pub fn source_ip(&self) -> Ipv4Addr {
        Ipv4Addr::from(slice_assert_len!(4, &self.buffer[12..16]))
    }

    /// Get the destination Ipv4 address.
    pub fn dest_ip(&self) -> Ipv4Addr {
        Ipv4Addr::from(slice_assert_len!(4, &self.buffer[16..20]))
    }

    /// Get the hop count/time-to-live of this packet.
    pub fn ttl(&self) -> u8 {
        self.buffer[8]
    }

    /// Get the length of the IPv4 packet header
    pub fn header_len(&self) -> usize {
        ((self.buffer[0] & 0x0f) as usize) * 4
    }

    /// Get the packet's payload
    pub fn payload(&self) -> Ipv4Payload {
        let header_len = self.header_len();
        match self.buffer[9] {
            17 => Ipv4Payload::Udp(UdpPacket::from_bytes(self.buffer.slice_from(header_len))),
            6 => Ipv4Payload::Tcp(TcpPacket::from_bytes(self.buffer.slice_from(header_len))),
            1 => Ipv4Payload::Icmp(Icmpv4Packet::from_bytes(self.buffer.slice_from(header_len))),
            p => Ipv4Payload::Unknown {
                protocol: p,
                payload: self.buffer.slice_from(header_len),
            },
        }
    }

    /// Returns the underlying packet data.
    pub fn as_bytes(&self) -> &Bytes {
        &self.buffer
    }

    /// Consume the packet and return the underlying buffer
    pub fn into_bytes(self) -> Bytes {
        self.buffer
    }

    /// Check that this packet has a valid checksum.
    pub fn verify_checksum(&self) -> bool {
        let header_len = self.header_len();
        checksum::data(&self.buffer[..header_len]) == !0
    }
}

/// One end of an Ipv4 connection that can be used to read/write packets to/from the other end.
#[derive(Debug)]
pub struct Ipv4Plug {
    /// The sender
    pub tx: UnboundedSender<Ipv4Packet>,
    /// The receiver.
    pub rx: UnboundedReceiver<Ipv4Packet>,
}

impl Ipv4Plug {
    /// Create a new Ipv4 connection, connecting the two returned plugs.
    pub fn new_wire() -> (Ipv4Plug, Ipv4Plug) {
        let (a_tx, b_rx) = future_utils::mpsc::unbounded();
        let (b_tx, a_rx) = future_utils::mpsc::unbounded();
        let a = Ipv4Plug {
            tx: a_tx,
            rx: a_rx,
        };
        let b = Ipv4Plug {
            tx: b_tx,
            rx: b_rx,
        };
        (a, b)
    }

    /// Add latency to the end of this connection.
    ///
    /// `min_latency` is the baseline for the amount of delay added to a packet travelling on this
    /// connection. `mean_additional_latency` controls the amount of extra, random latency added to
    /// any given packet on this connection. A non-zero `mean_additional_latency` can cause packets
    /// to be re-ordered.
    pub fn with_latency(
        self, 
        handle: &Handle,
        min_latency: Duration,
        mean_additional_latency: Duration,
    ) -> Ipv4Plug {
        let (plug_0, plug_1) = Ipv4Plug::new_wire();
        LatencyV4::spawn(handle, min_latency, mean_additional_latency, self, plug_0);
        plug_1
    }

    /// Add extra hops to the end of this connection. Packets travelling through this plug will
    /// have their TTL decremented by the amount of hops given.
    pub fn with_hops(
        self,
        handle: &Handle,
        num_hops: u32,
    ) -> Ipv4Plug {
        let mut plug = self;
        for _ in 0..num_hops {
            let (plug_0, plug_1) = Ipv4Plug::new_wire();
            HopV4::spawn(handle, plug, plug_0);
            plug = plug_1;
        }
        plug
    }

    /// Add packet loss to the connection. Loss happens in burst, rather than on an individual
    /// packet basis. `mean_loss_duration` controls the burstiness of the loss.
    pub fn with_packet_loss(
        self,
        handle: &Handle,
        loss_rate: f64,
        mean_loss_duration: Duration,
    ) -> Ipv4Plug {
        let (plug_0, plug_1) = Ipv4Plug::new_wire();
        PacketLossV4::spawn(handle, loss_rate, mean_loss_duration, self, plug_0);
        plug_1
    }
}

