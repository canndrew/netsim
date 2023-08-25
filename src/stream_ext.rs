use crate::priv_prelude::*;

pub trait PacketStreamExt: Stream<Item = io::Result<Box<IpPacket>>> {
    fn with_delay(
        self,
        min_delay: Duration,
        mean_additional_delay: Duration,
    ) -> crate::adapter::Delay<Self>
    where
        Self: Sized;

    fn with_loss(
        self,
        loss_rate: f64,
        jitter_period: Duration,
    ) -> crate::adapter::Loss<Self>
    where
        Self: Sized;
}

impl<S> PacketStreamExt for S
where
    S: Stream<Item = io::Result<Box<IpPacket>>>,
{
    fn with_delay(
        self,
        min_delay: Duration,
        mean_additional_delay: Duration,
    ) -> crate::adapter::Delay<Self>
    where
        S: Sized,
    {
        crate::adapter::Delay::new(self, min_delay, mean_additional_delay)
    }

    fn with_loss(
        self,
        loss_rate: f64,
        jitter_period: Duration,
    ) -> crate::adapter::Loss<Self>
    where
        Self: Sized,
    {
        crate::adapter::Loss::new(self, loss_rate, jitter_period)
    }
}

