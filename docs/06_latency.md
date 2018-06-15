# Latency

In this tutorial we will create a test network, introduce artificial latency
on some virtual devices and see how this affects packet delivery.

## Sample network

We will take an example from [NAT tutorial](04_nat.md) as a basis:

```rust
extern crate netsim;
extern crate tokio_core;
extern crate futures;

use futures::Future;
use futures::sync::oneshot;
use netsim::{node, spawn, Ipv4Range, Network};
use netsim::device::ipv4::Ipv4NatBuilder;
use tokio_core::reactor::Core;

use std::net::{SocketAddr, SocketAddrV4, UdpSocket};

fn main() {
    let mut evloop = Core::new().unwrap();
    let network = Network::new(&evloop.handle());
    let clock = Instant::now();

    let (server_addr_tx, server_addr_rx) = oneshot::channel();
    let server_recipe = node::ipv4::machine(move |ip| {
        println!("[server] ip = {}", ip);

        let bind_addr = SocketAddr::V4(SocketAddrV4::new(ip, 0));
        let sock = UdpSocket::bind(bind_addr).unwrap();
        let _ = server_addr_tx.send(sock.local_addr().unwrap());

        let mut buf = [0; 4096];
        let (_bytes_received, addr) = sock.recv_from(&mut buf).unwrap();
        println!("[server] received: {}, from: {}", String::from_utf8(buf.to_vec()).unwrap(), addr);
    });

    let client_recipe = node::ipv4::machine(|ip| {
        println!("[client] ip = {}", ip);

        let server_addr = server_addr_rx.wait().unwrap();
        println!("[client] Got server addr: {}", server_addr);

        let sock = UdpSocket::bind("0.0.0.0:0").unwrap();
        let _ = sock.send_to(b"hello world!", server_addr).unwrap();
    });
    let client_under_nat_recipe = node::ipv4::nat(Ipv4NatBuilder::default(), client_recipe);

    let router_recipe = node::ipv4::router((server_recipe, client_under_nat_recipe));
    let (spawn_complete, _ipv4_plug) =
        spawn::ipv4_tree(&network.handle(), Ipv4Range::global(), router_recipe);

    evloop.run(spawn_complete).unwrap();
}
```

This virtual network had two nodes connected together with the router and
client node is under NAT device:

```
                      +--------+
           +--------->| router |-----------------+
           |          +--------+                 |
           |                                     V
        +-----+                             +--------+
        | NAT |                             | server |
        +-----+                             +--------+
           ^
           |
      +--------+
      | client |
      +--------+
```

`client` node sends a message to server node. First we will measure how
long this simulation takes withouth any latency.

## Measure network performance

Modify `server` node to print how much time elapsed since example started
running:

```rust
use std::time::Instant;

let clock = Instant::now();

let (server_addr_tx, server_addr_rx) = oneshot::channel();
let server_recipe = node::ipv4::machine(move |ip| {
    println!("[server] ip = {}", ip);

    let bind_addr = SocketAddr::V4(SocketAddrV4::new(ip, 0));
    let sock = UdpSocket::bind(bind_addr).unwrap();
    let _ = server_addr_tx.send(sock.local_addr().unwrap());

    let mut buf = [0; 4096];
    let (_bytes_received, addr) = sock.recv_from(&mut buf).unwrap();
    println!(
        "[server] received: {}, from: {}, latency: {:?}",
        String::from_utf8(buf.to_vec()).unwrap(), addr, clock.elapsed(),
    );
});
```

Now if we run the example, we should see something like this:

```shell
$ cargo run
[server] ip = 91.253.204.108
[client] ip = 192.168.166.120
[client] Got server addr: 91.253.204.108:37083
[server] received: hello world!, from: 42.88.82.84:1000, latency: 7.303436ms
```

Notice the latency is around 7ms. Now we will introduce some latency on
`client` node.

## Introduce latency

All we have to do is chain `.latency()` function with `client` node recipe:

