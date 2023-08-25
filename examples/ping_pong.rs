use {
    std::{
        net::{SocketAddr, SocketAddrV4},
        future::IntoFuture,
    },
    tokio::net::UdpSocket,
    netsim_ng::Machine,
    net_literals::ipv4,
    futures::join,
};

#[tokio::main]
async fn main() {
    let ipv4_addr_0 = ipv4!("10.1.2.3");
    let port_0 = 45666;
    let addr_0 = SocketAddr::V4(SocketAddrV4::new(ipv4_addr_0, port_0));

    let ipv4_addr_1 = ipv4!("10.5.5.5");
    let port_1 = 5555;
    let addr_1 = SocketAddr::V4(SocketAddrV4::new(ipv4_addr_1, port_1));

    let machine_0 = Machine::new().unwrap();
    let machine_1 = Machine::new().unwrap();

    let iface_0 = {
        machine_0
        .add_ip_iface()
        .ipv4_addr(ipv4_addr_0)
        .await
        .unwrap()
    };
    let iface_1 = {
        machine_1
        .add_ip_iface()
        .ipv4_addr(ipv4_addr_1)
        .await
        .unwrap()
    };
    netsim_ng::connect(iface_0, iface_1);
    let join_handle_0 = machine_0.spawn(async move {
        let socket = UdpSocket::bind(addr_0).await.unwrap();

        let mut bytes = [0u8; 100];
        let (recv_len, peer_addr) = socket.recv_from(&mut bytes).await.unwrap();
        assert_eq!(peer_addr, addr_1);
        println!("received msg");

        let send_len = socket.send_to(&bytes[..recv_len], addr_1).await.unwrap();
        assert_eq!(recv_len, send_len);
        println!("sent reply");
    }).await;
    let join_handle_1 = machine_1.spawn(async move {
        let socket = UdpSocket::bind(addr_1).await.unwrap();

        let send_bytes = b"hello";
        let send_len = socket.send_to(send_bytes, addr_0).await.unwrap();
        assert_eq!(send_len, send_bytes.len());
        println!("sent msg");

        let mut recv_bytes = [0u8; 100];
        let (recv_len, peer_addr) = socket.recv_from(&mut recv_bytes).await.unwrap();
        assert_eq!(peer_addr, addr_0);
        assert_eq!(send_bytes, &recv_bytes[..recv_len]);
        println!("received reply");
    }).await;
    let (task_res_0, task_res_1) = join!(join_handle_0.into_future(), join_handle_1.into_future());
    let () = task_res_0.unwrap().unwrap();
    let () = task_res_1.unwrap().unwrap();
}
