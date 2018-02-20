use priv_prelude::*;

pub fn new_v4(handle: &Handle, addr: Ipv4Addr, ether: EtherPlug, ipv4: Ipv4Plug) -> MacAddr {
    let mac_addr = MacAddr::random();
    let veth_v4 = VethV4 {
        ether_plug: ether,
        ipv4_plug: ipv4,
        ipv4_addr: addr,
        mac_addr: mac_addr,
        arp_table: HashMap::new(),
        arp_pending: HashMap::new(),
    };
    handle.spawn(veth_v4.infallible());
    mac_addr
}

struct VethV4 {
    ether_plug: EtherPlug,
    ipv4_plug: Ipv4Plug,
    ipv4_addr: Ipv4Addr,
    mac_addr: MacAddr,
    arp_table: HashMap<Ipv4Addr, MacAddr>,
    arp_pending: HashMap<Ipv4Addr, Vec<Ipv4Packet>>,
}

impl Future for VethV4 {
    type Item = ();
    type Error = Void;

    fn poll(&mut self) -> Result<Async<()>, Void> {
        let ether_unplugged = loop {
            match self.ether_plug.rx.poll().void_unwrap() {
                Async::NotReady => break false,
                Async::Ready(None) => break true,
                Async::Ready(Some(frame)) => {
                    if !(frame.dest_mac().is_broadcast() || frame.dest_mac() == self.mac_addr) {
                        continue;
                    }
                    match frame.payload() {
                        EtherPayload::Arp(arp) => {
                            match arp.fields() {
                                ArpFields::Request {
                                    source_mac,
                                    source_ip,
                                    dest_ip,
                                } => {
                                    if dest_ip == self.ipv4_addr {
                                        let frame = EtherFrame::new_from_fields_recursive(
                                            EtherFields {
                                                source_mac: self.mac_addr,
                                                dest_mac: source_mac,
                                            },
                                            EtherPayloadFields::Arp {
                                                fields: ArpFields::Response {
                                                    source_mac: self.mac_addr,
                                                    source_ip: self.ipv4_addr,
                                                    dest_mac: source_mac,
                                                    dest_ip: source_ip,
                                                },
                                            },
                                        );
                                        let _ = self.ether_plug.tx.unbounded_send(frame);
                                    }
                                },
                                ArpFields::Response {
                                    source_mac,
                                    source_ip,
                                    dest_mac,
                                    dest_ip,
                                } => {
                                    if dest_mac == self.mac_addr && dest_ip == self.ipv4_addr  {
                                        let _ = self.arp_table.insert(source_ip, source_mac);
                                        if let Some(pending) = self.arp_pending.remove(&source_ip) {
                                            for packet in pending {
                                                let frame = EtherFrame::new_from_fields(
                                                    EtherFields {
                                                        source_mac: self.mac_addr,
                                                        dest_mac: source_mac,
                                                    },
                                                    EtherPayload::Ipv4(packet),
                                                );
                                                let _ = self.ether_plug.tx.unbounded_send(frame);
                                            }
                                        }
                                    }
                                },
                            }
                        },
                        EtherPayload::Ipv4(ipv4) => {
                            let _ = self.ipv4_plug.tx.unbounded_send(ipv4);
                        },
                        EtherPayload::Unknown { .. } => (),
                    }
                },
            }
        };

        let ipv4_unplugged = loop {
            match self.ipv4_plug.rx.poll().void_unwrap() {
                Async::NotReady => break false,
                Async::Ready(None) => break true,
                Async::Ready(Some(packet)) => {
                    let dest_ip = packet.dest_ip();
                    let dest_mac = match self.arp_table.get(&dest_ip) {
                        Some(dest_mac) => *dest_mac,
                        None => {
                            let frame = EtherFrame::new_from_fields_recursive(
                                EtherFields {
                                    source_mac: self.mac_addr,
                                    dest_mac: MacAddr::BROADCAST,
                                },
                                EtherPayloadFields::Arp {
                                    fields: ArpFields::Request {
                                        source_mac: self.mac_addr,
                                        source_ip: self.ipv4_addr,
                                        dest_ip: dest_ip,
                                    },
                                },
                            );
                            let _ = self.ether_plug.tx.unbounded_send(frame);
                            self.arp_pending.entry(dest_ip).or_insert(Vec::new()).push(packet);
                            continue;
                        },
                    };
                    let frame = EtherFrame::new_from_fields(
                        EtherFields {
                            source_mac: self.mac_addr,
                            dest_mac: dest_mac,
                        },
                        EtherPayload::Ipv4(packet),
                    );
                    let _ = self.ether_plug.tx.unbounded_send(frame);
                },
            }
        };

        if ether_unplugged && ipv4_unplugged {
            return Ok(Async::Ready(()));
        }

        Ok(Async::NotReady)
    }
}

