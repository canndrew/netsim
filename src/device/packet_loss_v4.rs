use priv_prelude::*;
use rand;
use rand::distributions::{Sample, Range};
use util;

/// Simulate packet loss on a link
pub struct PacketLossV4 {
    plug_a: Ipv4Plug,
    plug_b: Ipv4Plug,
    mean_loss_duration: Duration,
    mean_keep_duration: Duration,
    currently_losing: bool,
    state_toggle_time: Instant,
}

impl PacketLossV4 {
    /// Spawn a `PacketLossV4` directly onto the event loop
    pub fn spawn(
        handle: &Handle,
        loss_rate: f64,
        mean_loss_duration: Duration,
        plug_a: Ipv4Plug,
        plug_b: Ipv4Plug,
    ) {
        let mean_keep_duration = mean_loss_duration.mul_f64(1.0 / loss_rate - 1.0);
        let currently_losing = Range::new(0.0, 1.0).sample(&mut rand::thread_rng()) < loss_rate;
        let state_toggle_time = Instant::now() + if currently_losing {
            mean_loss_duration.mul_f64(util::expovariate_rand())
        } else {
            mean_keep_duration.mul_f64(util::expovariate_rand())
        };
        let packet_loss = PacketLossV4 {
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

impl Future for PacketLossV4 {
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

