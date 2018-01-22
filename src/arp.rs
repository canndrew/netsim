use priv_prelude::*;

#[derive(Clone)]
pub struct ArpPacket {
    data: Bytes,
}

impl fmt::Debug for ArpPacket {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f
        .debug_struct("ArpPacket")
        .field("operation", &self.operation())
        .field("source_mac", &self.source_mac())
        .field("source_ip", &self.source_ip())
        .field("destination_mac", &self.destination_mac())
        .field("destination_ip", &self.destination_ip())
        .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ArpOperation {
    Request,
    Reply,
}

impl ArpPacket {
    pub fn from_bytes(data: Bytes) -> ArpPacket {
        ArpPacket {
            data,
        }
    }

    pub fn as_bytes(&self) -> &Bytes {
        &self.data
    }

    pub fn operation(&self) -> ArpOperation {
        let op = ((self.data[6] as u16) << 8) | (self.data[7] as u16);
        match op {
            1 => ArpOperation::Request,
            2 => ArpOperation::Reply,
            _ => panic!("unknown arp operation number: {}", op),
        }
    }

    pub fn source_mac(&self) -> MacAddr {
        MacAddr::from_bytes(&self.data[8..14])
    }

    pub fn source_ip(&self) -> Ipv4Addr {
        Ipv4Addr::new(self.data[14], self.data[15], self.data[16], self.data[17])
    }

    pub fn destination_mac(&self) -> MacAddr {
        MacAddr::from_bytes(&self.data[18..24])
    }

    pub fn destination_ip(&self) -> Ipv4Addr {
        Ipv4Addr::new(self.data[24], self.data[25], self.data[26], self.data[27])
    }

    pub fn request(source_mac: MacAddr, source_ip: Ipv4Addr, destination_ip: Ipv4Addr) -> ArpPacket {
        let mut data = BytesMut::with_capacity(28);
        data.extend_from_slice(&[0x00, 0x01, 0x08, 0x00, 0x06, 0x04, 0x00, 0x01]);
        data.extend_from_slice(&source_mac.as_bytes());
        data.extend_from_slice(&source_ip.octets());
        data.extend_from_slice(&MacAddr::broadcast().as_bytes());
        data.extend_from_slice(&destination_ip.octets());
        ArpPacket {
            data: Bytes::from(data),
        }
    }

    pub fn response(self, mac: MacAddr) -> ArpPacket {
        let source_ip = self.destination_ip();
        let destination_ip = self.source_ip();
        let destination_mac = self.source_mac();

        let ArpPacket { data: bytes } = self;
        let mut bytes_mut = BytesMut::from(bytes);

        bytes_mut[6] = 0;
        bytes_mut[7] = 2;
        bytes_mut[8..14].clone_from_slice(&mac.as_bytes()[..]);
        bytes_mut[14..18].clone_from_slice(&source_ip.octets()[..]);
        bytes_mut[18..24].clone_from_slice(&destination_mac.as_bytes()[..]);
        bytes_mut[24..28].clone_from_slice(&destination_ip.octets()[..]);

        ArpPacket::from_bytes(bytes_mut.into())
    }
}

/*
pub struct ArpTable {
    table: HashMap<Ipv4Addr, MacAddr>
}

impl ArpTable {
    pub fn new() -> ArpTable {
        ArpTable {
            table: HashMap::new(),
        }
    }

    pub fn process(&mut self, arp: ArpPacket) -> Option<ArpPacket> {
        let _ = self.table.insert(arp.source_ip(), arp.source_mac());

        if arp.operation() != ArpOperation::Request {
            return None;
        }
        if arp.destination_ip() != self.cfg.subnet.gateway_addr() {
            return None;
        }
    }
}
*/

