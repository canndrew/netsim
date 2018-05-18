use priv_prelude::*;

/// Adds packet loss to an IPv4 connection
pub struct PacketLossV4 {
    //inner: PacketLoss<Ipv4Packet>,
}

impl PacketLossV4 {
    /// Spawn a `PacketLossV4` directly onto the event loop
    pub fn spawn(
        handle: &NetworkHandle,
        loss_rate: f64,
        mean_loss_duration: Duration,
        plug_a: Ipv4Plug,
        plug_b: Ipv4Plug,
    ) {
        PacketLoss::spawn(handle, loss_rate, mean_loss_duration, plug_a.into(), plug_b.into())
    }
}

