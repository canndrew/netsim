use crate::priv_prelude::*;

pub trait PacketStreamExt: Stream<Item = Vec<u8>> {
    fn with_delay<R>(
        self,
        min_delay: Duration,
        mean_additional_delay: Duration,
    ) -> crate::adapter::Delay<Self, Vec<u8>, R>
    where
        Self: Sized,
        R: Rng + Default;

    fn with_loss<R>(
        self,
        loss: f64,
    ) -> crate::adapter::Loss<Self, R>
    where
        Self: Sized,
        R: Rng + Default;
}

impl<S> PacketStreamExt for S
where
    S: Stream<Item = Vec<u8>>,
{
    fn with_delay<R>(
        self,
        min_delay: Duration,
        mean_additional_delay: Duration,
    ) -> crate::adapter::Delay<Self, Vec<u8>, R>
    where
        S: Sized,
        R: Rng + Default,
    {
        crate::adapter::Delay::new(self, R::default(), min_delay, mean_additional_delay)
    }

    fn with_loss<R>(
        self,
        loss: f64,
    ) -> crate::adapter::Loss<Self, R>
    where
        Self: Sized,
        R: Rng + Default,
    {
        crate::adapter::Loss::new(self, R::default(), loss)
    }
}

