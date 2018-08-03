# Packet routing

In the [hello world](02_hellow_world.md) example we created one virtual network
device and sent some data to it. Now we will create two devices, connect them
together and exchange a "hello world" message.

To connect multiple virtual devices together we use
[Ipv4Router](https://docs.rs/netsim/0.2.2/netsim/device/ipv4/struct.Ipv4Router.html).
`Ipv4Router` manages packets sent by all connected devices and passes them
to appropriate devices according to destination IP address.

## Dependencies

As in the previous example we need to install some dependencies. Put this
into your Cargo.toml:

```toml
[dependencies]
netsim = "~0.2.2"
tokio-core = "0.1.12"
futures = "0.1.18"
```

and this in your `main.rs`:

```rust
extern crate netsim;
extern crate tokio_core;
extern crate futures;
```

## Server node

First we'll create a simulated server device that will be listening for incoming
UDP datagrams - identical to what we did in the [hello world](02_hello_world.md)
example.

```rust
use futures::Future;
use futures::sync::oneshot;
use netsim::{node, spawn, Ipv4Range, Network};
use tokio_core::reactor::Core;

use std::net::{SocketAddr, SocketAddrV4, UdpSocket};

fn main() {
    let mut evloop = Core::new().unwrap();
    let network = Network::new(&evloop.handle());

    let (server_addr_tx, server_addr_rx) = oneshot::channel();
    let server_recipe = node::ipv4::machine(|ip| {
        println!("[server] ip = {}", ip);

        let bind_addr = SocketAddr::V4(SocketAddrV4::new(ip, 0));
        let sock = UdpSocket::bind(bind_addr).unwrap();
        let _ = server_addr_tx.send(sock.local_addr().unwrap());

        let mut buf = [0; 4096];
        let (_bytes_received, addr) = sock.recv_from(&mut buf).unwrap();
        println!("[server] received: {}, from: {}", String::from_utf8(buf.to_vec()).unwrap(), addr);
    });
```

## Client node

In this example instead of constructing and sending a packet manually we will
create another virtual device and make it send packets to the server. From
netsim's perspective the client node is identical to the server node, so the
API is the same. The client behaves differently though, so the callback it's
given is also different:

```rust
let client_recipe = node::ipv4::machine(|ip| {
    println!("[client] ip = {}", ip);

    let server_addr = server_addr_rx.wait().unwrap();
    println!("[client] Got server addr: {}", server_addr);

    let sock = UdpSocket::bind("0.0.0.0:0").unwrap();
    let _ = sock.send_to(b"hello world!", server_addr).unwrap();
});
```

In this case, when the client virtual device runs it waits to receive the server's
address, then sends a UDP datagram containing "hello world!" to the server.

## Connecting nodes

We have only wrote recipes to build the server and client nodes, but we still need
to connect them together and spawn them on a virtual network. As mentioned earlier
`Ipv4Router` connects multiple devices together. But instead of constructing an
`Ipv4Router` manually, we'll use a recipe for that too:

```rust
let router_recipe = node::ipv4::router((server_recipe, client_recipe));
let (spawn_complete, _ipv4_plug) =
    spawn::ipv4_tree(&network.handle(), Ipv4Range::global(), router_recipe);
```

`node::ipv4::router()` is a recipe that takes a tuple (or `Vec`) of simulated
devices that will be connected together via a simulated router. We then use
this recipe as the basis for our virtual network.

## Complete example

See [complete example](../examples/routing.rs) from netsim:

```shell
$ cargo run --example routing
```

## Next

This tutorial covered routing network packets between multiple devices.  All
simualted devices were publicly accessible - ie they had an
externally-addressable IP. In the [NAT tutorial](04_nat.md) we will see how we
can simulate LANs connected to the internet with the help of simulated NAT
(Network Address Translation) devices.
