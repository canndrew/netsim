use {
    tokio::net::UdpSocket,
    netsim::{
        Machine,
        packet::{IpPacketVersion, Ipv4PacketProtocol},
    },
    net_literals::addrv4,
    futures::prelude::stream::StreamExt,
};

#[tokio::main]
async fn main() {
    let addr = addrv4!("10.1.2.3:5555");

    let machine = Machine::new().unwrap();
    let mut iface = {
        machine
        .add_ip_iface()
        .ipv4_addr(*addr.ip())
        .ipv4_default_route()
        .build()
        .unwrap()
    };
    machine.spawn(async move {
        let socket = UdpSocket::bind(addr).await.unwrap();
        socket.send_to(b"hello", addrv4!("1.1.1.1:80")).await.unwrap();
    }).await.unwrap();

    let packet = loop {
        let packet = iface.next().await.unwrap().unwrap();
        let IpPacketVersion::V4(packet) = packet.version_box() else { continue };
        let Ipv4PacketProtocol::Udp(packet) = packet.protocol_box() else { continue };
        break packet;
    };
    assert_eq!(packet.data(), b"hello");
}

