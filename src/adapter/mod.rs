//! `Sink`/`Stream` adapters.

use crate::priv_prelude::*;

mod delay;
mod loss;

pub use self::{
    delay::Delay,
    loss::Loss,
};

pub(crate) fn expovariate_duration<R>(
    mean_duration: Duration,
    rng: &mut R,
) -> Duration
where
    R: Rng,
{
    let mean_duration = mean_duration.as_secs_f64();
    loop {
        let duration = mean_duration * -rng.gen::<f64>().ln();
        match Duration::try_from_secs_f64(duration) {
            Ok(duration) => break duration,
            Err(_) => continue,
        }
    }
}

