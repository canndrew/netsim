use priv_prelude::*;

pub struct TunTask {
    tun: IpIface,
    packet_tx: IpSender,
    packet_rx: IpReceiver,
    sending_packet: Option<IpPacket>,
    handle: NetworkHandle,
    state: TunTaskState,
}

impl TunTask {
    pub fn new(
        tun: IpIface,
        handle: &NetworkHandle,
        plug: IpPlug,
        drop_rx: DropNotice,
    ) -> TunTask {
        let (tx, rx) = plug.split();
        TunTask {
            tun,
            handle: handle.clone(),
            packet_tx: tx,
            packet_rx: rx,
            sending_packet: None,
            state: TunTaskState::Receiving { drop_rx },
        }
    }
}

enum TunTaskState {
    Receiving {
        drop_rx: DropNotice,
    },
    Dying(Timeout),
    Invalid,
}

impl Future for TunTask {
    type Item = ();
    type Error = Void;

    fn poll(&mut self) -> Result<Async<()>, Void> {
        trace!("polling TunTask");
        let grace_period: Duration = Duration::from_millis(100);

        let mut received_frames = false;
        loop {
            match self.tun.poll() {
                Ok(Async::Ready(Some(frame))) => {
                    self.packet_tx.unbounded_send(frame);
                    received_frames = true;
                },
                Ok(Async::Ready(None)) => {
                    panic!("TAP stream ended somehow");
                },
                Ok(Async::NotReady) => break,
                Err(e) => {
                    panic!("reading TAP device yielded an error: {}", e);
                },
            }
        }

        loop {
            trace!("looping receiver ...");
            if let Some(frame) = self.sending_packet.take() {
                trace!("we have a frame ready to send");
                match self.tun.start_send(frame) {
                    Ok(AsyncSink::Ready) => (),
                    Ok(AsyncSink::NotReady(frame)) => {
                        trace!("couldn't send the frame ;(");
                        self.sending_packet = Some(frame);
                        break;
                    },
                    Err(e) => {
                        panic!("writing TAP device yielded an error: {}", e);
                    },
                }
            }

            match self.packet_rx.poll().void_unwrap() {
                Async::Ready(Some(frame)) => {
                    trace!("we received a frame");
                    self.sending_packet = Some(frame);
                    continue;
                },
                _ => break,
            }
        }
        trace!("done looping");

        match self.tun.poll_complete() {
            Ok(..) => (),
            Err(e) => {
                panic!("completing TAP device write yielded an error: {}", e);
            },
        }

        let mut state = mem::replace(&mut self.state, TunTaskState::Invalid);
        loop {
            match state {
                TunTaskState::Receiving {
                    mut drop_rx,
                } => {
                    trace!("state == receiving");
                    match drop_rx.poll().void_unwrap() {
                        Async::Ready(()) => {
                            state = TunTaskState::Dying(Timeout::new(grace_period, self.handle.event_loop()));
                            continue;
                        },
                        Async::NotReady => {
                            state = TunTaskState::Receiving { drop_rx };
                            break;
                        },
                    }
                },
                TunTaskState::Dying(mut timeout) => {
                    trace!("state == dying");
                    if received_frames {
                        timeout.reset(Instant::now() + grace_period);
                    }
                    match timeout.poll().void_unwrap() {
                        Async::Ready(()) => {
                            return Ok(Async::Ready(()));
                        },
                        Async::NotReady => {
                            state = TunTaskState::Dying(timeout);
                            break;
                        },
                    }
                }
                TunTaskState::Invalid => {
                    panic!("TunTask in invalid state!");
                },
            }
        }
        self.state = state;

        Ok(Async::NotReady)
    }
}


