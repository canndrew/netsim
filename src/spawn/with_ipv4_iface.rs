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
        packet_tx: plug_a.tx,
        packet_rx: plug_a.rx,
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
    packet_tx: UnboundedSender<Ipv4Packet>,
    packet_rx: UnboundedReceiver<Ipv4Packet>,
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
        trace!("polling TunTask");
        let grace_period: Duration = Duration::from_millis(100);

        let mut received_frames = false;
        loop {
            match self.tun.poll() {
                Ok(Async::Ready(Some(frame))) => {
                    let _ = self.packet_tx.unbounded_send(frame);
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
            if let Some(frame) = self.sending_frame.take() {
                trace!("we have a frame ready to send");
                match self.tun.start_send(frame) {
                    Ok(AsyncSink::Ready) => (),
                    Ok(AsyncSink::NotReady(frame)) => {
                        trace!("couldn't send the frame ;(");
                        self.sending_frame = Some(frame);
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
                    self.sending_frame = Some(frame);
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

#[cfg(test)]
#[test]
fn test() {
    use rand;
    use void;
    use env_logger;

    let  _ = env_logger::init();

    let mut core = unwrap!(Core::new());
    let handle = core.handle();

    let res = core.run(future::lazy(move || {
        let remote_ip = Ipv4Addr::random_global();
        let remote_port = 123;
        let remote_addr = SocketAddrV4::new(remote_ip, remote_port);

        let iface_ip = Ipv4Addr::random_global();

        let iface = {
            Ipv4IfaceBuilder::new()
            .address(iface_ip)
            .route(RouteV4::new(SubnetV4::global(), None))
        };

        let (join_handle, ipv4_plug) = with_ipv4_iface(&handle, iface, move || {
            let buffer_out = rand::random::<[u8; 8]>();
            let socket = unwrap!(std::net::UdpSocket::bind(addr!("0.0.0.0:0")));
            let n = unwrap!(socket.send_to(&buffer_out, &SocketAddr::V4(remote_addr)));
            assert_eq!(n, 8);

            let mut buffer_in = [0u8; 8];
            trace!("waiting to receive reply");
            let (n, recv_addr) = unwrap!(socket.recv_from(&mut buffer_in));
            assert_eq!(n, 8);
            assert_eq!(recv_addr, SocketAddr::V4(remote_addr));
            assert_eq!(buffer_out, buffer_in);
        });

        let Ipv4Plug { tx: plug_tx, rx: plug_rx } = ipv4_plug;

        plug_rx
        .into_future()
        .map_err(|(v, _plug_rx)| void::unreachable(v))
        .and_then(move |(packet_opt, _plug_rx)| {
            let packet = unwrap!(packet_opt);
            assert_eq!(packet.source_ip(), iface_ip);
            assert_eq!(packet.dest_ip(), remote_ip);

            let udp = match packet.payload() {
                Ipv4Payload::Udp(udp) => udp,
                payload => panic!("unexpected packet payload: {:?}", payload),
            };
            assert_eq!(udp.dest_port(), remote_port);
            let iface_port = udp.source_port();

            let reply_packet = Ipv4Packet::new_from_fields_recursive(
                Ipv4Fields {
                    source_ip: remote_ip,
                    dest_ip: iface_ip,
                    ttl: 12,
                },
                Ipv4PayloadFields::Udp {
                    fields: UdpFields::V4 {
                        source_addr: remote_addr,
                        dest_addr: SocketAddrV4::new(iface_ip, iface_port),
                    },
                    payload: udp.payload(),
                },
            );

            trace!("sending reply packet");
            plug_tx
            .send(reply_packet)
            .map_err(|_e| panic!("plug hung up!"))
            .and_then(move |_plug_tx| {
                future_utils::thread_future(|| unwrap!(join_handle.join()))
            })
        })
    }));
    res.void_unwrap()
}

