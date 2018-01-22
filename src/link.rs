



// TODO: figure out how to do rate-limiting.
// seems like the kernel will allow userspace to just keep sending data out a TAP interface even if
// the frames aren't being read quickly enough on the otherside. Data seems to just get dropped.
// *haven't confirmed this though* - could label the packets and see.
//
// Otherwise, I could make the sender thread rate-limit itself. Which may be good for some purposes
// but isn't really a complete solution.



use priv_prelude::*;
use util;

/// Configure a link. A link wraps a stream/sink of ethernet frames and introduces packet loss,
/// randomized latency, extra hops (which effect ttl values), etc.
pub struct LinkBuilder {
    min_latency: Duration,
    mean_additional_latency: Duration,
    loss_burst: Duration,
    loss_rate: f32,
    //ttl: u8,
    bandwidth_rx: f32,
    bandwidth_tx: f32,
}

impl Default for LinkBuilder {
    fn default() -> LinkBuilder {
        LinkBuilder {
            min_latency: Duration::new(0, 0),
            mean_additional_latency: Duration::new(0, 0),
            loss_rate: 0.0,
            loss_burst: Duration::from_millis(10),
            //ttl: 0,
            bandwidth_rx: 12.5e6,   // 100 megabit
            bandwidth_tx: 12.5e6,
        }
    }
}

impl LinkBuilder {
    /// Start configuring a new link with the default settings.
    pub fn new() -> LinkBuilder {
        Default::default()
    }

    /// Set the minimum latency of the link. All packets that traverse the link will be delayed by
    /// at least this amount.
    pub fn min_latency(&mut self, min_latency: Duration) -> &mut Self {
        self.min_latency = min_latency;
        self
    }

    /// Set the additional latency of the link. The additional latency (added to the minimum
    /// latency) for each packet will be a random duration with the given mean.
    pub fn mean_additional_latency(&mut self, mean_additional_latency: Duration) -> &mut Self {
        self.mean_additional_latency = mean_additional_latency;
        self
    }

    /// Set the loss rate, the proportion of packets that get dropped on this link. eg. set to
    /// `0.5` for 50% packet loss.
    pub fn loss_rate(&mut self, loss_rate: f32) -> &mut Self {
        if loss_rate < 0.0 || loss_rate > 1.0 {
            panic!("loss_rate must be between 0 and 1 inclusive");
        }

        self.loss_rate = loss_rate;
        self
    }

    /// Set the average burst length of lost packets. A higher value makes packet loss more bursty.
    pub fn loss_burst(&mut self, loss_burst: Duration) -> &mut Self {
        self.loss_burst = loss_burst;
        self
    }

    /// Set the upload bandwidth of the link.
    pub fn bandwidth_rx(&mut self, bandwidth_rx: f32) -> &mut Self {
        if bandwidth_rx <= 0.0 {
            panic!("bandwidth_rx must be greater than zero");
        }
        self.bandwidth_rx = bandwidth_rx;
        self
    }

    /// Set the download bandwidth of the link.
    pub fn bandwidth_tx(&mut self, bandwidth_tx: f32) -> &mut Self {
        if bandwidth_tx <= 0.0 {
            panic!("bandwidth_tx must be greater than zero");
        }
        self.bandwidth_tx = bandwidth_tx;
        self
    }

    /// Build the link, wrapping the given `channel` and utilising the tokio event loop given by
    /// `handle`.
    pub fn build(self, channel: EtherBox, handle: &Handle) -> Link {
        Link {
            cfg: self,
            channel: channel,
            in_transit_rx: BTreeMap::new(),
            in_transit_tx: BTreeMap::new(),
            timeout_rx_read: Timeout::new(Duration::new(0, 0), handle),
            timeout_rx_write: Timeout::new(Duration::new(0, 0), handle),
            timeout_tx_read: Timeout::new(Duration::new(0, 0), handle),
            timeout_tx_write: Timeout::new(Duration::new(0, 0), handle),
            loss_state: false,
            next_loss_state_toggle: Instant::now(),
            sending: None,
            unplugged: false,
            debug_frames_read: 0,
        }
    }

    fn buffer_frame(
        &self,
        mut frame: EtherFrame,
        buffer: &mut BTreeMap<Instant, EtherFrame>,
    ) {
        /*
        let ipv4 = match frame.payload() {
            EtherPayload::Ipv4(ipv4) => ipv4,
            _ => return,
        };
        */
        //let ttl = ipv4.ttl().saturating_sub(self.ttl);
        //if ttl > 0 {
            //ipv4.set_ttl(ttl);
            let latency = self.min_latency
                        + self.mean_additional_latency.mul_f32(util::expovariant_rand());
            let arrival = Instant::now() + latency;
            //frame.set_payload(EtherPayload::Ipv4(ipv4));
            buffer.insert(arrival, frame);
        //}
    }
}

