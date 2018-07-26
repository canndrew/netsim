use priv_prelude::*;
use futures::sync::mpsc::SendError;

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

impl Ipv6PayloadFields {
    /// Calculate the total length of an Ipv6 packet with the given fields.
    pub fn total_packet_len(&self) -> usize {
        40 + match *self {
            Ipv6PayloadFields::Udp { ref payload, .. } => 8 + payload.len(),
            Ipv6PayloadFields::Tcp { ref payload, ref fields } => {
                fields.header_len() + payload.len()
            },
        }
    }
}

pub fn set_fields(buffer: &mut [u8], fields: &Ipv6Fields) {
    buffer[0] = 0x60;
    buffer[1] = 0x00;
    buffer[2] = 0x00;
    buffer[3] = 0x00;
    let len = buffer.len() as u16;
    NetworkEndian::write_u16(&mut buffer[4..6], len);
    buffer[7] = fields.hop_limit;
    buffer[8..24].clone_from_slice(&fields.source_ip.octets());
    buffer[24..40].clone_from_slice(&fields.dest_ip.octets());
}

impl Ipv6Packet {
    /// Create a new `Ipv6Packet` with the given header fields and payload. If you are also
    /// creating the packet's payload data it can be more efficient to use
    /// `new_from_fields_recursive` and save an allocation/copy.
    pub fn new_from_fields(
        fields: &Ipv6Fields,
        payload: &Ipv6Payload,
    ) -> Ipv6Packet {
        let len = 40 + match *payload {
            Ipv6Payload::Udp(ref udp) => udp.as_bytes().len(),
            Ipv6Payload::Tcp(ref tcp) => tcp.as_bytes().len(),
            Ipv6Payload::Unknown { ref payload, .. } => payload.len(),
        };
        let mut buffer = unsafe { BytesMut::uninit(len) };
        buffer[6] = match *payload {
            Ipv6Payload::Udp(..) => 17,
            Ipv6Payload::Tcp(..) => 6,
            Ipv6Payload::Unknown { protocol, .. } => protocol,
        };

        set_fields(&mut buffer, fields);

        match *payload {
            Ipv6Payload::Udp(ref udp) => buffer[40..].clone_from_slice(udp.as_bytes()),
            Ipv6Payload::Tcp(ref tcp) => buffer[40..].clone_from_slice(tcp.as_bytes()),
            Ipv6Payload::Unknown { ref payload, .. } => buffer[40..].clone_from_slice(payload),
        }

        Ipv6Packet {
            buffer: buffer.freeze(),
        }
    }

    /// Create a new `Ipv6Packet` with the given header fields and payload fields.
    pub fn new_from_fields_recursive(
        fields: &Ipv6Fields,
        payload_fields: &Ipv6PayloadFields,
    ) -> Ipv6Packet {
        let len = payload_fields.total_packet_len();
        let mut buffer = unsafe { BytesMut::uninit(len) };
        Ipv6Packet::write_to_buffer(&mut buffer, fields, payload_fields);
        Ipv6Packet {
            buffer: buffer.freeze(),
        }
    }

    /// Create a new Ipv6 packet by writing it to the given empty buffer.
    pub fn write_to_buffer(
        buffer: &mut [u8],
        fields: &Ipv6Fields,
        payload_fields: &Ipv6PayloadFields,
    ) {
        buffer[6] = match payload_fields {
            Ipv6PayloadFields::Udp { .. } => 17,
            Ipv6PayloadFields::Tcp { .. } => 6,
        };

        set_fields(buffer, fields);

        match payload_fields {
            Ipv6PayloadFields::Udp { fields: udp_fields, ref payload } => {
                UdpPacket::write_to_buffer_v6(
                    &mut buffer[40..],
                    udp_fields,
                    fields.source_ip,
                    fields.dest_ip,
                    payload,
                );
            },
            Ipv6PayloadFields::Tcp { fields: tcp_fields, ref payload } => {
                TcpPacket::write_to_buffer_v6(
                    &mut buffer[40..],
                    tcp_fields,
                    fields.source_ip,
                    fields.dest_ip,
                    payload,
                );
            },
        }
    }

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

#[derive(Debug)]
/// An IPv6 plug
pub struct Ipv6Plug {
    inner: Plug<Ipv6Packet>,
}

impl Ipv6Plug {
    /// Create a pair of connected plugs
    pub fn new_pair() -> (Ipv6Plug, Ipv6Plug) {
        let (plug_a, plug_b) = Plug::new_pair();
        let plug_a = Ipv6Plug { inner: plug_a };
        let plug_b = Ipv6Plug { inner: plug_b };
        (plug_a, plug_b)
    }

