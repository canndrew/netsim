use priv_prelude::*;
use std;
use future_utils;
use spawn;

/// Spawn a function into a new network namespace with a network interface described by `iface`.
/// Returns a `JoinHandle` which can be used to join the spawned thread, along with a `Tap` which
/// can be used to read/write network activity from the spawned thread.
pub fn with_iface<F, R>(
    handle: &Handle,
    iface: IfaceBuilder,
    func: F,
) -> (JoinHandle<R>, EtherPlug)
where
    R: Send + 'static,
    F: FnOnce() -> R + Send + 'static,
{
    let (tx, rx) = std::sync::mpsc::channel();
    let join_handle = spawn::new_namespace(move || {
        trace!("building tap {:?}", iface);
        let (drop_tx, drop_rx) = future_utils::drop_notify();
        let tap_unbound = unwrap!(iface.build_unbound());
        unwrap!(tx.send((tap_unbound, drop_rx)));
        let ret = func();
        drop(drop_tx);
        ret
    });

    let (tap_unbound, drop_rx) = unwrap!(rx.recv());
    let tap = tap_unbound.bind(handle);

    let (plug_a, plug_b) = EtherPlug::new_wire();

    let task = TapTask {
        tap: tap,
        handle: handle.clone(),
        frame_tx: plug_a.tx,
        frame_rx: plug_a.rx,
        sending_frame: None,
        state: TapTaskState::Receiving {
            drop_rx: drop_rx,
        },
    };

    handle.spawn(task.infallible());

    (join_handle, plug_b)
}

struct TapTask {
    tap: Tap,
    frame_tx: UnboundedSender<EtherFrame>,
    frame_rx: UnboundedReceiver<EtherFrame>,
    sending_frame: Option<EtherFrame>,
    handle: Handle,
    state: TapTaskState,
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

#[cfg(test)]
mod test {
    use priv_prelude::*;
    use super::*;
    use env_logger;
    use rand;
    use std;
    use void;

    #[test]
    fn one_interface_send_udp() {
        let _ = env_logger::init();
        let mut core = unwrap!(Core::new());
        let handle = core.handle();
        let res = core.run(future::lazy(|| {
            trace!("starting");
            let subnet = SubnetV4::random_local();
            let mut iface = IfaceBuilder::new();
            let iface_ip = subnet.random_client_addr();
            let gateway_ip = subnet.gateway_ip();
            iface.address(iface_ip);
            iface.netmask(subnet.netmask());
            iface.route(RouteV4::new(
                SubnetV4::new(ipv4!("0.0.0.0"), 0),
                Some(gateway_ip),
            ));

            let payload: [u8; 8] = rand::random();
            let target_ip = Ipv4Addr::random_global();
            let target_port = rand::random::<u16>() / 2 + 1000;
            let target_addr = SocketAddrV4::new(target_ip, target_port);

            trace!("spawning thread");
            let (join_handle, EtherPlug { tx, rx }) = with_iface(
                &handle,
                iface,
                move || {
                    let socket = unwrap!(std::net::UdpSocket::bind(addr!("0.0.0.0:0")));
                    unwrap!(socket.send_to(&payload[..], SocketAddr::V4(target_addr)));
                    trace!("sent udp packet");
                },
            );

            let gateway_mac = MacAddr::random();

            rx
            .into_future()
            .map_err(|(v, _rx)| void::unreachable(v))
            .and_then(move |(frame_opt, rx)| {
                let frame = unwrap!(frame_opt);
                trace!("got frame from iface: {:?}", frame);
                let iface_mac = frame.source_mac();
                let arp = match frame.payload() {
                    EtherPayload::Arp(arp) => arp,
                    payload => panic!("unexpected payload: {:?}", payload),
                };
                assert_eq!(arp.fields(), ArpFields::Request {
                    source_mac: iface_mac,
                    source_ip: iface_ip,
                    dest_ip: gateway_ip,
                });
                let frame = EtherFrame::new_from_fields_recursive(
                    EtherFields {
                        source_mac: gateway_mac,
                        dest_mac: iface_mac,
                    },
                    EtherPayloadFields::Arp {
                        fields: ArpFields::Response {
                            source_mac: gateway_mac,
                            source_ip: gateway_ip,
                            dest_mac: iface_mac,
                            dest_ip: iface_ip,
                        },
                    },
                );

                tx
                .send(frame)
                .map_err(|_e| panic!("channel hung up!"))
                .and_then(|_tx| {
                    rx
                    .into_future()
                    .map_err(|(v, _rx)| void::unreachable(v))
                })
                .map(move |(frame_opt, _rx)| {
                    let frame = unwrap!(frame_opt);
                    assert_eq!(frame.fields(), EtherFields {
                        source_mac: iface_mac,
                        dest_mac: gateway_mac,
                    });
                    let ipv4 = match frame.payload() {
                        EtherPayload::Ipv4(ipv4) => ipv4,
                        payload => panic!("unexpected payload: {:?}", payload),
                    };
                    assert_eq!(ipv4.source_ip(), iface_ip);
                    assert_eq!(ipv4.dest_ip(), target_ip);
                    let udp = match ipv4.payload() {
                        Ipv4Payload::Udp(udp) => udp,
                        payload => panic!("unexpected payload: {:?}", payload),
                    };
                    assert_eq!(udp.dest_port(), target_port);
                    assert_eq!(&udp.payload(), &payload[..]);
                    unwrap!(join_handle.join())
                })
            })
        }));
        res.void_unwrap()
    }
}

