use crate::priv_prelude::*;

/// Adds packet loss to an ethernet connection
pub struct EtherPacketLoss {
    //inner: PacketLoss<EtherFrame>,
}

impl EtherPacketLoss {
    /// Spawn a `PacketLossEther` directly onto the event loop
    pub fn spawn(
        handle: &NetworkHandle,
        loss_rate: f64,
        mean_loss_duration: Duration,
        plug_a: EtherPlug,
        plug_b: EtherPlug,
    ) {
        PacketLoss::spawn(
            handle,
            loss_rate,
            mean_loss_duration,
            plug_a.into(),
            plug_b.into(),
        )
    }
}
