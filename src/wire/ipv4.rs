use priv_prelude::*;
use super::*;
use futures::sync::mpsc::SendError;

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
            Ipv4PayloadFields::Udp { fields: ref udp_fields, ref payload } => {
                UdpPacket::write_to_buffer_v4(
                    &mut buffer[header_len..],
                    udp_fields,
                    fields.source_ip,
                    fields.dest_ip,
                    payload,
                );
            },
            Ipv4PayloadFields::Tcp { fields: ref tcp_fields, ref payload } => {
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

    /// Unwrap's this Ipv4 packet's inner TCP packet if possible.
    pub fn to_tcp_packet(&self) -> Option<TcpPacket> {
        match self.payload() {
            Ipv4Payload::Tcp(tcp_packet) => Some(tcp_packet),
            _ => None,
        }
    }

    /// Unwrap's this Ipv4 packet's inner UDP packet if possible.
    pub fn to_udp_packet(&self) -> Option<UdpPacket> {
        match self.payload() {
            Ipv4Payload::Udp(udp_packet) => Some(udp_packet),
            _ => None,
        }
    }

    /// Unwrap's this Ipv4 packet's inner ICMP packet if possible.
    pub fn to_icmpv4_packet(&self) -> Option<Icmpv4Packet> {
        match self.payload() {
            Ipv4Payload::Icmp(icmpv4_packet) => Some(icmpv4_packet),
            _ => None,
        }
    }
}

#[derive(Debug)]
/// An IPv4 plug
pub struct Ipv4Plug {
    inner: Plug<Ipv4Packet>,
}

impl Ipv4Plug {
    /// Create a pair of connected plugs
    pub fn new_pair() -> (Ipv4Plug, Ipv4Plug) {
        let (plug_a, plug_b) = Plug::new_pair();
        let plug_a = Ipv4Plug { inner: plug_a };
        let plug_b = Ipv4Plug { inner: plug_b };
        (plug_a, plug_b)
    }

    /// Add latency to a connection
    pub fn with_latency(
        self, 
        handle: &NetworkHandle,
        min_latency: Duration,
        mean_additional_latency: Duration,
    ) -> Ipv4Plug {
        Ipv4Plug {
            inner: self.inner.with_latency(handle, min_latency, mean_additional_latency),
        }
    }

    /// Add packet loss to a connection
    pub fn with_packet_loss(
        self,
        handle: &NetworkHandle,
        loss_rate: f64,
        mean_loss_duration: Duration,
    ) -> Ipv4Plug {
        Ipv4Plug {
            inner: self.inner.with_packet_loss(handle, loss_rate, mean_loss_duration),
        }
    }

    /// Split into sending and receiving halves
    pub fn split(self) -> (Ipv4Sender, Ipv4Receiver) {
        let (tx, rx) = self.inner.split();
        let tx = Ipv4Sender { tx };
        let rx = Ipv4Receiver { rx };
        (tx, rx)
    }

    /// Add extra hops to the end of this connection. Packets travelling through this plug will
    /// have their TTL decremented by the amount of hops given.
    pub fn with_hops(
        self,
        handle: &NetworkHandle,
        num_hops: u32,
    ) -> Ipv4Plug {
        let mut plug = self;
        for _ in 0..num_hops {
            let (plug_0, plug_1) = Ipv4Plug::new_pair();
            Ipv4Hop::spawn(handle, plug, plug_0);
            plug = plug_1;
        }
        plug
    }

    /// Poll for incoming packets
    pub fn poll_incoming(&mut self) -> Async<Option<Ipv4Packet>> {
        self.inner.rx.poll().void_unwrap()
    }

    /// Send a packet
    pub fn unbounded_send(&mut self, packet: Ipv4Packet) -> Result<(), SendError<Ipv4Packet>> {
        self.inner.tx.unbounded_send(packet)
    }
}

impl From<Ipv4Plug> for Plug<Ipv4Packet> {
    fn from(plug: Ipv4Plug) -> Plug<Ipv4Packet> {
        plug.inner
    }
}

impl Stream for Ipv4Plug {
    type Item = Ipv4Packet;
    type Error = Void;

    fn poll(&mut self) -> Result<Async<Option<Ipv4Packet>>, Void> {
        self.inner.poll()
    }
}

impl Sink for Ipv4Plug {
    type SinkItem = Ipv4Packet;
    type SinkError = Void;

    fn start_send(&mut self, packet: Ipv4Packet) -> Result<AsyncSink<Ipv4Packet>, Void> {
        self.inner.start_send(packet)
    }

    fn poll_complete(&mut self) -> Result<Async<()>, Void> {
        self.inner.poll_complete()
    }
}

/// A trait for types that can be converted into an `Ipv4Plug`
pub trait IntoIpv4Plug {
    /// Convert into an `Ipv4Plug`
    fn into_ipv4_plug(self, handle: &NetworkHandle) -> Ipv4Plug;
}

impl<S> IntoIpv4Plug for S
where
    S: Stream<Item = Ipv4Packet, Error = Void>,
    S: Sink<SinkItem = Ipv4Packet, SinkError = Void>,
    S: Send + 'static,
{
    fn into_ipv4_plug(self, handle: &NetworkHandle) -> Ipv4Plug {
        let (self_tx, self_rx) = self.split();
        let (plug_a, plug_b) = Ipv4Plug::new_pair();
        let (plug_tx, plug_rx) = plug_a.split();
        handle.spawn(self_rx.forward(plug_tx).map(|(_rx, _tx)| ()));
        handle.spawn(plug_rx.forward(self_tx).map(|(_rx, _tx)| ()));
        plug_b
    }
}

/// The sending half of an `Ipv4Plug`
pub struct Ipv4Sender {
    tx: UnboundedSender<Ipv4Packet>,
}

impl Ipv4Sender {
    /// Send a packet down the wire
    pub fn unbounded_send(&self, packet: Ipv4Packet) {
        let _ = self.tx.unbounded_send(packet);
    }
}

impl Sink for Ipv4Sender {
    type SinkItem = Ipv4Packet;
    type SinkError = Void;

    fn start_send(&mut self, packet: Ipv4Packet) -> Result<AsyncSink<Ipv4Packet>, Void> {
        Ok(self.tx.start_send(packet).unwrap_or(AsyncSink::Ready))
    }

    fn poll_complete(&mut self) -> Result<Async<()>, Void> {
        Ok(self.tx.poll_complete().unwrap_or(Async::Ready(())))
    }
}

/// The receiving half of an `Ipv4Plug`
pub struct Ipv4Receiver {
    rx: UnboundedReceiver<Ipv4Packet>,
}

impl Stream for Ipv4Receiver {
    type Item = Ipv4Packet;
    type Error = Void;

    fn poll(&mut self) -> Result<Async<Option<Ipv4Packet>>, Void> {
        self.rx.poll()
    }
}

