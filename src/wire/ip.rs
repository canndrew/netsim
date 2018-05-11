use priv_prelude::*;
use future_utils;
use futures::future::Loop;

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

/// One end of an IP connection that can be used to read/write packets to/from the other end.
#[derive(Debug)]
pub struct IpPlug {
    /// The sender
    pub tx: UnboundedSender<IpPacket>,
    /// The receiver.
    pub rx: UnboundedReceiver<IpPacket>,
}

impl IpPlug {
    /// Create a new Ip connection, connecting the two returned plugs.
    pub fn new_wire() -> (IpPlug, IpPlug) {
        let (a_tx, b_rx) = future_utils::mpsc::unbounded();
        let (b_tx, a_rx) = future_utils::mpsc::unbounded();
        let a = IpPlug {
            tx: a_tx,
            rx: a_rx,
        };
        let b = IpPlug {
            tx: b_tx,
            rx: b_rx,
        };
        (a, b)
    }

    /// Adapt the plug to an IPv4 plug, dropping all incoming IPv6 packets.
    pub fn into_ipv4_plug(self, handle: &Handle) -> Ipv4Plug {
        let (ipv4_plug_a, ipv4_plug_b) = Ipv4Plug::new_wire();

        let IpPlug { tx: ip_tx, rx: ip_rx } = self;
        let Ipv4Plug { tx: ipv4_tx, rx: ipv4_rx } = ipv4_plug_a;
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
        let (ipv6_plug_a, ipv6_plug_b) = Ipv6Plug::new_wire();

        let IpPlug { tx: ip_tx, rx: ip_rx } = self;
        let Ipv6Plug { tx: ipv6_tx, rx: ipv6_rx } = ipv6_plug_a;
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
}

