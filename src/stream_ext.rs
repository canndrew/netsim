use crate::priv_prelude::*;

pub trait SinkStreamExt<T>: Stream + Sink<T> {
    /// Delays items sent/received through this `Sink`/`Stream`.
    /// 
    /// * `min_delay` is the minimum delay which is applied to all items.
    /// * `mean_additional_delay` is the average randomized delay applied to all items in addition to
    /// `min_delay`. Setting this to zero disables delay randomization and guarantees that
    /// items are recieved in the order they're sent.
    fn with_delay(
        self,
        min_delay: Duration,
        mean_additional_delay: Duration,
    ) -> crate::adapter::Delay<Self, T>
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

impl<S, T> SinkStreamExt<T> for S
where
    S: Stream + Sink<T>,
{
    fn with_delay(
        self,
        min_delay: Duration,
        mean_additional_delay: Duration,
    ) -> crate::adapter::Delay<Self, T>
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

