use priv_prelude::*;
use util;

struct FrameInTransit {
    arrival: Timeout,
    frame: Option<EthernetFrame<Bytes>>,
}

impl Future for FrameInTransit {
    type Item = EthernetFrame<Bytes>;
    type Error = Void;

    fn poll(&mut self) -> Result<Async<EthernetFrame<Bytes>>, Void> {
        match self.arrival.poll().void_unwrap() {
            Async::Ready(()) => {
                let frame = unwrap!(self.frame.take());
                Ok(Async::Ready(frame))
            },
            Async::NotReady => {
                Ok(Async::NotReady)
            },
        }
    }
}

pub struct Latency {
    channel: Option<EtherBox>,
    min_latency: Duration,
    mean_additional_latency: Duration,
    frames_rx: FuturesUnordered<FrameInTransit>,
    frames_tx: FuturesUnordered<FrameInTransit>,
    sending: VecDeque<EthernetFrame<Bytes>>,
    handle: Handle,
}

impl Latency {
    pub fn new<C: EtherChannel + 'static>(
        channel: C,
        min_latency: Duration,
        mean_additional_latency: Duration,
        handle: &Handle,
    ) -> Latency {
        Latency {
            channel: Some(Box::new(channel) as EtherBox),
            min_latency: min_latency,
            mean_additional_latency: mean_additional_latency,
            frames_rx: FuturesUnordered::new(),
            frames_tx: FuturesUnordered::new(),
            sending: VecDeque::new(),
            handle: handle.clone(),
        }
    }
}

impl Stream for Latency {
    type Item = EthernetFrame<Bytes>;
    type Error = io::Error;

    fn poll(&mut self) -> io::Result<Async<Option<EthernetFrame<Bytes>>>> {
        if let Some(mut channel) = self.channel.take() {
            loop {
                match channel.poll()? {
                    Async::Ready(Some(frame)) => {
                        let r = util::expovariant_rand();
                        let additional_latency = self.mean_additional_latency.mul_f32(r);
                        let latency = self.min_latency + additional_latency;
                        let fit = FrameInTransit {
                            arrival: Timeout::new(latency, &self.handle),
                            frame: Some(frame),
                        };
                        self.frames_rx.push(fit);
                    },
                    Async::Ready(None) => {
                        break;
                    },
                    Async::NotReady => {
                        self.channel = Some(channel);
                        break;
                    },
                }
            }
        }

        match self.frames_rx.poll().void_unwrap() {
            Async::Ready(Some(frame)) => Ok(Async::Ready(Some(frame))),
            Async::Ready(None) => {
                if self.channel.is_some() {
                    Ok(Async::NotReady)
                } else {
                    Ok(Async::Ready(None))
                }
            },
            Async::NotReady => Ok(Async::NotReady)
        }
    }
}

impl Sink for Latency {
    type SinkItem = EthernetFrame<Bytes>;
    type SinkError = io::Error;

    fn start_send(&mut self, frame: EthernetFrame<Bytes>) -> io::Result<AsyncSink<EthernetFrame<Bytes>>> {
        let r = util::expovariant_rand();
        let additional_latency = self.mean_additional_latency.mul_f32(r);
        let latency = self.min_latency + additional_latency;
        let fit = FrameInTransit {
            arrival: Timeout::new(latency, &self.handle),
            frame: Some(frame),
        };
        self.frames_tx.push(fit);

        Ok(AsyncSink::Ready)
    }

    fn poll_complete(&mut self) -> io::Result<Async<()>> {
        while let Async::Ready(Some(frame)) = self.frames_tx.poll().void_unwrap() {
            self.sending.push_back(frame);
        }

        loop {
            let frame = match self.sending.pop_front() {
                Some(frame) => frame,
                None => break,
            };
            if let Some(mut channel) = self.channel.take() {
                match channel.start_send(frame)? {
                    AsyncSink::Ready => {
                        self.channel = Some(channel);
                    },
                    AsyncSink::NotReady(frame) => {
                        self.sending.push_front(frame);
                        self.channel = Some(channel);
                        break;
                    },
                }
            }
        }

        if let Some(ref mut channel) = self.channel {
            if let Async::Ready(()) = channel.poll_complete()? {
                if self.sending.is_empty() && self.frames_tx.is_empty() {
                    return Ok(Async::Ready(()));
                }
            }
        }

        Ok(Async::NotReady)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use spawn;
    use std;
    use bincode;
    use env_logger;
    use ethernet;

    #[test]
    fn test() {
        const NUM_PACKETS: usize = 100;

        let _ = env_logger::init();

        let min_latency = Duration::from_millis(1000);
        let mean_additional_latency = Duration::from_millis(500);

        let mut core = unwrap!(Core::new());
        let handle = core.handle();

        let res = core.run(future::lazy(move || {
            let start_time = Instant::now();

            let (join_handle, tap) = spawn::on_subnet(&handle, SubnetV4::local_10(), move |_ip| {
                let socket = unwrap!(std::net::UdpSocket::bind(addr!("0.0.0.0:0")));
                for _ in 0..NUM_PACKETS {
                    let now = Instant::now() - start_time;
                    let data = unwrap!(bincode::serialize(&now, bincode::Infinite));
                    unwrap!(socket.send_to(&data, &addr!("10.2.3.4:567")));
                }
            });

            let mac_addr = ethernet::random_mac();
            ethernet::respond_to_arp(tap, ipv4!("10.2.3.4"), mac_addr)
            .map_err(|e| panic!("early tap read error: {}", e))
            .and_then(move |tap| {
                let latency = Latency::new(
                    tap,
                    min_latency,
                    mean_additional_latency,
                    &handle,
                );

                latency
                .map_err(|e| panic!("tap read error: {}", e))
                .filter_map(move |frame| {
                    let frame = EthernetFrame::new(frame.as_ref());
                    if let EthernetProtocol::Ipv4 = frame.ethertype() {
                        let ipv4 = Ipv4Packet::new(frame.payload());
                        if let IpProtocol::Udp = ipv4.protocol() {
                            let udp = UdpPacket::new(ipv4.payload());
                            let data = udp.payload();
                            let sent_time: Duration = unwrap!(bincode::deserialize(&data));
                            let sent_time = start_time + sent_time;
                            let delay = Instant::now() - sent_time;
                            assert!(delay >= min_latency);
                            return Some(delay);
                        }
                    }
                    None
                })
                .take(NUM_PACKETS as u64)
                .collect()
                .map(move |delays| {
                    assert_eq!(delays.len(), NUM_PACKETS);

                    let mut total = Duration::new(0, 0);
                    for delay in delays {
                        total += delay;
                    }
                    let mean_latency = total.mul_f32(1.0 / NUM_PACKETS as f32);
                    let actual_additional_latency = mean_latency - min_latency;
                    trace!("actual_additional_latency == {:?}", actual_additional_latency);
                    trace!("mean_additional_latency == {:?}", mean_additional_latency);
                    if actual_additional_latency > mean_additional_latency.mul_f32(1.2) {
                        panic!("latency too high");
                    }
                    if actual_additional_latency < mean_additional_latency.mul_f32(0.8) {
                        panic!("latency too low");
                    }
                    unwrap!(join_handle.join());
                })

            })
        }));
        res.void_unwrap()
    }
}