```rust
use netsim::node::Ipv4Node;
use std::time::Duration;

let client_recipe = node::ipv4::machine(|ip| {
    println!("[client] ip = {}", ip);

    let server_addr = server_addr_rx.wait().unwrap();
    println!("[client] Got server addr: {}", server_addr);

    let sock = UdpSocket::bind("0.0.0.0:0").unwrap();
    let _ = sock.send_to(b"hello world!", server_addr).unwrap();
}).latency(Duration::from_secs(2), Duration::from_millis(100));
let client_under_nat_recipe = node::ipv4::nat(Ipv4NatBuilder::default(), client_recipe);
```

Now if we rerun the example we should see the increased latency:

```shell
[server] ip = 76.139.189.105
[client] ip = 192.168.239.253
[client] Got server addr: 76.139.189.105:38409
[server] received: hello world!, from: 61.52.34.255:1000, latency: 2.044732788s
````

[.latency()](https://docs.rs/netsim/0.2.2/netsim/node/ipv4/trait.Ipv4Node.html#method.latency)
takes two parameters:
1. minimal latency that will always be introduced
2. some additional random latency with given average value

Actually, we can introduce latency on any virtual device, in this case client,
server, NAT or even all of them. For example let's introduce some latency
on NAT device:

```rust
let client_under_nat_recipe = node::ipv4::nat(Ipv4NatBuilder::default(), client_recipe)
    .latency(Duration::from_secs(2), Duration::from_millis(100));
```

## Complete example

Cargo.toml:

```toml
[package]
authors = ["Super Me <me@can.do>"]
name = "routing"
version = "0.1.0"

[dependencies]
netsim = "~0.2.2"
tokio-core = "0.1.12"
futures = "0.1.18"
```

main.rs:

```rust
extern crate netsim;
extern crate tokio_core;
extern crate futures;

use futures::Future;
use futures::sync::oneshot;
use netsim::{node, spawn, Ipv4Range, Network};
use netsim::node::Ipv4Node;
use netsim::device::ipv4::Ipv4NatBuilder;
use tokio_core::reactor::Core;

use std::net::{SocketAddr, SocketAddrV4, UdpSocket};
use std::time::{Duration, Instant};

fn main() {
    let mut evloop = Core::new().unwrap();
    let network = Network::new(&evloop.handle());
    let clock = Instant::now();

    let (server_addr_tx, server_addr_rx) = oneshot::channel();
    let server_recipe = node::ipv4::machine(move |ip| {
        println!("[server] ip = {}", ip);

        let bind_addr = SocketAddr::V4(SocketAddrV4::new(ip, 0));
        let sock = UdpSocket::bind(bind_addr).unwrap();
        let _ = server_addr_tx.send(sock.local_addr().unwrap());

        let mut buf = [0; 4096];
        let (_bytes_received, addr) = sock.recv_from(&mut buf).unwrap();
        println!(
            "[server] received: {}, from: {}, latency: {:?}",
            String::from_utf8(buf.to_vec()).unwrap(), addr, clock.elapsed(),
        );
    });

    let client_recipe = node::ipv4::machine(|ip| {
        println!("[client] ip = {}", ip);

        let server_addr = server_addr_rx.wait().unwrap();
        println!("[client] Got server addr: {}", server_addr);

        let sock = UdpSocket::bind("0.0.0.0:0").unwrap();
        let _ = sock.send_to(b"hello world!", server_addr).unwrap();
    }).latency(Duration::from_secs(2), Duration::from_millis(100));
    let client_under_nat_recipe = node::ipv4::nat(Ipv4NatBuilder::default(), client_recipe);

    let router_recipe = node::ipv4::router((server_recipe, client_under_nat_recipe));
    let (spawn_complete, _ipv4_plug) =
        spawn::ipv4_tree(&network.handle(), Ipv4Range::global(), router_recipe);

    let _ = evloop.run(spawn_complete).unwrap();
}
```

Or try [runnable example](../examples/latency.rs) from netsim:

```shell
cargo run --example latency
```
