use priv_prelude::*;
use future_utils;

pub struct VethV4 {
    outgoing_tx: UnboundedSender<EtherFrame>,
    outgoing_rx: UnboundedReceiver<EtherFrame>,
    incoming_tx: UnboundedSender<Ipv4Packet>,
    incoming_rx: UnboundedReceiver<Ipv4Packet>,
    waiting_on_mac: HashMap<Ipv4Addr, Vec<Ipv4Packet>>,
    arp_table: HashMap<Ipv4Addr, MacAddr>,
    routes: Vec<RouteV4>,
    mac_addr: MacAddr,
    ip_addr: Ipv4Addr,
}

impl VethV4 {
    pub fn new(mac_addr: MacAddr, ip_addr: Ipv4Addr,) -> VethV4 {
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

    pub fn mac(&self) -> MacAddr {
        self.mac_addr
    }

    pub fn add_route(&mut self, route: RouteV4) {
        self.routes.push(route);
    }

    pub fn send_packet(&mut self, packet: Ipv4Packet) {
        let dest_ip = packet.destination();
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
                let mut frame = EtherFrame::new();
                frame.set_source(self.mac_addr);
                frame.set_destination(*dest_mac);
                frame.set_payload(EtherPayload::Ipv4(packet));
                let _ = self.outgoing_tx.unbounded_send(frame);
            },
            None => {
                let arp = ArpPacket::request(self.mac_addr, self.ip_addr, dest_ip);
                let mut frame = EtherFrame::new();
                frame.set_source(self.mac_addr);
                frame.set_destination(MacAddr::broadcast());
                frame.set_payload(EtherPayload::Arp(arp));
                let _ = self.outgoing_tx.unbounded_send(frame);
                self.waiting_on_mac.entry(next_ip).or_insert(Vec::new()).push(packet);
            },
        }
    }

    pub fn recv_frame(&mut self, frame: EtherFrame) {
        if !frame.destination().matches(self.mac_addr) {
            trace!("veth dropping frame: {:?}", frame);
            return;
        }

        match frame.payload() {
            EtherPayload::Ipv6(..) => (),
            EtherPayload::Unknown(..) => (),
            EtherPayload::Ipv4(ipv4) => {
                let _ = self.incoming_tx.unbounded_send(ipv4);
            },
            EtherPayload::Arp(arp) => {
                let _ = self.arp_table.insert(arp.source_ip(), arp.source_mac());
                if arp.operation() == ArpOperation::Request {
                    if arp.destination_ip() == self.ip_addr {
                        let arp = ArpPacket::response(arp.clone(), self.mac_addr);
                        let mut frame = EtherFrame::new();
                        frame.set_source(self.mac_addr);
                        frame.set_destination(arp.destination_mac());
                        frame.set_payload(EtherPayload::Arp(arp));
                        let _ = self.outgoing_tx.unbounded_send(frame);
                    }
                }
                if let Some(packets) = self.waiting_on_mac.remove(&arp.source_ip()) {
                    for packet in packets {
                        let mut frame = EtherFrame::new();
                        frame.set_source(self.mac_addr);
                        frame.set_destination(arp.source_mac());
                        frame.set_payload(EtherPayload::Ipv4(packet));
                        let _ = self.outgoing_tx.unbounded_send(frame);
                    }
                }
            },
        }
    }

    pub fn next_incoming(&mut self) -> Async<Ipv4Packet> {
        match self.incoming_rx.poll().void_unwrap() {
            Async::Ready(Some(packet)) => Async::Ready(packet),
            Async::Ready(None) => unreachable!(),
            Async::NotReady => Async::NotReady,
        }
    }

    pub fn next_outgoing(&mut self) -> Async<EtherFrame> {
        match self.outgoing_rx.poll().void_unwrap() {
            Async::Ready(Some(frame)) => Async::Ready(frame),
            Async::Ready(None) => unreachable!(),
            Async::NotReady => Async::NotReady,
        }
    }
}

