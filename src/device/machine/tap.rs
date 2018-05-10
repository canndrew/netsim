use priv_prelude::*;

pub struct TapTask {
    tap: EtherIface,
    frame_tx: UnboundedSender<EtherFrame>,
    frame_rx: UnboundedReceiver<EtherFrame>,
    sending_frame: Option<EtherFrame>,
    handle: Handle,
    state: TapTaskState,
}

impl TapTask {
    pub fn new(
        tap: EtherIface,
        handle: &Handle,
        plug: EtherPlug,
        drop_rx: DropNotice,
    ) -> TapTask {
        TapTask {
            tap: tap,
            handle: handle.clone(),
            frame_tx: plug.tx,
            frame_rx: plug.rx,
            sending_frame: None,
            state: TapTaskState::Receiving { drop_rx },
        }
    }
}

enum TapTaskState {
    Receiving {
        drop_rx: DropNotice,
    },
    Dying(Timeout),
    Invalid,
}

impl Future for TapTask {
    type Item = ();
    type Error = Void;

    fn poll(&mut self) -> Result<Async<()>, Void> {
        let grace_period: Duration = Duration::from_millis(100);

        let mut received_frames = false;
        loop {
            match self.tap.poll() {
                Ok(Async::Ready(Some(frame))) => {
                    let _ = self.frame_tx.unbounded_send(frame);
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
            if let Some(frame) = self.sending_frame.take() {
                match self.tap.start_send(frame) {
                    Ok(AsyncSink::Ready) => (),
                    Ok(AsyncSink::NotReady(frame)) => {
                        self.sending_frame = Some(frame);
                        break;
                    },
                    Err(e) => {
                        panic!("writing TAP device yielded an error: {}", e);
                    },
                }
            }

            match self.frame_rx.poll().void_unwrap() {
                Async::Ready(Some(frame)) => {
                    self.sending_frame = Some(frame);
                    continue;
                },
                _ => break,
            }
        }
        match self.tap.poll_complete() {
            Ok(..) => (),
            Err(e) => {
                panic!("completing TAP device write yielded an error: {}", e);
            },
        }

        let mut state = mem::replace(&mut self.state, TapTaskState::Invalid);
        trace!("polling TapTask");
        loop {
            match state {
                TapTaskState::Receiving {
                    mut drop_rx,
                } => {
                    trace!("state == receiving");
                    match drop_rx.poll().void_unwrap() {
                        Async::Ready(()) => {
                            state = TapTaskState::Dying(Timeout::new(grace_period, &self.handle));
                            continue;
                        },
                        Async::NotReady => {
                            state = TapTaskState::Receiving { drop_rx };
                            break;
                        },
                    }
                },
                TapTaskState::Dying(mut timeout) => {
                    trace!("state == dying");
                    if received_frames {
                        timeout.reset(Instant::now() + grace_period);
                    }
                    match timeout.poll().void_unwrap() {
                        Async::Ready(()) => {
                            return Ok(Async::Ready(()));
                        },
                        Async::NotReady => {
                            state = TapTaskState::Dying(timeout);
                            break;
                        },
                    }
                }
                TapTaskState::Invalid => {
                    panic!("TapTask in invalid state!");
                },
            }
        }
        self.state = state;

        Ok(Async::NotReady)
    }
}


