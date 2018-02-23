use priv_prelude::*;
use std;
use future_utils;
use spawn;

/// Spawn a function into a new network namespace with a network interface described by `iface`.
/// Returns a `JoinHandle` which can be used to join the spawned thread, along with a channel which
/// can be used to read/write IPv4 packets to the spawned thread's interface.
pub fn with_ipv4_iface<F, R>(
    handle: &Handle,
    iface: Ipv4IfaceBuilder,
    func: F,
) -> (JoinHandle<R>, Ipv4Plug)
where
    R: Send + 'static,
    F: FnOnce() -> R + Send + 'static,
{
    let (tx, rx) = std::sync::mpsc::channel();
    let join_handle = spawn::new_namespace(move || {
        trace!("building tun {:?}", iface);
        let (drop_tx, drop_rx) = future_utils::drop_notify();
        let tun_unbound = unwrap!(iface.build_unbound());
        unwrap!(tx.send((tun_unbound, drop_rx)));
        let ret = func();
        drop(drop_tx);
        ret
    });

    let (tun_unbound, drop_rx) = unwrap!(rx.recv());
    let tun = tun_unbound.bind(handle);

    let (plug_a, plug_b) = Ipv4Plug::new_wire();

    let task = TunTask {
        tun: tun,
        handle: handle.clone(),
        frame_tx: plug_a.tx,
        frame_rx: plug_a.rx,
        sending_frame: None,
        state: TunTaskState::Receiving {
            drop_rx: drop_rx,
        },
    };

    handle.spawn(task.infallible());

    (join_handle, plug_b)
}

struct TunTask {
    tun: Ipv4Iface,
    frame_tx: UnboundedSender<Ipv4Packet>,
    frame_rx: UnboundedReceiver<Ipv4Packet>,
    sending_frame: Option<Ipv4Packet>,
    handle: Handle,
    state: TunTaskState,
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
        let grace_period: Duration = Duration::from_millis(100);

        let mut received_frames = false;
        loop {
            match self.tun.poll() {
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
                match self.tun.start_send(frame) {
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
        match self.tun.poll_complete() {
            Ok(..) => (),
            Err(e) => {
                panic!("completing TAP device write yielded an error: {}", e);
            },
        }

        let mut state = mem::replace(&mut self.state, TunTaskState::Invalid);
        trace!("polling TunTask");
        loop {
            match state {
                TunTaskState::Receiving {
                    mut drop_rx,
                } => {
                    trace!("state == receiving");
                    match drop_rx.poll().void_unwrap() {
                        Async::Ready(()) => {
                            state = TunTaskState::Dying(Timeout::new(grace_period, &self.handle));
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

