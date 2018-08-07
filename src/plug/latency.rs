use priv_prelude::*;
use util;

struct InTransit<T> {
    packet: Option<T>,
    timeout: Delay,
}

impl<T> Future for InTransit<T> {
    type Item = T;
    type Error = Void;

    fn poll(&mut self) -> Result<Async<T>, Void> {
        match self.timeout.poll().void_unwrap() {
            Async::Ready(()) => Ok(Async::Ready(unwrap!(self.packet.take()))),
            Async::NotReady => Ok(Async::NotReady),
        }
    }
}

/// Links two `Ipv4Plug`s and adds delay to packets travelling between them.
pub struct Latency<T: fmt::Debug + 'static> {
    plug_a: Plug<T>,
    plug_b: Plug<T>,
    outgoing_a: FuturesUnordered<InTransit<T>>,
    outgoing_b: FuturesUnordered<InTransit<T>>,
    min_latency: Duration,
    mean_additional_latency: Duration,
}

impl<T: fmt::Debug + Send + 'static> Latency<T> {
    pub fn spawn(
        handle: &NetworkHandle,
        min_latency: Duration,
        mean_additional_latency: Duration,
        plug_a: Plug<T>,
        plug_b: Plug<T>,
    ) {
        let latency = Latency {
            plug_a,
            plug_b,
            outgoing_a: FuturesUnordered::new(),
            outgoing_b: FuturesUnordered::new(),
            min_latency,
            mean_additional_latency,
        };
        handle.spawn(latency);
    }
}

impl<T: fmt::Debug + 'static> Future for Latency<T> {
    type Item = ();
    type Error = Void;

    fn poll(&mut self) -> Result<Async<()>, Void> {
        let now = Instant::now();
        let a_unplugged = loop {
            match self.plug_a.rx.poll().void_unwrap() {
                Async::NotReady => break false,
                Async::Ready(None) => break true,
                Async::Ready(Some(packet)) => {
                    let delay
                        = self.min_latency
                        + self.mean_additional_latency.mul_f64(util::expovariate_rand());
                    let in_transit = InTransit {
                        packet: Some(packet),
                        timeout: Delay::new(now + delay),
                    };
                    self.outgoing_b.push(in_transit);
                },
            }
        };

        let b_unplugged = loop {
            match self.plug_b.rx.poll().void_unwrap() {
                Async::NotReady => break false,
                Async::Ready(None) => break true,
                Async::Ready(Some(packet)) => {
                    let delay
                        = self.min_latency
                        + self.mean_additional_latency.mul_f64(util::expovariate_rand());
                    let in_transit = InTransit {
                        packet: Some(packet),
                        timeout: Delay::new(now + delay),
                    };
                    self.outgoing_a.push(in_transit);
                },
            }
        };

        loop {
            match self.outgoing_a.poll().void_unwrap() {
                Async::NotReady => break,
                Async::Ready(None) => break,
                Async::Ready(Some(packet)) => {
                    let _ = self.plug_a.tx.unbounded_send(packet);
                },
            }
        }

        loop {
            match self.outgoing_b.poll().void_unwrap() {
                Async::NotReady => break,
                Async::Ready(None) => break,
                Async::Ready(Some(packet)) => {
                    let _ = self.plug_b.tx.unbounded_send(packet);
                },
            }
        }

        if a_unplugged && b_unplugged {
            return Ok(Async::Ready(()));
        }

        Ok(Async::NotReady)
    }
}

/*
   TODO: fix the math in this test

#[cfg(test)]
#[test]
fn test() {
    run_test(|| {
        use rand;

        const NUM_PACKETS: u64 = 1000;

        let mut core = unwrap!(Core::new());
        let handle = core.handle();

        let source_addr = SocketAddrV4::new(
            Ipv4Addr::random_global(),
            rand::random::<u16>() / 2 + 1000,
        );
        let dest_addr = SocketAddrV4::new(
            Ipv4Addr::random_global(),
            rand::random::<u16>() / 2 + 1000,
        );
        let packet = Ipv4Packet::new_from_fields_recursive(
            Ipv4Fields {
                source_ip: *source_addr.ip(),
                dest_ip: *dest_addr.ip(),
                ttl: 16,
            },
            Ipv4PayloadFields::Udp {
                fields: UdpFields::V4 {
                    source_addr: source_addr,
                    dest_addr: dest_addr,
                },
                payload: Bytes::from(&rand::random::<[u8; 8]>()[..]),
            },
        );

        let min_latency = Duration::from_millis(100).mul_f64(util::expovariate_rand());
        let mean_additional_latency = Duration::from_millis(100).mul_f64(util::expovariate_rand());

        let (plug_a, plug_a_pass) = Ipv4Plug::new_pair();
        let (plug_b, plug_b_pass) = Ipv4Plug::new_pair();
        Ipv4Latency::spawn(&handle, min_latency, mean_additional_latency, plug_a_pass, plug_b_pass);

        let res = core.run({
            let start_time_0 = Instant::now();
            for _ in 0..NUM_PACKETS {
                let _ = plug_a.tx.unbounded_send(packet.clone());
            }
            let start_time_1 = Instant::now();
            let start_time = start_time_0 + (start_time_1 - start_time_0) / 2;

            plug_b.rx
            .take(NUM_PACKETS)
            .map(move |_packet| {
                let delay = Instant::now() - start_time;
                assert!(delay >= min_latency);
                let additional_delay = delay - min_latency;
                additional_delay.div_to_f64(mean_additional_latency)
            })
            .collect()
            .map(move |samples| {
                // let this test fail one in a million times due to randomness
                const CHANCE_OF_FAILURE: f64 = 1e-6f64;

                // inverse of the normal distribution cumulative probability function
                fn quantile(mean: f64, variance: f64, p: f64) -> f64 {
                    use statrs::function::erf::erf_inv;

                    mean + f64::sqrt(2.0 * variance) * erf_inv(2.0 * p - 1.0)
                }

                // see: https://en.wikipedia.org/wiki/Exponential_distribution#Confidence_intervals
                // a chi-squared(k) distribution can be approximated by normal distribution with mean k
                // and variance 2 * k
                let lower_chi_squared = quantile(
                    NUM_PACKETS as f64,
                    (2 * NUM_PACKETS) as f64,
                    CHANCE_OF_FAILURE / 2.0,
                );
                let upper_chi_squared = quantile(
                    NUM_PACKETS as f64,
                    (2 * NUM_PACKETS) as f64,
                    1.0 - CHANCE_OF_FAILURE / 2.0,
                );

                let mean = samples.into_iter().sum::<f64>() / (NUM_PACKETS as f64);

                assert!(2.0 * NUM_PACKETS as f64 * mean / lower_chi_squared < 1.0);
                assert!(2.0 * NUM_PACKETS as f64 * mean / upper_chi_squared > 1.0);
            })
        });
        res.void_unwrap()
    })
}
*/

