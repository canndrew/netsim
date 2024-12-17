use crate::priv_prelude::*;

#[tokio::test]
async fn connect_to_outside_world() {
    let internal_addr = addrv4!("172.16.5.5:45666");
    let external_ip = ipv4!("115.70.254.200");
    let addr_1 = addrv4!("115.70.254.190:45000");

    let machine_0 = Machine::new().unwrap();
    let machine_1 = Machine::new().unwrap();

    let iface_0 = {
        machine_0
        .add_ip_iface()
        .ipv4_addr(*internal_addr.ip())
        .ipv4_default_route()
        .build()
        .unwrap()
    };
    let iface_1 = {
        machine_1
        .add_ip_iface()
        .ipv4_addr(*addr_1.ip())
        .ipv4_default_route()
        .build()
        .unwrap()
    };

    let (mut nat, nat_iface) = {
        NatBuilder::new(
            external_ip,
            Ipv4Network::infer_from_addr(*internal_addr.ip()),
        )
        .build()
    };
    nat.insert_iface(iface_0);
    
    let mut hub = IpHub::new();
    hub.insert_iface(nat_iface);
    hub.insert_iface(iface_1);

    let (ready_tx, ready_rx) = oneshot::channel();
    const MSG_0: &[u8; 4] = b"ping";
    const MSG_1: &[u8; 4] = b"pong";

    let task_0 = machine_0.spawn(async move {
        let () = ready_rx.await.unwrap();
        let mut stream = TcpStream::connect(addr_1).await.unwrap();
        let mut received_msg = [0u8; MSG_0.len()];
        stream.read_exact(&mut received_msg).await.unwrap();
        assert_eq!(received_msg, *MSG_0);
        stream.write_all(MSG_1).await.unwrap();
    });

    let task_1 = machine_1.spawn(async move {
        let listener = TcpListener::bind(addr_1).await.unwrap();
        ready_tx.send(()).unwrap();
        let (mut stream, addr) = listener.accept().await.unwrap();
        assert_eq!(addr.ip(), external_ip);
        stream.write_all(MSG_0).await.unwrap();
        let mut received_msg = [0u8; MSG_1.len()];
        stream.read_exact(&mut received_msg).await.unwrap();
        assert_eq!(received_msg, *MSG_1);
    });

    let (res_0, res_1) = futures::join!(task_0.join(), task_1.join());
    let () = res_0.unwrap().unwrap();
    let () = res_1.unwrap().unwrap();
}

#[tokio::test]
async fn nat_tcp_reset() {
    let machine_addr = addrv4!("115.70.254.190:45000");
    let nat_addr = addrv4!("115.70.254.200:45666");

    let machine = Machine::new().unwrap();
    let machine_iface = {
        machine
        .add_ip_iface()
        .ipv4_addr(*machine_addr.ip())
        .ipv4_default_route()
        .build()
        .unwrap()
    };
    let (_nat, nat_iface) = {
        NatBuilder::new(
            *nat_addr.ip(),
            Ipv4Network::new(ipv4!("192.168.0.0"), 16),
        )
        .reply_with_rst_to_unexpected_tcp_packets()
        .build()
    };

    crate::connect(machine_iface, nat_iface);

    let res = machine.spawn(async move {
        tokio::time::timeout(Duration::from_millis(100), TcpStream::connect(nat_addr)).await
    }).await.unwrap().unwrap().unwrap();
    match res {
        Ok(_) => unreachable!(),
        Err(err) => match err.kind() {
            io::ErrorKind::ConnectionRefused => (),
            kind => panic!("unexpected error kind: {}", kind),
        },
    }
}

