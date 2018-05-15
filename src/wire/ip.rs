use priv_prelude::*;
use futures::future::Loop;
use futures::sync::mpsc::SendError;

#[derive(Clone, PartialEq)]
/// An IP packet.
pub enum IpPacket {
    /// IPv4
    V4(Ipv4Packet),
    /// IPv6
    V6(Ipv6Packet),
}

impl fmt::Debug for IpPacket {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            IpPacket::V4(packet) => packet.fmt(f),
            IpPacket::V6(packet) => packet.fmt(f),
        }
    }
}

impl IpPacket {
    /// Parse an IP packet from a byte buffer
    pub fn from_bytes(buffer: Bytes) -> IpPacket {
        match buffer[0] >> 4 {
            4 => IpPacket::V4(Ipv4Packet::from_bytes(buffer)),
            6 => IpPacket::V6(Ipv6Packet::from_bytes(buffer)),
            v => panic!("invalid IP version number: {}", v),
        }
    }

    /// Get a reference to the packet's underlying byte buffer
    pub fn as_bytes(&self) -> &Bytes {
        match self {
            IpPacket::V4(packet) => packet.as_bytes(),
            IpPacket::V6(packet) => packet.as_bytes(),
        }
    }

    /// Get the packet's source IP address
    pub fn source_ip(&self) -> IpAddr {
        match self {
            IpPacket::V4(packet) => IpAddr::V4(packet.source_ip()),
            IpPacket::V6(packet) => IpAddr::V6(packet.source_ip()),
        }
    }

    /// Get the packet's destination IP address
    pub fn dest_ip(&self) -> IpAddr {
        match self {
            IpPacket::V4(packet) => IpAddr::V4(packet.dest_ip()),
            IpPacket::V6(packet) => IpAddr::V6(packet.dest_ip()),
        }
    }
}

#[derive(Debug)]
/// An IP plug
pub struct IpPlug {
    inner: Plug<IpPacket>,
}

impl IpPlug {
    /// Create a pair of connected plugs
    pub fn new_pair() -> (IpPlug, IpPlug) {
        let (plug_a, plug_b) = Plug::new_pair();
        let plug_a = IpPlug { inner: plug_a };
        let plug_b = IpPlug { inner: plug_b };
        (plug_a, plug_b)
    }

    /// Add latency to a connection
    pub fn with_latency(
        self, 
        handle: &Handle,
        min_latency: Duration,
        mean_additional_latency: Duration,
    ) -> IpPlug {
        IpPlug {
            inner: self.inner.with_latency(handle, min_latency, mean_additional_latency),
        }
    }

    /// Add packet loss to a connection
    pub fn with_packet_loss(
        self,
        handle: &Handle,
        loss_rate: f64,
        mean_loss_duration: Duration,
    ) -> IpPlug {
        IpPlug {
            inner: self.inner.with_packet_loss(handle, loss_rate, mean_loss_duration),
        }
    }

    /// Adapt the plug to an IPv4 plug, dropping all incoming IPv6 packets.
    pub fn into_ipv4_plug(self, handle: &Handle) -> Ipv4Plug {
        let (ipv4_plug_a, ipv4_plug_b) = Ipv4Plug::new_pair();

        let (ip_tx, ip_rx) = self.split();
        let (ipv4_tx, ipv4_rx) = ipv4_plug_a.split();
        handle.spawn({
            future::loop_fn((ipv4_tx, ip_rx), move |(ipv4_tx, ip_rx)| {
                ip_rx
                .into_future()
                .map_err(|(v, _)| v)
                .map(move |(ip_packet_opt, ip_rx)| {
                    match ip_packet_opt {
                        Some(IpPacket::V4(ipv4_packet)) => {
                            match ipv4_tx.unbounded_send(ipv4_packet) {
                                Ok(()) => Loop::Continue((ipv4_tx, ip_rx)),
                                Err(..) => Loop::Break(()),
                            }
                        },
                        Some(..) => Loop::Continue((ipv4_tx, ip_rx)),
                        None => Loop::Break(()),
                    }
                })
            })
            .infallible()
        });
        handle.spawn({
            future::loop_fn((ip_tx, ipv4_rx), move |(ip_tx, ipv4_rx)| {
                ipv4_rx
                .into_future()
                .map_err(|(v, _)| v)
                .map(move |(ipv4_packet_opt, ipv4_rx)| {
                    match ipv4_packet_opt {
                        Some(ipv4_packet) => {
                            let ip_packet = IpPacket::V4(ipv4_packet);
                            match ip_tx.unbounded_send(ip_packet) {
                                Ok(()) => Loop::Continue((ip_tx, ipv4_rx)),
                                Err(..) => Loop::Break(()),
                            }
                        },
                        None => Loop::Break(()),
                    }
                })
            })
            .infallible()
        });
        ipv4_plug_b
    }

    /// Adapt the plug to an IPv6 plug, dropping all incoming IPv6 packets.
    pub fn into_ipv6_plug(self, handle: &Handle) -> Ipv6Plug {
        let (ipv6_plug_a, ipv6_plug_b) = Ipv6Plug::new_pair();

        let (ip_tx, ip_rx) = self.split();
        let (ipv6_tx, ipv6_rx) = ipv6_plug_a.split();
        handle.spawn({
            future::loop_fn((ipv6_tx, ip_rx), move |(ipv6_tx, ip_rx)| {
                ip_rx
                .into_future()
                .map_err(|(v, _)| v)
                .map(move |(ip_packet_opt, ip_rx)| {
                    match ip_packet_opt {
                        Some(IpPacket::V6(ipv6_packet)) => {
                            match ipv6_tx.unbounded_send(ipv6_packet) {
                                Ok(()) => Loop::Continue((ipv6_tx, ip_rx)),
                                Err(..) => Loop::Break(()),
                            }
                        },
                        Some(..) => Loop::Continue((ipv6_tx, ip_rx)),
                        None => Loop::Break(()),
                    }
                })
            })
            .infallible()
        });
        handle.spawn({
            future::loop_fn((ip_tx, ipv6_rx), move |(ip_tx, ipv6_rx)| {
                ipv6_rx
                .into_future()
                .map_err(|(v, _)| v)
                .map(move |(ipv6_packet_opt, ipv6_rx)| {
                    match ipv6_packet_opt {
                        Some(ipv6_packet) => {
                            let ip_packet = IpPacket::V6(ipv6_packet);
                            match ip_tx.unbounded_send(ip_packet) {
                                Ok(()) => Loop::Continue((ip_tx, ipv6_rx)),
                                Err(..) => Loop::Break(()),
                            }
                        },
                        None => Loop::Break(()),
                    }
                })
            })
            .infallible()
        });
        ipv6_plug_b
    }

    /// Split into sending and receiving halves
    pub fn split(self) -> (UnboundedSender<IpPacket>, UnboundedReceiver<IpPacket>) {
        self.inner.split()
    }

    /// Poll for incoming packets
    pub fn poll_incoming(&mut self) -> Async<Option<IpPacket>> {
        self.inner.rx.poll().void_unwrap()
    }

    /// Send a packet
    pub fn unbounded_send(&mut self, packet: IpPacket) -> Result<(), SendError<IpPacket>> {
        self.inner.tx.unbounded_send(packet)
    }
}

impl From<IpPlug> for Plug<IpPacket> {
    fn from(plug: IpPlug) -> Plug<IpPacket> {
        plug.inner
    }
}

