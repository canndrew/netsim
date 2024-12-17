/*!
`netsim` is a library for testing networking code. It allows you to run tests in network-isolated
threads, on virtual network interfaces, in parallel, without tests interfering with each other or
with your actual network. You can also connect these isolated environments to each other
to create simulated networks on which you can capture and inject packets.

### Example 1: Isolating tests.

Suppose you have multiple tests that need to bind to the same port for some reason. By using
`#[netsim::isolate]` you can run your test suite without having to use `--test-threads=1` and
without having to stop any daemons running on your dev machine.

```no_run
#[test]
#[netsim::isolate]
fn a_test() {
    let _listener = std::net::TcpListener::bind("0.0.0.0:80").unwrap();
}

#[test]
#[netsim::isolate]
fn another_test_that_runs_in_parallel() {
    let _listener = std::net::TcpListener::bind("0.0.0.0:80").unwrap();
}
```

### Example 2: Capturing a packet.

The `#[netsim::isolate]` attribute showcased above is just a convenient way to setup a `Machine`
and spawn a task onto it. To capture a packet you'll need to do these steps yourself. You'll also
need to give the machine a network interface to send the packet on.

In this example we create a UDP socket and use it to send the message "hello" towards an arbitary
address. The packet then arrives on our `IpIface`, hoping to be routed somewhere, and we can check
that it still contains the correct message.

```rust
# use {
#     std::net::SocketAddrV4,
#     netsim::{
#         Machine,
#         packet::{IpPacketVersion, Ipv4PacketProtocol},
#     },
#     tokio::net::UdpSocket,
#     futures::prelude::stream::StreamExt,
# };
# #[tokio::main]
# async fn main() {
let local_addr: SocketAddrV4 = "10.1.2.3:5555".parse().unwrap();
let remote_addr: SocketAddrV4 = "1.1.1.1:53".parse().unwrap();
let machine = Machine::new().unwrap();
let mut iface = {
    machine
    .add_ip_iface()
    .ipv4_addr(*local_addr.ip())
    .ipv4_default_route()
    .build()
    .unwrap()
};
machine.spawn(async move {
    let socket = UdpSocket::bind(local_addr).await.unwrap();
    socket.send_to(b"hello", remote_addr).await.unwrap();
}).await.unwrap();

let packet = loop {
    let packet = iface.next().await.unwrap().unwrap();
    let IpPacketVersion::V4(packet) = packet.version_box() else { continue };
    let Ipv4PacketProtocol::Udp(packet) = packet.protocol_box() else { continue };
    break packet;
};
assert_eq!(packet.data(), b"hello");
# }
```

### More, longer examples.

Check out the [examples](https://github.com/canndrew/netsim-ng/tree/master/examples) directory in
the repo.

*/

#![allow(clippy::let_unit_value)]

extern crate self as netsim;

mod priv_prelude;
mod namespace;
mod machine;
mod iface;
mod ioctl;
mod network;
mod connect;
mod stream_ext;
pub mod adapter;
pub mod device;
pub mod packet;
mod sys;

pub use {
    machine::{Machine, JoinHandle},
    iface::{
        create::IpIfaceBuilder,
        stream::{IpIface, IpSinkStream},
    },
    connect::connect,
    network::{Ipv4Network, Ipv6Network, NetworkParseError, Ipv4NetworkIter, Ipv6NetworkIter},
    netsim_macros::{ipv4_network, ipv6_network, isolate},
    stream_ext::SinkStreamExt,
    tokio,
};

#[cfg(test)]
mod tests;

