use priv_prelude::*;

/// A node representing an Ipv4 machine.
pub struct MachineNode<F> {
    func: F,
}

/// Create a node for an Ipv4 machine. This node will run the given function in a network
/// namespace with a single interface in a separate thread of it's own.
pub fn machine<R, F>(func: F) -> MachineNode<F>
where
    R: Send + 'static,
    F: FnOnce(Ipv4Addr) -> R + Send + 'static,
{
    MachineNode { func }
}

impl<R, F> Ipv4Node for MachineNode<F>
where
    R: Send + 'static,
    F: FnOnce(Ipv4Addr) -> R + Send + 'static,
{
    type Output = R;

    fn build(
        self,
        handle: &NetworkHandle,
        ipv4_range: Ipv4Range,
    ) -> (SpawnComplete<R>, Ipv4Plug) {
        let address = ipv4_range.random_client_addr();
        let iface = {
            IpIfaceBuilder::new()
            .ipv4_addr(address, ipv4_range.netmask_prefix_length())
            .ipv4_route(Ipv4Route::new(Ipv4Range::global(), None))
        };
        let (plug_a, plug_b) = IpPlug::new_pair();

        let spawn_complete = {
            MachineBuilder::new()
            .add_ip_iface(iface, plug_b)
            .spawn(handle, move || (self.func)(address))
        };

        let plug_a = plug_a.into_ipv4_plug(handle);

        (spawn_complete, plug_a)
    }
}

#[cfg(feature = "linux_host")]
#[cfg(test)]
mod test {
    use super::*;

    use std;
    use rand;
    use void;
    use spawn;
    use node;
    use futures::future::Loop;

    #[test]
    fn test_udp() {
        run_test(3, || {
            let mut core = unwrap!(Core::new());
            let network = Network::new(&core.handle());
            let handle = network.handle();

            let res = core.run(future::lazy(move || {
                let remote_ip = Ipv4Addr::random_global();
                let remote_port = rand::random::<u16>() / 2 + 1000;
                let remote_addr = SocketAddrV4::new(remote_ip, remote_port);

                let ipv4_range = Ipv4Range::random_local_subnet();
                let (ipv4_addr_tx, ipv4_addr_rx) = std::sync::mpsc::channel();
                let (spawn_complete, ipv4_plug) = spawn::ipv4_tree(
                    &handle,
                    ipv4_range,
                    node::ipv4::machine(move |ipv4_addr| {
                        unwrap!(ipv4_addr_tx.send(ipv4_addr));
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
                    }),
                );

                let (plug_tx, plug_rx) = ipv4_plug.split();
                let iface_ip = unwrap!(ipv4_addr_rx.recv());

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
            let network = Network::new(&core.handle());
            let handle = network.handle();

            let res = core.run(future::lazy(move || {
                let remote_ip = Ipv4Addr::random_global();
                let remote_port = rand::random::<u16>() / 2 + 1000;
                let remote_addr = SocketAddrV4::new(remote_ip, remote_port);

                let ipv4_range = Ipv4Range::random_local_subnet();
                let (ipv4_addr_tx, ipv4_addr_rx) = std::sync::mpsc::channel();
                let (spawn_complete, ipv4_plug) = spawn::ipv4_tree(
                    &handle,
                    ipv4_range,
                    node::ipv4::machine(move |ipv4_addr| {
                        unwrap!(ipv4_addr_tx.send(ipv4_addr));
                        let buffer_out = rand::random::<[u8; 8]>();
                        let mut stream = unwrap!(std::net::TcpStream::connect(&remote_addr));
                        let n = unwrap!(stream.write(&buffer_out));
                        assert_eq!(n, 8);

                        let mut buffer_in = [0u8; 8];
                        trace!("waiting to receive reply");
                        let n = unwrap!(stream.read(&mut buffer_in));
                        assert_eq!(n, 8);
                        assert_eq!(buffer_out, buffer_in);
                    }),
                );

                let (plug_tx, plug_rx) = ipv4_plug.split();
                let iface_ip = unwrap!(ipv4_addr_rx.recv());

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
                    assert!(tcp.is_syn());

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
                                syn: true,
                                ack: true,
                                fin: false,
                                rst: false,
                                ns: false,
                                cwr: false,
                                ece: false,
                                psh: false,
                                urgent: None,
                                mss: None,
                                window_scale: None,
                                selective_ack_permitted: false,
                                selective_acks: None,
                                timestamps: None,
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
                                if tcp.is_fin() {
                                    return future::ok(Loop::Break((
                                        plug_tx,
                                        plug_rx,
                                        next_seq_num_0,
                                        next_seq_num_1,
                                    ))).into_boxed();
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
                                            syn: false,
                                            ack: true,
                                            fin: false,
                                            rst: false,
                                            ns: false,
                                            cwr: false,
                                            ece: false,
                                            psh: false,
                                            urgent: None,
                                            mss: None,
                                            window_scale: None,
                                            selective_ack_permitted: false,
                                            selective_acks: None,
                                            timestamps: None,
                                        },
                                        payload: tcp.payload(),
                                    },
                                );

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
                                        syn: false,
                                        ack: true,
                                        fin: false,
                                        rst: false,
                                        ns: false,
                                        cwr: false,
                                        ece: false,
                                        psh: false,
                                        urgent: None,
                                        mss: None,
                                        window_scale: None,
                                        selective_ack_permitted: false,
                                        selective_acks: None,
                                        timestamps: None,
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
                                            syn: false,
                                            ack: true,
                                            fin: true,
                                            rst: false,
                                            ns: false,
                                            cwr: false,
                                            ece: false,
                                            psh: false,
                                            urgent: None,
                                            mss: None,
                                            window_scale: None,
                                            selective_ack_permitted: false,
                                            selective_acks: None,
                                            timestamps: None,
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
                                        assert!(tcp.is_ack());

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
            let network = Network::new(&core.handle());
            let handle = network.handle();

            let res = core.run(future::lazy(move || {
                let (done_tx, done_rx) = std::sync::mpsc::channel();

                let client_ip = Ipv4Addr::random_global();

                let ipv4_range = Ipv4Range::random_local_subnet();
                let (ipv4_addr_tx, ipv4_addr_rx) = std::sync::mpsc::channel();
                let (spawn_complete, ipv4_plug) = spawn::ipv4_tree(
                    &handle,
                    ipv4_range,
                    node::ipv4::machine(move |ipv4_addr| {
                        unwrap!(ipv4_addr_tx.send(ipv4_addr));
                        unwrap!(done_rx.recv());
                    }),
                );

                let (tx, rx) = ipv4_plug.split();
                let iface_ip = unwrap!(ipv4_addr_rx.recv());

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
