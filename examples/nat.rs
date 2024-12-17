use {
    std::{
        str,
        str::FromStr,
        time::Duration,
        net::SocketAddr,
    },
    netsim::{
        Machine, ipv4_network, Ipv4Network,
        device::{IpHub, NatBuilder},
    },
    net_literals::addrv4,
    tokio::net::UdpSocket,
};

#[tokio::main]
async fn main() {
    let mut rng = rand::thread_rng();

    let machine_0 = Machine::new().unwrap();
    let machine_1 = Machine::new().unwrap();
    
    let network_0 = ipv4_network!("192.168.0.0/16");
    let ipv4_addr_0 = network_0.random_addr(&mut rng);
    let iface_0 = {
        machine_0
        .add_ip_iface()
        .ipv4_addr(ipv4_addr_0)
        .ipv4_default_route()
        .build()
        .unwrap()
    };
    let global_ipv4_addr_0 = Ipv4Network::GLOBAL.random_addr(&mut rng);
    let (mut nat_0, nat_iface_0) = NatBuilder::new(global_ipv4_addr_0, network_0).port_restricted().build();
    nat_0.insert_iface(iface_0);

    let network_1 = ipv4_network!("10.0.0.0/8");
    let ipv4_addr_1 = network_1.random_addr(&mut rng);
    let iface_1 = {
        machine_1
        .add_ip_iface()
        .ipv4_addr(ipv4_addr_1)
        .ipv4_default_route()
        .build()
        .unwrap()
    };
    let global_ipv4_addr_1 = Ipv4Network::GLOBAL.random_addr(&mut rng);
    let (mut nat_1, nat_iface_1) = NatBuilder::new(global_ipv4_addr_1, network_1).port_restricted().build();
    nat_1.insert_iface(iface_1);

    let machine_rendezvous = Machine::new().unwrap();
    let ipv4_addr_rendezvous = Ipv4Network::GLOBAL.random_addr(&mut rng);
    let port_rendezvous = 12345;
    let addr_rendezvous = SocketAddr::from((ipv4_addr_rendezvous, port_rendezvous));
    let iface_rendezvous = {
        machine_rendezvous
        .add_ip_iface()
        .ipv4_addr(ipv4_addr_rendezvous)
        .ipv4_default_route()
        .build()
        .unwrap()
    };


    let mut hub = IpHub::new();
    hub.insert_iface(nat_iface_0);
    hub.insert_iface(nat_iface_1);
    hub.insert_iface(iface_rendezvous);

    println!("machine 0 has local ip {} and global ip {}", ipv4_addr_0, global_ipv4_addr_0);
    println!("machine 1 has local ip {} and global ip {}", ipv4_addr_1, global_ipv4_addr_1);
    println!("rendezvous machine has ip {}", ipv4_addr_rendezvous);

    let join_handle_rendezvous = machine_rendezvous.spawn(async move {
        let socket = UdpSocket::bind(addr_rendezvous).await.unwrap();

        let peer_addr_0 = {
            let mut recv_bytes = [0u8; 100];
            let (recv_len, peer_addr_0) = socket.recv_from(&mut recv_bytes).await.unwrap();
            assert_eq!(recv_len, 0);
            peer_addr_0
        };
        println!("rendevous node received packet from {}", peer_addr_0);

        let peer_addr_1 = loop {
            let mut recv_bytes = [0u8; 100];
            let (recv_len, peer_addr_1) = socket.recv_from(&mut recv_bytes).await.unwrap();
            assert_eq!(recv_len, 0);
            if peer_addr_1 == peer_addr_0 {
                println!("rendezvous node ignoring extra packet from {}", peer_addr_0);
                continue;
            }
            break peer_addr_1
        };
        println!("rendevous node received packet from {}", peer_addr_1);

        let peer_addr_0_str = peer_addr_0.to_string();
        let peer_addr_1_str = peer_addr_1.to_string();
        socket.send_to(peer_addr_0_str.as_bytes(), peer_addr_1).await.unwrap();
        socket.send_to(peer_addr_1_str.as_bytes(), peer_addr_0).await.unwrap();
    });
    let join_handle_0 = machine_0.spawn(async move {
        let socket = UdpSocket::bind(addrv4!("0.0.0.0:0")).await.unwrap();
        let mut recv_bytes = [0u8; 100];

        let peer_addr_1 = {
            let (recv_len, recv_addr) = loop {
                socket.send_to(&[], addr_rendezvous).await.unwrap();
                println!("machine 0 sending to {}", addr_rendezvous);
                tokio::select! {
                    recv_result = socket.recv_from(&mut recv_bytes) => break recv_result.unwrap(),
                    () = tokio::time::sleep(Duration::from_secs(1)) => (),
                }
            };
            assert_eq!(recv_addr, addr_rendezvous);
            let recv_msg = str::from_utf8(&recv_bytes[..recv_len]).unwrap();
            SocketAddr::from_str(recv_msg).unwrap()
        };
        let (recv_len, recv_addr) = loop {
            socket.send_to("hello from machine 0".as_bytes(), peer_addr_1).await.unwrap();
            tokio::select! {
                recv_result = socket.recv_from(&mut recv_bytes) => break recv_result.unwrap(),
                () = tokio::time::sleep(Duration::from_secs(1)) => (),
            }
        };
        assert_eq!(recv_addr, peer_addr_1);
        let recv_msg = str::from_utf8(&recv_bytes[..recv_len]).unwrap();
        assert_eq!(recv_msg, "hello from machine 1");
        socket.send_to("hello from machine 0".as_bytes(), peer_addr_1).await.unwrap();
    });
    let join_handle_1 = machine_1.spawn(async move {
        let socket = UdpSocket::bind(addrv4!("0.0.0.0:0")).await.unwrap();
        let mut recv_bytes = [0u8; 100];

        let peer_addr_1 = {
            let (recv_len, recv_addr) = loop {
                socket.send_to(&[], addr_rendezvous).await.unwrap();
                println!("machine 1 sending to {}", addr_rendezvous);
                tokio::select! {
                    recv_result = socket.recv_from(&mut recv_bytes) => break recv_result.unwrap(),
                    () = tokio::time::sleep(Duration::from_secs(1)) => (),
                }
            };
            assert_eq!(recv_addr, addr_rendezvous);
            let recv_msg = str::from_utf8(&recv_bytes[..recv_len]).unwrap();
            SocketAddr::from_str(recv_msg).unwrap()
        };
        let (recv_len, recv_addr) = loop {
            socket.send_to("hello from machine 1".as_bytes(), peer_addr_1).await.unwrap();
            tokio::select! {
                recv_result = socket.recv_from(&mut recv_bytes) => break recv_result.unwrap(),
                () = tokio::time::sleep(Duration::from_secs(1)) => (),
            }
        };
        assert_eq!(recv_addr, peer_addr_1);
        let recv_msg = str::from_utf8(&recv_bytes[..recv_len]).unwrap();
        assert_eq!(recv_msg, "hello from machine 0");
        socket.send_to("hello from machine 1".as_bytes(), peer_addr_1).await.unwrap();
    });

    let () = join_handle_0.await.unwrap().unwrap();
    let () = join_handle_1.await.unwrap().unwrap();
    let () = join_handle_rendezvous.await.unwrap().unwrap();
}

