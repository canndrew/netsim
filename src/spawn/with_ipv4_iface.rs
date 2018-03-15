use priv_prelude::*;
use std;
use future_utils;
use spawn;

/// Spawn a function into a new network namespace with a network interface described by `iface`.
/// Returns a `SpawnComplete` which can be used to join the spawned thread, along with a channel which
/// can be used to read/write IPv4 packets to the spawned thread's interface.
pub fn with_ipv4_iface<F, R>(
    handle: &Handle,
    iface: Ipv4IfaceBuilder,
    func: F,
) -> (SpawnComplete<R>, Ipv4Plug)
where
    R: Send + 'static,
    F: FnOnce() -> R + Send + 'static,
{
    let (tx, rx) = std::sync::mpsc::channel();
    let spawn_complete = spawn::new_namespace(move || {
        let tid = unsafe { ::sys::syscall(::libc::c_long::from(::sys::SYS_gettid)) };
        trace!("{} building tun {:?}", tid, iface);
        let (drop_tx, drop_rx) = future_utils::drop_notify();
        let tun_unbound = unwrap!(iface.build_unbound());
        unwrap!(tx.send((tun_unbound, drop_rx)));
        let ret = func();
        trace!("{} dropping TUN handle", tid);
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

    (spawn_complete, plug_b)
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
mod test {
    use super::*;

    use rand;
    use void;
    use futures::future::Loop;

    #[test]
    fn test_udp() {
        run_test(3, || {
            let mut core = unwrap!(Core::new());
            let handle = core.handle();

            let res = core.run(future::lazy(move || {
                let remote_ip = Ipv4Addr::random_global();
                let remote_port = rand::random::<u16>() / 2 + 1000;
                let remote_addr = SocketAddrV4::new(remote_ip, remote_port);

                let iface_ip = Ipv4Addr::random_global();

                let iface = {
                    Ipv4IfaceBuilder::new()
                    .address(iface_ip)
                    .route(RouteV4::new(SubnetV4::global(), None))
                };

                let (spawn_complete, ipv4_plug) = with_ipv4_iface(&handle, iface, move || {
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
                            fields: UdpFields {
                                source_port: remote_addr.port(),
                                dest_port: iface_port,
                            },
                            payload: udp.payload(),
                        },
                    );

                    trace!("sending reply packet");
                    plug_tx
                    .send(reply_packet)
                    .map_err(|_e| panic!("plug hung up!"))
                    .and_then(move |_plug_tx| {
                        spawn_complete
                        .map_err(|e| panic::resume_unwind(e))
                    })
                })
            }));
            res.void_unwrap()
        })
    }

    #[test]
    fn test_tcp_connect() {
        run_test(3, || {
            let mut core = unwrap!(Core::new());
            let handle = core.handle();

            let res = core.run(future::lazy(move || {
                let remote_ip = Ipv4Addr::random_global();
                let remote_port = rand::random::<u16>() / 2 + 1000;
                let remote_addr = SocketAddrV4::new(remote_ip, remote_port);

                let iface_ip = Ipv4Addr::random_global();

                let iface = {
                    Ipv4IfaceBuilder::new()
                    .address(iface_ip)
                    .route(RouteV4::new(SubnetV4::global(), None))
                };

                let (spawn_complete, ipv4_plug) = with_ipv4_iface(&handle, iface, move || {
                    let buffer_out = rand::random::<[u8; 8]>();
                    let mut stream = unwrap!(std::net::TcpStream::connect(&remote_addr));
                    let n = unwrap!(stream.write(&buffer_out));
                    assert_eq!(n, 8);

                    let mut buffer_in = [0u8; 8];
                    trace!("waiting to receive reply");
                    let n = unwrap!(stream.read(&mut buffer_in));
                    assert_eq!(n, 8);
                    assert_eq!(buffer_out, buffer_in);
                });

                let Ipv4Plug { tx: plug_tx, rx: plug_rx } = ipv4_plug;

                plug_rx
                .into_future()
                .map_err(|(v, _plug_rx)| void::unreachable(v))
                .and_then(move |(syn_packet_opt, plug_rx)| {
                    let syn_packet = unwrap!(syn_packet_opt);
                    assert_eq!(syn_packet.source_ip(), iface_ip);
                    assert_eq!(syn_packet.dest_ip(), remote_ip);

                    let tcp = match syn_packet.payload() {
                        Ipv4Payload::Tcp(tcp) => tcp,
                        payload => panic!("unexpected packet payload: {:?}", payload),
                    };
                    assert_eq!(tcp.dest_port(), remote_port);
                    let iface_port = tcp.source_port();
                    assert_eq!(tcp.kind(), TcpPacketKind::Syn);

                    let init_seq_num_0 = tcp.seq_num();
                    let init_seq_num_1 = rand::random::<u32>();
                    let window_size = tcp.window_size();

                    let ack_packet = Ipv4Packet::new_from_fields_recursive(
                        Ipv4Fields {
                            source_ip: remote_ip,
                            dest_ip: iface_ip,
                            ttl: 12,
                        },
                        Ipv4PayloadFields::Tcp {
                            fields: TcpFields {
                                source_port: remote_addr.port(),
                                dest_port: iface_port,
                                seq_num: init_seq_num_1,
                                ack_num: init_seq_num_0.wrapping_add(1),
                                window_size: window_size,
                                kind: TcpPacketKind::SynAck,
                            },
                            payload: Bytes::new(),
                        },
                    );

                    trace!("sending SYN-ACK packet");
                    plug_tx
                    .send(ack_packet)
                    .map_err(|_e| panic!("plug hung up!"))
                    .and_then(move |plug_tx| {
                        future::loop_fn((
                            plug_tx,
                            plug_rx,
                            init_seq_num_0.wrapping_add(1),
                            init_seq_num_1.wrapping_add(1),
                        ), move |(plug_tx, plug_rx, seq_num_0, seq_num_1)| {
                            plug_rx
                            .into_future()
                            .map_err(|(v, _plug_rx)| void::unreachable(v))
                            .and_then(move |(packet_opt, plug_rx)| {
                                let packet = unwrap!(packet_opt);
                                trace!("received ACK packet: {:?}", packet);
                                assert_eq!(packet.source_ip(), iface_ip);
                                assert_eq!(packet.dest_ip(), remote_ip);
                                let tcp = match packet.payload() {
                                    Ipv4Payload::Tcp(tcp) => tcp,
                                    payload => panic!("unexpected packet payload: {:?}", payload),
                                };
                                assert_eq!(tcp.dest_port(), remote_port);
                                assert_eq!(tcp.source_port(), iface_port);
                                assert_eq!(tcp.seq_num(), seq_num_0);
                                assert_eq!(tcp.ack_num(), seq_num_1);
                                let next_seq_num_0 = seq_num_0.wrapping_add(tcp.payload().len() as u32);
                                let next_seq_num_1 = seq_num_1.wrapping_add(tcp.payload().len() as u32);
                                match tcp.kind() {
                                    TcpPacketKind::Ack => (),
                                    TcpPacketKind::Fin => {
                                        return future::ok(Loop::Break((
                                            plug_tx,
                                            plug_rx,
                                            next_seq_num_0,
                                            next_seq_num_1,
                                        ))).into_boxed();
                                    },
                                    kind => panic!("unexpected TCP packet kind: {:?}", kind),
                                }
                                
                                let ack_packet = Ipv4Packet::new_from_fields_recursive(
                                    Ipv4Fields {
                                        source_ip: remote_ip,
                                        dest_ip: iface_ip,
                                        ttl: 12,
                                    },
                                    Ipv4PayloadFields::Tcp {
                                        fields: TcpFields {
                                            source_port: remote_addr.port(),
                                            dest_port: iface_port,
                                            seq_num: seq_num_1,
                                            ack_num: next_seq_num_0,
                                            window_size: window_size,
                                            kind: TcpPacketKind::Ack,
                                        },
                                        payload: tcp.payload(),
                                    },
                                );

                                trace!("\n\n\t\tSENDING {} bytes!", tcp.payload().len());

                                plug_tx
                                .send(ack_packet)
                                .map_err(|_e| panic!("plug hung up!"))
                                .map(move |plug_tx| {
                                    Loop::Continue((
                                        plug_tx,
                                        plug_rx,
                                        next_seq_num_0,
                                        next_seq_num_1,
                                    ))
                                })
                                .into_boxed()
                            })
                        })
                        .and_then(move |(plug_tx, plug_rx, seq_num_0, seq_num_1)| {
                            let fin_ack_packet = Ipv4Packet::new_from_fields_recursive(
                                Ipv4Fields {
                                    source_ip: remote_ip,
                                    dest_ip: iface_ip,
                                    ttl: 12,
                                },
                                Ipv4PayloadFields::Tcp {
                                    fields: TcpFields {
                                        source_port: remote_addr.port(),
                                        dest_port: iface_port,
                                        seq_num: seq_num_1,
                                        ack_num: seq_num_0,
                                        window_size: window_size,
                                        kind: TcpPacketKind::Ack,
                                    },
                                    payload: tcp.payload(),
                                },
                            );

                            plug_tx
                            .send(fin_ack_packet)
                            .map_err(|_e| panic!("plug hung up!"))
                            .and_then(move |plug_tx| {
                                let fin_packet = Ipv4Packet::new_from_fields_recursive(
                                    Ipv4Fields {
                                        source_ip: remote_ip,
                                        dest_ip: iface_ip,
                                        ttl: 12,
                                    },
                                    Ipv4PayloadFields::Tcp {
                                        fields: TcpFields {
                                            source_port: remote_addr.port(),
                                            dest_port: iface_port,
                                            seq_num: seq_num_1,
                                            ack_num: seq_num_0,
                                            window_size: window_size,
                                            kind: TcpPacketKind::Fin,
                                        },
                                        payload: tcp.payload(),
                                    },
                                );

                                plug_tx
                                .send(fin_packet)
                                .map_err(|_e| panic!("plug hung up!"))
                                .and_then(move |_plug_tx| {
                                    plug_rx
                                    .into_future()
                                    .map_err(|(v, _plug_rx)| void::unreachable(v))
                                    .and_then(move |(packet_opt, _plug_rx)| {
                                        let packet = unwrap!(packet_opt);

                                        assert_eq!(packet.source_ip(), iface_ip);
                                        assert_eq!(packet.dest_ip(), remote_ip);
                                        let tcp = match packet.payload() {
                                            Ipv4Payload::Tcp(tcp) => tcp,
                                            payload => panic!("unexpected packet payload: {:?}", payload),
                                        };
                                        assert_eq!(tcp.dest_port(), remote_port);
                                        assert_eq!(tcp.source_port(), iface_port);
                                        assert_eq!(tcp.kind(), TcpPacketKind::Ack);

                                        spawn_complete
                                        .map_err(|e| panic::resume_unwind(e))
                                    })
                                })
                            })
                        })
                    })
                })
            }));
            res.void_unwrap()
        })
    }

    #[test]
    fn test_ping_reply() {
        run_test(3, || {
            let mut core = unwrap!(Core::new());
            let handle = core.handle();

            let res = core.run(future::lazy(move || {
                let (done_tx, done_rx) = std::sync::mpsc::channel();

                let client_ip = Ipv4Addr::random_global();
                let iface_ip = Ipv4Addr::random_global();

                let iface = {
                    Ipv4IfaceBuilder::new()
                    .address(iface_ip)
                    .route(RouteV4::new(SubnetV4::global(), None))
                };

                let (spawn_complete, ipv4_plug) = with_ipv4_iface(&handle, iface, move || {
                    unwrap!(done_rx.recv());
                });

                let Ipv4Plug { tx, rx } = ipv4_plug;

                let id = rand::random();
                let seq_num = rand::random();
                let payload = Bytes::from(&rand::random::<[u8; 8]>()[..]);
                let ping = Ipv4Packet::new_from_fields_recursive(
                    Ipv4Fields {
                        source_ip: client_ip,
                        dest_ip: iface_ip,
                        ttl: 16,
                    },
                    Ipv4PayloadFields::Icmp {
                        kind: Icmpv4PacketKind::EchoRequest {
                            id, seq_num,
                            payload: payload.clone(),
                        },
                    },
                );

                tx
                .send(ping)
                .map_err(|_e| panic!("interface hung up!"))
                .and_then(move |_tx| {
                    rx
                    .into_future()
                    .map_err(|(v, _rx)| void::unreachable(v))
                    .and_then(move |(packet_opt, _rx)| {
                        let packet = unwrap!(packet_opt);
                        let icmp = match packet.payload() {
                            Ipv4Payload::Icmp(icmp) => icmp,
                            payload => panic!("unexpected ipv4 payload kind in reply: {:?}", payload),
                        };
                        match icmp.kind() {
                            Icmpv4PacketKind::EchoReply { 
                                id: reply_id,
                                seq_num: reply_seq_num,
                                payload: reply_payload,
                            } => {
                                assert_eq!(id, reply_id);
                                assert_eq!(seq_num, reply_seq_num);
                                assert_eq!(payload, reply_payload);
                            },
                            kind => panic!("unexpected ICMP reply kind: {:?}", kind),
                        }
                        unwrap!(done_tx.send(()));

                        spawn_complete
                        .map_err(|e| panic::resume_unwind(e))
                    })
                })
            }));
            res.void_unwrap()
        })
    }
}
