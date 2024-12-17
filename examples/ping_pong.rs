use {
    std::{
        time::Duration,
        net::SocketAddr,
        str,
    },
    tokio::net::UdpSocket,
    netsim::Machine,
    net_literals::ipv4,
};


// This example creates two network-isolated threads and gives each a network interface with which
// they send messages back-and-forth.
#[tokio::main]
async fn main() {

    // First create two machines.
    let machine_0 = Machine::new().unwrap();
    let machine_1 = Machine::new().unwrap();


    // Then give each machine a network interface.
    let ipv4_addr_0 = ipv4!("10.1.2.3");
    let port_0 = 45666;
    let addr_0 = SocketAddr::from((ipv4_addr_0, port_0));

    let ipv4_addr_1 = ipv4!("192.168.5.5");
    let port_1 = 5555;
    let addr_1 = SocketAddr::from((ipv4_addr_1, port_1));

    let iface_0 = {
        machine_0
        .add_ip_iface()
        .ipv4_addr(ipv4_addr_0)
        .ipv4_default_route()
        .build()
        .unwrap()
    };
    let iface_1 = {
        machine_1
        .add_ip_iface()
        .ipv4_addr(ipv4_addr_1)
        .ipv4_default_route()
        .build()
        .unwrap()
    };


    // Connect the network interfaces directly to each other.
    netsim::connect(iface_0, iface_1);


    // Execute a task on machine 0. This task waits to receive a UDP packet then sends a reply.
    let join_handle_0 = machine_0.spawn(async move {
        let socket = UdpSocket::bind(addr_0).await.unwrap();

        let mut recv_bytes = [0u8; 100];
        let (recv_len, peer_addr) = socket.recv_from(&mut recv_bytes).await.unwrap();
        assert_eq!(peer_addr, addr_1);
        let recv_msg = str::from_utf8(&recv_bytes[..recv_len]).unwrap();
        println!("received msg: '{recv_msg}'");

        let send_msg = "pong";
        let send_len = socket.send_to(send_msg.as_bytes(), addr_1).await.unwrap();
        assert_eq!(send_len, send_msg.len());
        println!("sent reply: '{send_msg}'");
    });
    // Execute a task on machine 1. This task sends UDP packets until it receives a reply.
    let join_handle_1 = machine_1.spawn(async move {
        let socket = UdpSocket::bind(addr_1).await.unwrap();
        let mut recv_bytes = [0u8; 100];

        let (recv_len, peer_addr) = loop {
            let send_msg = "ping";
            let send_len = socket.send_to(send_msg.as_bytes(), addr_0).await.unwrap();
            assert_eq!(send_len, send_msg.len());
            println!("sent msg: '{send_msg}'");

            tokio::select! {
                recv_result = socket.recv_from(&mut recv_bytes) => break recv_result.unwrap(),
                () = tokio::time::sleep(Duration::from_secs(1)) => (),
            }
        };
        assert_eq!(peer_addr, addr_0);
        let recv_msg = str::from_utf8(&recv_bytes[..recv_len]).unwrap();
        println!("received reply: '{recv_msg}'");
    });


    // Wait for both machines to run their tasks to completion.
    let () = join_handle_0.await.unwrap().unwrap();
    let () = join_handle_1.await.unwrap().unwrap();
}
