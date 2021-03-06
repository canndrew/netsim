use crate::priv_prelude::*;

pub struct TunTask {
    tun: IpIface,
    packet_tx: IpSender,
    packet_rx: IpReceiver,
    sending_packet: Option<IpPacket>,
    state: TunTaskState,
}

impl TunTask {
    pub fn new(
        tun: IpIface,
        plug: IpPlug,
        drop_rx: DropNotice,
    ) -> TunTask {
        trace!("TunTask: creating");
        let (tx, rx) = plug.split();
        TunTask {
            tun,
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
    Dying(Delay),
    Invalid,
}

impl Future for TunTask {
    type Item = ();
    type Error = Void;

    fn poll(&mut self) -> Result<Async<()>, Void> {
        trace!("TunTask: polling");
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
            trace!("TunTask: looping receiver ...");
            if let Some(frame) = self.sending_packet.take() {
                trace!("TunTask: we have a frame ready to send");
                match self.tun.start_send(frame) {
                    Ok(AsyncSink::Ready) => (),
                    Ok(AsyncSink::NotReady(frame)) => {
                        trace!("TunTask: couldn't send the frame ;(");
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
                    trace!("TunTask: we received a frame");
                    self.sending_packet = Some(frame);
                    continue;
                },
                _ => break,
            }
        }
        trace!("TunTask: done looping receiver");

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
                    trace!("TunTask: state == receiving");
                    match drop_rx.poll().void_unwrap() {
                        Async::Ready(()) => {
                            state = TunTaskState::Dying(Delay::new(Instant::now() + grace_period));
                            continue;
                        },
                        Async::NotReady => {
                            state = TunTaskState::Receiving { drop_rx };
                            break;
                        },
                    }
                },
                TunTaskState::Dying(mut timeout) => {
                    trace!("TunTask: state == dying");
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

        trace!("TunTask: exiting");
        Ok(Async::NotReady)
    }
}


