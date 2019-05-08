use crate::priv_prelude::*;

/// Adds packet loss to an IPv6 connection
pub struct Ipv6PacketLoss {
    //inner: PacketLoss<Ipv6Packet>,
}

impl Ipv6PacketLoss {
    /// Spawn a `Ipv6PacketLoss` directly onto the event loop
    pub fn spawn(
        handle: &NetworkHandle,
        loss_rate: f64,
        mean_loss_duration: Duration,
        plug_a: Ipv6Plug,
        plug_b: Ipv6Plug,
    ) {
        PacketLoss::spawn(handle, loss_rate, mean_loss_duration, plug_a.into(), plug_b.into())
    }
}