    /// Add latency to a connection
    pub fn with_latency(
        self, 
        handle: &NetworkHandle,
        min_latency: Duration,
        mean_additional_latency: Duration,
    ) -> Ipv6Plug {
        Ipv6Plug {
            inner: self.inner.with_latency(handle, min_latency, mean_additional_latency),
        }
    }

    /// Add packet loss to a connection
    pub fn with_packet_loss(
        self,
        handle: &NetworkHandle,
        loss_rate: f64,
        mean_loss_duration: Duration,
    ) -> Ipv6Plug {
        Ipv6Plug {
            inner: self.inner.with_packet_loss(handle, loss_rate, mean_loss_duration),
        }
    }

    /// Split into sending and receiving halves
    pub fn split(self) -> (Ipv6Sender, Ipv6Receiver) {
        let (tx, rx) = self.inner.split();
        let tx = Ipv6Sender { tx };
        let rx = Ipv6Receiver { rx };
        (tx, rx)
    }

    /// Poll for incoming packets
    pub fn poll_incoming(&mut self) -> Async<Option<Ipv6Packet>> {
        self.inner.rx.poll().void_unwrap()
    }

    /// Send a packet
    pub fn unbounded_send(&mut self, packet: Ipv6Packet) -> Result<(), SendError<Ipv6Packet>> {
        self.inner.tx.unbounded_send(packet)
    }
}

impl From<Ipv6Plug> for Plug<Ipv6Packet> {
    fn from(plug: Ipv6Plug) -> Plug<Ipv6Packet> {
        plug.inner
    }
}

/// A trait for types that can be converted into an `Ipv6Plug`
pub trait IntoIpv6Plug {
    /// Convert into an `Ipv6Plug`
    fn into_ipv6_plug(self, handle: &NetworkHandle) -> Ipv6Plug;
}

impl<S> IntoIpv6Plug for S
where
    S: Stream<Item = Ipv6Packet, Error = Void>,
    S: Sink<SinkItem = Ipv6Packet, SinkError = Void>,
    S: 'static,
{
    fn into_ipv6_plug(self, handle: &NetworkHandle) -> Ipv6Plug {
        let (self_tx, self_rx) = self.split();
        let (plug_a, plug_b) = Ipv6Plug::new_pair();
        let (plug_tx, plug_rx) = plug_a.split();
        handle.spawn(self_rx.forward(plug_tx).map(|(_rx, _tx)| ()));
        handle.spawn(plug_rx.forward(self_tx).map(|(_rx, _tx)| ()));
        plug_b
    }
}

/// The sending half of an `Ipv6Plug`
pub struct Ipv6Sender {
    tx: UnboundedSender<Ipv6Packet>,
}

impl Ipv6Sender {
    /// Send a packet down the wire
    pub fn unbounded_send(&self, packet: Ipv6Packet) {
        let _ = self.tx.unbounded_send(packet);
    }
}

impl Sink for Ipv6Sender {
    type SinkItem = Ipv6Packet;
    type SinkError = Void;

    fn start_send(&mut self, packet: Ipv6Packet) -> Result<AsyncSink<Ipv6Packet>, Void> {
        Ok(self.tx.start_send(packet).unwrap_or(AsyncSink::Ready))
    }

    fn poll_complete(&mut self) -> Result<Async<()>, Void> {
        Ok(self.tx.poll_complete().unwrap_or(Async::Ready(())))
    }
}

/// The receiving half of an `Ipv6Plug`
pub struct Ipv6Receiver {
    rx: UnboundedReceiver<Ipv6Packet>,
}

impl Stream for Ipv6Receiver {
    type Item = Ipv6Packet;
    type Error = Void;

    fn poll(&mut self) -> Result<Async<Option<Ipv6Packet>>, Void> {
        self.rx.poll()
    }
}

