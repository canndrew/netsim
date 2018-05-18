use priv_prelude::*;

/// Adds packet loss to an IP connection
pub struct IpPacketLoss {
    //inner: PacketLoss<IpPacket>,
}

impl IpPacketLoss {
    /// Spawn a `IpPacketLoss` directly onto the event loop
    pub fn spawn(
        handle: &NetworkHandle,
        loss_rate: f64,
        mean_loss_duration: Duration,
        plug_a: IpPlug,
        plug_b: IpPlug,
    ) {
        PacketLoss::spawn(handle, loss_rate, mean_loss_duration, plug_a.into(), plug_b.into())
    }
}