/// A link, wrapping an underlying stream/sink of ethernet frames and introducing packet loss,
/// bandwidth limits, etc. Created using `LinkBuilder`.
pub struct Link {
    cfg: LinkBuilder,
    channel: EtherBox,
    in_transit_rx: BTreeMap<Instant, EtherFrame>,
    in_transit_tx: BTreeMap<Instant, EtherFrame>,
    timeout_rx_read: Timeout,
    timeout_rx_write: Timeout,
    timeout_tx_read: Timeout,
    timeout_tx_write: Timeout,
    loss_state: bool,
    next_loss_state_toggle: Instant,
    sending: Option<EtherFrame>,
    unplugged: bool,
    debug_frames_read: usize,
}

impl Link {
    fn update_loss_state(&mut self) {
        let now = Instant::now();
        if self.cfg.loss_rate > 0.0 {
            while self.next_loss_state_toggle < now {
                self.loss_state = !self.loss_state;
                let avg_duration = if self.loss_state {
                    self.cfg.loss_burst
                } else {
                    self.cfg.loss_burst.mul_f32((1.0 / self.cfg.loss_rate) - 1.0)
                };
                let duration = avg_duration.mul_f32(util::expovariant_rand());
                self.next_loss_state_toggle += duration;
            }
        }
    }
}

impl Stream for Link {
    type Item = EtherFrame;
    type Error = io::Error;

    fn poll(&mut self) -> io::Result<Async<Option<EtherFrame>>> {
        self.update_loss_state();

        loop {
            trace!("in link stream wow, frames_read == {}, frames_in_trasit == {}", self.debug_frames_read, self.in_transit_rx.len());
            match self.timeout_rx_read.poll().void_unwrap() {
                Async::NotReady => {
                    trace!("timeout not ready");
                    break;
                },
                Async::Ready(()) => {
                    trace!("polling channel");
                    match self.channel.poll()? {
                        Async::Ready(Some(frame)) => {
                            self.debug_frames_read += 1;
                            let len = frame.len() as f32;
                            //let read_delay = Duration::from_secs(1).mul_f32(len / self.cfg.bandwidth_rx);
                            let read_delay = Duration::new(0, 0);
                            self.timeout_rx_read.reset(Instant::now() + read_delay);
                            if !self.loss_state {
                                self.cfg.buffer_frame(frame, &mut self.in_transit_rx);
                            }
                        },
                        Async::Ready(None) => self.unplugged = true,
                        Async::NotReady => break,
                    }
                },
            }
        }

        while let Some(instant) = self.in_transit_rx.keys().next().cloned() {
            trace!("in link stream yo");
            self.timeout_rx_write.reset(instant);
            match self.timeout_rx_write.poll().void_unwrap() {
                Async::NotReady => break,
                Async::Ready(()) => {
                    let frame = unwrap!(self.in_transit_rx.remove(&instant));
                    return Ok(Async::Ready(Some(frame)));
                },
            }
        }

        if self.unplugged && self.in_transit_rx.is_empty() {
            return Ok(Async::Ready(None));
        }

        Ok(Async::NotReady)
    }
}

impl Sink for Link {
    type SinkItem = EtherFrame;
    type SinkError = io::Error;

    fn start_send(&mut self, frame: EtherFrame) -> io::Result<AsyncSink<EtherFrame>> {
        self.update_loss_state();

        match self.timeout_tx_read.poll().void_unwrap() {
            Async::Ready(()) => (),
            Async::NotReady => return Ok(AsyncSink::NotReady(frame)),
        };

        let len = frame.len() as f32;
        //let read_delay = Duration::from_secs(1).mul_f32(len / self.cfg.bandwidth_tx);
        let read_delay = Duration::new(0, 0);
        self.timeout_tx_read.reset(Instant::now() + read_delay);
        let _ = self.timeout_tx_read.poll().void_unwrap();

        if !self.loss_state {
            self.cfg.buffer_frame(frame, &mut self.in_transit_tx);
        }

        Ok(AsyncSink::Ready)
    }

    fn poll_complete(&mut self) -> io::Result<Async<()>> {
        loop {
            if let Some(frame) = self.sending.take() {
                trace!("trying to send frame");
                match self.channel.start_send(frame)? {
                    AsyncSink::Ready => (),
                    AsyncSink::NotReady(frame) => {
                        self.sending = Some(frame);
                    },
                }
            }

            if let Some(instant) = self.in_transit_tx.keys().next().cloned() {
                self.timeout_tx_write.reset(instant);
                match self.timeout_tx_write.poll().void_unwrap() {
                    Async::NotReady => return Ok(Async::NotReady),
                    Async::Ready(()) => {
                        let frame = unwrap!(self.in_transit_tx.remove(&instant));
                        trace!("dropping frame!");
                        self.sending = Some(frame); // may drop the currently sending frame,
                    },
                }
            } else {
                break;
            }
        }

        Ok(Async::Ready(()))
    }
}

#[cfg(test)]
mod test {
    use priv_prelude::*;
    use spawn;
    use std;
    use env_logger;
    use util;
    use bincode;
    use future_utils;