#[test]
fn test() {
    use rand;
    use void;

    let mut core = unwrap!(Core::new());
    let handle = core.handle();
    let res = core.run({
        let (ether_plug_0, ether_plug_1) = EtherPlug::new_wire();
        let (ipv4_plug_0, ipv4_plug_1) = Ipv4Plug::new_wire();
        let veth_ip = Ipv4Addr::random_global();
        let veth_mac = new_v4(&handle, veth_ip, ether_plug_0, ipv4_plug_0);

        let EtherPlug { tx: ether_tx, rx: ether_rx } = ether_plug_1;
        let Ipv4Plug { tx: ipv4_tx, rx: ipv4_rx } = ipv4_plug_1;

        let source_ip = Ipv4Addr::random_global();
        let dest_ip = Ipv4Addr::random_global();
        let ipv4_packet = Ipv4Packet::new_from_fields_recursive(
            Ipv4Fields {
                source_ip: source_ip,
                dest_ip: dest_ip,
                ttl: rand::random(),
            },
            Ipv4PayloadFields::Udp {
                fields: UdpFields::V4 {
                    source_addr: SocketAddrV4::new(source_ip, rand::random()),
                    dest_addr: SocketAddrV4::new(dest_ip, rand::random()),
                },
                payload: Bytes::from(&rand::random::<[u8; 8]>()[..]),
            },
        );

        let frame_with_ipv4_packet = EtherFrame::new_from_fields(
            EtherFields {
                source_mac: MacAddr::random(),
                dest_mac: veth_mac,
            },
            EtherPayload::Ipv4(ipv4_packet.clone()),
        );

        ether_tx
        .send(frame_with_ipv4_packet)
        .map_err(|_e| panic!("ether channel hung up!"))
        .and_then(move |ether_tx| {
            ipv4_rx
            .into_future()
            .map_err(|(v, _ipv4_rx)| void::unreachable(v))
            .and_then(move |(packet_opt, _ipv4_rx)| {
                let packet = unwrap!(packet_opt);
                assert_eq!(packet, ipv4_packet);

                let requester_mac = MacAddr::random();
                let requester_ip = Ipv4Addr::random_global();
                let frame = EtherFrame::new_from_fields_recursive(
                    EtherFields {
                        source_mac: requester_mac,
                        dest_mac: MacAddr::BROADCAST,
                    },
                    EtherPayloadFields::Arp {
                        fields: ArpFields::Request {
                            source_mac: requester_mac,
                            source_ip: requester_ip,
                            dest_ip: veth_ip,
                        },
                    },
                );

                ether_tx
                .send(frame)
                .map_err(|_e| panic!("ether channel hung up!"))
                .and_then(move |ether_tx| {
                    ether_rx
                    .into_future()
                    .map_err(|(v, _ether_rx)| void::unreachable(v))
                    .and_then(move |(frame_opt, ether_rx)| {
                        let frame = unwrap!(frame_opt);
                        assert_eq!(frame.fields(), EtherFields {
                            source_mac: veth_mac,
                            dest_mac: requester_mac,
                        });
                        match frame.payload() {
                            EtherPayload::Arp(arp) => {
                                assert_eq!(arp.fields(), ArpFields::Response {
                                    source_mac: veth_mac,
                                    source_ip: veth_ip,
                                    dest_mac: requester_mac,
                                    dest_ip: requester_ip,
                                });
                            },
                            payload => panic!("unexpected ether payload: {:?}", payload),
                        }

                        let source_ip = Ipv4Addr::random_global();
                        let dest_ip = Ipv4Addr::random_global();
                        let ipv4_packet = Ipv4Packet::new_from_fields_recursive(
                            Ipv4Fields {
                                source_ip: source_ip,
                                dest_ip: dest_ip,
                                ttl: rand::random(),
                            },
                            Ipv4PayloadFields::Udp {
                                fields: UdpFields::V4 {
                                    source_addr: SocketAddrV4::new(source_ip, rand::random()),
                                    dest_addr: SocketAddrV4::new(dest_ip, rand::random()),
                                },
                                payload: Bytes::from(&rand::random::<[u8; 8]>()[..]),
                            },
                        );

                        ipv4_tx
                        .send(ipv4_packet.clone())
                        .map_err(|_e| panic!("ipv4 channel hung up!"))
                        .and_then(move |_ipv4_tx| {
                            ether_rx
                            .into_future()
                            .map_err(|(v, _ether_rx)| void::unreachable(v))
                        })
                        .and_then(move |(frame_opt, ether_rx)| {
                            let frame = unwrap!(frame_opt);
                            assert_eq!(frame.fields(), EtherFields {
                                source_mac: veth_mac,
                                dest_mac: MacAddr::BROADCAST,
                            });
                            match frame.payload() {
                                EtherPayload::Arp(arp) => {
                                    assert_eq!(arp.fields(), ArpFields::Request {
                                        source_mac: veth_mac,
                                        source_ip: veth_ip,
                                        dest_ip: dest_ip,
                                    });
                                },
                                payload => panic!("unexpected ether payload: {:?}", payload),
                            }

                            let dest_mac = MacAddr::random();
                            let frame = EtherFrame::new_from_fields_recursive(
                                EtherFields {
                                    source_mac: dest_mac,
                                    dest_mac: veth_mac,
                                },
                                EtherPayloadFields::Arp {
                                    fields: ArpFields::Response {
                                        source_mac: dest_mac,
                                        source_ip: dest_ip,
                                        dest_mac: veth_mac,
                                        dest_ip: veth_ip,
                                    },
                                },
                            );

                            ether_tx
                            .send(frame)
                            .map_err(|_e| panic!("ether channel hung up!"))
                            .and_then(move |_ether_tx| {
                                ether_rx
                                .into_future()
                                .map_err(|(v, _ether_rx)| void::unreachable(v))
                            })
                            .map(move |(frame_opt, _ether_rx)| {
                                let frame = unwrap!(frame_opt);
                                assert_eq!(frame.fields(), EtherFields {
                                    source_mac: veth_mac,
                                    dest_mac: dest_mac,
                                });
                                match frame.payload() {
                                    EtherPayload::Ipv4(packet) => {
                                        assert_eq!(packet, ipv4_packet);
                                    },
                                    payload => panic!("unexpected ether payload: {:?}", payload),
                                }
                            })
                        })
                    })
                })
            })
        })
    });
    res.void_unwrap()
}

