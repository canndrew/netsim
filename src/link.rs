use priv_prelude::*;
use util;

/// Configure a link. A link wraps a stream/sink of ethernet frames and introduces packet loss,
/// randomized latency, extra hops (which effect ttl values), etc.
pub struct LinkBuilder {
    min_latency: Duration,
    mean_additional_latency: Duration,
    loss_burst: Duration,
    loss_rate: f32,
    ttl: u8,
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
            ttl: 0,
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
        }
    }

    fn buffer_frame(
        &self,
        mut frame: EtherFrame,
        buffer: &mut BTreeMap<Instant, EtherFrame>,
    ) {
        let mut ipv4 = match frame.payload() {
            EtherPayload::Ipv4(ipv4) => ipv4,
            _ => return,
        };
        let ttl = ipv4.ttl().saturating_sub(self.ttl);
        if ttl > 0 {
            ipv4.set_ttl(ttl);
            let latency = self.min_latency
                        + self.mean_additional_latency.mul_f32(util::expovariant_rand());
            let arrival = Instant::now() + latency;
            frame.set_payload(EtherPayload::Ipv4(ipv4));
            buffer.insert(arrival, frame);
        }
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
            match self.timeout_rx_read.poll().void_unwrap() {
                Async::NotReady => break,
                Async::Ready(()) => {
                    match self.channel.poll()? {
                        Async::Ready(Some(frame)) => {
                            let len = frame.len() as f32;
                            let read_delay = Duration::from_secs(1).mul_f32(len / self.cfg.bandwidth_rx);
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
        let read_delay = Duration::from_secs(1).mul_f32(len / self.cfg.bandwidth_tx);
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
                    Async::NotReady => break,
                    Async::Ready(()) => {
                        let frame = unwrap!(self.in_transit_tx.remove(&instant));
                        self.sending = Some(frame); // may drop the currently sending frame,
                    },
                }
            }
        }

        Ok(Async::Ready(()))
    }
}

