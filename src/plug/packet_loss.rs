use priv_prelude::*;
use rand;
use rand::distributions::{Sample, Range};
use util;

/// Simulate packet loss on a link
pub struct PacketLoss<T: fmt::Debug + 'static> {
    plug_a: Plug<T>,
    plug_b: Plug<T>,
    mean_loss_duration: Duration,
    mean_keep_duration: Duration,
    currently_losing: bool,
    state_toggle_time: Instant,
}

impl<T: fmt::Debug + Send + 'static> PacketLoss<T> {
    /// Spawn a `PacketLoss` directly onto the event loop
    pub fn spawn(
        handle: &NetworkHandle,
        loss_rate: f64,
        mean_loss_duration: Duration,
        plug_a: Plug<T>,
        plug_b: Plug<T>,
    ) {
        let mean_keep_duration = mean_loss_duration.mul_f64(1.0 / loss_rate - 1.0);
        let currently_losing = Range::new(0.0, 1.0).sample(&mut rand::thread_rng()) < loss_rate;
        let state_toggle_time = Instant::now() + if currently_losing {
            mean_loss_duration.mul_f64(util::expovariate_rand())
        } else {
            mean_keep_duration.mul_f64(util::expovariate_rand())
        };
        let packet_loss = PacketLoss {
            plug_a,
            plug_b,
            mean_loss_duration,
            mean_keep_duration,
            currently_losing,
            state_toggle_time,
        };
        handle.spawn(packet_loss.infallible())
    }
}

impl<T: fmt::Debug + 'static> Future for PacketLoss<T> {
    type Item = ();
    type Error = Void;

    fn poll(&mut self) -> Result<Async<()>, Void> {
        let now = Instant::now();
        while self.state_toggle_time < now {
            self.currently_losing = !self.currently_losing;
            self.state_toggle_time += if self.currently_losing {
                self.mean_loss_duration.mul_f64(util::expovariate_rand())
            } else {
                self.mean_keep_duration.mul_f64(util::expovariate_rand())
            };
        }

        let a_unplugged = loop {
            match self.plug_a.rx.poll().void_unwrap() {
                Async::NotReady => break false,
                Async::Ready(None) => break true,
                Async::Ready(Some(packet)) => {
                    if self.currently_losing {
                        trace!("packet loss randomly dropping packet: {:?}", packet);
                    } else {
                        let _ = self.plug_b.tx.unbounded_send(packet);
                    }
                },
            }
        };

        let b_unplugged = loop {
            match self.plug_b.rx.poll().void_unwrap() {
                Async::NotReady => break false,
                Async::Ready(None) => break true,
                Async::Ready(Some(packet)) => {
                    if self.currently_losing {
                        trace!("packet loss randomly dropping packet: {:?}", packet);
                    } else {
                        let _ = self.plug_a.tx.unbounded_send(packet);
                    }
                },
            }
        };

        if a_unplugged && b_unplugged {
            return Ok(Async::Ready(()));
        }

        Ok(Async::NotReady)
    }
}

