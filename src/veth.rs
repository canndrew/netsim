use priv_prelude::*;
use future_utils;

pub struct VethV4 {
    outgoing_tx: UnboundedSender<EthernetFrame<Bytes>>,
    outgoing_rx: UnboundedReceiver<EthernetFrame<Bytes>>,
    incoming_tx: UnboundedSender<Ipv4Packet<Bytes>>,
    incoming_rx: UnboundedReceiver<Ipv4Packet<Bytes>>,
    waiting_on_mac: HashMap<Ipv4Addr, Vec<Ipv4Packet<Bytes>>>,
    arp_table: HashMap<Ipv4Addr, EthernetAddress>,
    routes: Vec<RouteV4>,
    mac_addr: EthernetAddress,
    ip_addr: Ipv4Addr,
}

impl VethV4 {
    pub fn new(mac_addr: EthernetAddress, ip_addr: Ipv4Addr,) -> VethV4 {
        let (outgoing_tx, outgoing_rx) = future_utils::mpsc::unbounded();
        let (incoming_tx, incoming_rx) = future_utils::mpsc::unbounded();
        let waiting_on_mac = HashMap::new();
        let arp_table = HashMap::new();
        let routes = Vec::new();
        VethV4 {
            outgoing_tx,
            outgoing_rx,
            incoming_tx,
            incoming_rx,
            waiting_on_mac,
            arp_table,
            routes,
            mac_addr,
            ip_addr,
        }
    }

    pub fn ip(&self) -> Ipv4Addr {
        self.ip_addr
    }

    pub fn mac(&self) -> EthernetAddress {
        self.mac_addr
    }

    pub fn add_route(&mut self, route: RouteV4) {
        self.routes.push(route);
    }

    pub fn send_packet(&mut self, packet: Ipv4Packet<Bytes>) {
        let dest_ip = packet.dst_addr().into();
        let mut correct_route = None;
        for route in &self.routes {
            if route.destination().contains(dest_ip) {
                correct_route = Some(route.gateway());
                break;
            }
        }
        let correct_route = match correct_route {
            Some(correct_route) => correct_route,
            None => {
                trace!("no route for packet. Dropping.");
                return;
            },
        };
        let next_ip = match correct_route {
            Some(gateway) => gateway,
            None => dest_ip,
        };
        
        match self.arp_table.get(&next_ip) {
            Some(dest_mac) => {
                let frame = EthernetFrame::new_ipv4(self.mac_addr, *dest_mac, &packet);
                let _ = self.outgoing_tx.unbounded_send(frame);
            },
            None => {
                let arp = ArpPacket::new_request(
                    self.mac_addr,
                    self.ip_addr,
                    dest_ip,
                );
                let frame = EthernetFrame::new_arp(
                    self.mac_addr,
                    EthernetAddress::BROADCAST,
                    &arp,
                );
                let _ = self.outgoing_tx.unbounded_send(frame);
                self.waiting_on_mac.entry(next_ip).or_insert(Vec::new()).push(packet);
            },
        }
    }

    pub fn recv_frame(&mut self, frame: EthernetFrame<Bytes>) {
        if frame.dst_addr() != EthernetAddress::BROADCAST
            && frame.dst_addr() != self.mac_addr
        {
            trace!("veth dropping frame: {:?}", frame);
            return;
        }

        match frame.ethertype() {
            EthernetProtocol::Ipv6 => (),
            EthernetProtocol::Unknown(..) => (),
            EthernetProtocol::Ipv4 => {
                let frame_ref = EthernetFrame::new(frame.as_ref());
                let ipv4 = Ipv4Packet::new(Bytes::from(frame_ref.payload()));
                let _ = self.incoming_tx.unbounded_send(ipv4);
            },
            EthernetProtocol::Arp => {
                let frame_ref = EthernetFrame::new(frame.as_ref());
                let arp = ArpPacket::new(Bytes::from(frame_ref.payload()));
                let source_ip = Ipv4Addr::from(assert_len!(4, arp.source_protocol_addr()));
                let source_mac = EthernetAddress(assert_len!(6, arp.source_hardware_addr()));
                let dest_ip = Ipv4Addr::from(assert_len!(4, arp.target_protocol_addr()));

                let _ = self.arp_table.insert(source_ip, source_mac);
                if arp.operation() == ArpOperation::Request {
                    if dest_ip == self.ip_addr {
                        let arp = ArpPacket::new_reply(
                            self.mac_addr,
                            self.ip_addr,
                            source_mac,
                            source_ip,
                        );
                        let frame = EthernetFrame::new_arp(
                            self.mac_addr,
                            source_mac,
                            &arp,
                        );

                        let _ = self.outgoing_tx.unbounded_send(frame);
                    }
                }
                if let Some(packets) = self.waiting_on_mac.remove(&source_ip) {
                    for packet in packets {
                        let frame = EthernetFrame::new_ipv4(self.mac_addr, source_mac, &packet);
                        let _ = self.outgoing_tx.unbounded_send(frame);
                    }
                }
            },
        }
    }

    pub fn next_incoming(&mut self) -> Async<Ipv4Packet<Bytes>> {
        match self.incoming_rx.poll().void_unwrap() {
            Async::Ready(Some(packet)) => Async::Ready(packet),
            Async::Ready(None) => unreachable!(),
            Async::NotReady => Async::NotReady,
        }
    }

    pub fn next_outgoing(&mut self) -> Async<EthernetFrame<Bytes>> {
        match self.outgoing_rx.poll().void_unwrap() {
            Async::Ready(Some(frame)) => Async::Ready(frame),
            Async::Ready(None) => unreachable!(),
            Async::NotReady => Async::NotReady,
        }
    }
}