    #[test]
    fn correct_stats() {
        let _ = env_logger::init();
        let mut core = unwrap!(Core::new());
        let handle = core.handle();
        let res = core.run(future::lazy(|| {
            const NUM_PACKETS: usize = 10_000;
            const ENCODED_LIMIT: usize = 64;

            let ip_0 = Ipv4Addr::random_global();
            let ip_1 = Ipv4Addr::random_global();
            let addr_0 = SocketAddr::V4(SocketAddrV4::new(ip_0, 123));
            let addr_1 = SocketAddr::V4(SocketAddrV4::new(ip_1, 123));
            let mut tap_builder_0 = TapBuilderV4::new();
            tap_builder_0.name("netsim0");
            tap_builder_0.address(ip_0);
            tap_builder_0.route(RouteV4 {
                destination: SubnetV4::new(ip_1, 32),
                gateway: None,
            });
            let mut tap_builder_1 = TapBuilderV4::new();
            tap_builder_1.name("netsim1");
            tap_builder_1.address(ip_1);
            tap_builder_1.route(RouteV4 {
                destination: SubnetV4::new(ip_0, 32),
                gateway: None,
            });

            let min_latency = 20.0 + 20.0 * util::expovariant_rand();
            let min_latency = Duration::from_millis(1).mul_f32(min_latency);
            let mean_additional_latency = 10.0 + 10.0 * util::expovariant_rand();
            let mean_additional_latency = Duration::from_millis(1).mul_f32(mean_additional_latency);
            let start_time = Instant::now();

            let (join_handle_0, mut taps) = spawn::spawn_with_ifaces(
                &handle,
                vec![tap_builder_0],
                move || {
                    let sock_0 = unwrap!(std::net::UdpSocket::bind(addr_0));
                    for i in 0..NUM_PACKETS {
                        let now = Instant::now() - start_time;
                        let data = (now.as_secs(), now.subsec_nanos());
                        let encoded = unwrap!(bincode::serialize(&data, bincode::Bounded(ENCODED_LIMIT as u64)));
                        unwrap!(sock_0.send_to(&encoded[..], addr_1));
                        trace!("packets actually sent: {}", i);
                    }
                },
            );
            let tap_0 = unwrap!(taps.pop());

            let (drop_tx, drop_rx) = future_utils::drop_notify();
            let (join_handle_1, mut taps) = spawn::spawn_with_ifaces(
                &handle,
                vec![tap_builder_1],
                move || {
                    println!("address is {}", addr_1);
                    ::std::process::Command::new("ifconfig").status().unwrap();
                    let sock_1 = unwrap!(std::net::UdpSocket::bind(addr_1));
                    sock_1.set_read_timeout(Some(min_latency + mean_additional_latency * 100));
                    let mut total_latency = Duration::new(0, 0);
                    let mut num_received = 0;
                    let mut actual_min_latency = None;
                    loop {
                        let mut buffer = [0u8; ENCODED_LIMIT];
                        match sock_1.recv_from(&mut buffer) {
                            Ok((n, addr)) => {
                                let data  = unwrap!(bincode::deserialize(&buffer[..n]));
                                let (secs, subsec_nanos) = data;
                                let latency = Instant::now() - (start_time + Duration::new(secs, subsec_nanos));
                                num_received += 1;
                                total_latency += latency;
                                actual_min_latency = Some(match actual_min_latency {
                                    Some(actual_min_latency) => cmp::min(actual_min_latency, latency),
                                    None => latency,
                                });
                            },
                            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock || 
                                          e.kind() == io::ErrorKind::TimedOut => break,
                            Err(e) => panic!("read error: {}", e),
                        };
                    }
                    let actual_min_latency = unwrap!(actual_min_latency);
                    if actual_min_latency > min_latency.mul_f32(1.1) {
                        panic!("actual_min_latency is suspiciously large. actual_min_latency == {:?}, min_latency == {:?}, num_received == {}", actual_min_latency, min_latency, num_received);
                    }

                    let actual_mean_latency = total_latency.mul_f32(1.0 / (num_received as f32));
                    let actual_mean_additional_latency = actual_mean_latency - min_latency;

                    // Were the stats rougly correct? Note that this could (very rarely) fail just
                    // due to randomness.
                    assert!(actual_mean_additional_latency > mean_additional_latency.mul_f32(0.9));
                    assert!(actual_mean_additional_latency < mean_additional_latency.mul_f32(1.1));
                    drop(drop_tx);
                },
            );
            let tap_1 = unwrap!(taps.pop());

            let mut link_builder = LinkBuilder::new();
            link_builder.min_latency(min_latency);
            let link = link_builder.build(Box::new(tap_0), &handle);

            let pump_frames_around = {
                let (link_tx, link_rx) = link.split();
                let (tap_tx, tap_rx) = tap_1.split();
                let f0 = link_tx.send_all(tap_rx);
                let f1 = tap_tx.send_all(link_rx);

                f0
                .join(f1)
                .map(|((_link_tx, _tap_rx), (_tap_tx, _link_rx))| ())
                .map_err(|e| panic!("io error: {}", e))
            };

            drop_rx
            .while_driving(pump_frames_around)
            .map(|((), _pump_frames_around)| {
                unwrap!(join_handle_0.join());
                unwrap!(join_handle_1.join());
            })
            .map_err(|(v, _pump_frames_around)| v)
        }));
        res.void_unwrap()
    }
}


