# Packet loss

In the [last tutorial](06_latency.md) we saw how to introduce some latency to our
network. This time we will go even further and make our network even less
reliable by dropping some packets.

## Sample network

This time we will have a very simple network consisting of two nodes connected
via a router:

```
                      +--------+
           +--------->| router |-----------------+
           |          +--------+                 |
           |                                     v
        +--------+                          +--------+
        | client |                          | server |
        +--------+                          +--------+
```

Then we will send multiple UDP datagrams from the client to the server. We will
introduce some packet loss on the client device and monitor how that affects
what the server receives.

## Dependencies

We need to install some dependency libraries. Put this into your Cargo.toml:

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

## Server

Let's create a server that infinitely receives packets:

```rust
use futures::sync::oneshot;
use netsim::{node, Network};
use tokio_core::reactor::Core;

use std::net::{SocketAddr, SocketAddrV4, UdpSocket};

fn main() {
    let mut evloop = Core::new().unwrap();
    let network = Network::new(&evloop.handle());

    let (server_addr_tx, server_addr_rx) = oneshot::channel();
    let server_recipe = node::ipv4::machine(move |ip| {
        println!("[server] ip = {}", ip);

        let bind_addr = SocketAddr::V4(SocketAddrV4::new(ip, 0));
        let sock = UdpSocket::bind(bind_addr).unwrap();
        let _ = server_addr_tx.send(sock.local_addr().unwrap());

        let mut buf = [0; 4096];
        loop {
            let _  = sock.recv_from(&mut buf).unwrap();
            println!("[server] received: packet nr. {}", buf[0]);
        }
    });
}
```

## Client

Now we will create a client node that will send 10 UDP datagrams to the server:

```rust
let client_recipe = node::ipv4::machine(|ip| {
    println!("[client] ip = {}", ip);

    let server_addr = server_addr_rx.wait().unwrap();
    println!("[client] Got server addr: {}", server_addr);

    let sock = UdpSocket::bind("0.0.0.0:0").unwrap();
    for i in 1..11 {
        let _ = sock.send_to(&[i], server_addr).unwrap();
    }
});
```

## Connect nodes

We already have recipes to build client and server nodes. Now we just need to
connect them together and start the network:

```rust

let router_recipe = node::ipv4::router((server_recipe, client_recipe));
let (spawn_complete, _ipv4_plug) =
    spawn::ipv4_tree(&network.handle(), Ipv4Range::global(), router_recipe);

evloop.run(spawn_complete).unwrap();
```

Then try running the network. You should see all 10 packets delivered:

```shell
[server] ip = 85.69.200.28
[client] ip = 60.227.49.200
[client] Got server addr: 85.69.200.28:46837
[server] received: packet nr. 1
[server] received: packet nr. 2
[server] received: packet nr. 3
[server] received: packet nr. 4
[server] received: packet nr. 5
[server] received: packet nr. 6
[server] received: packet nr. 7
[server] received: packet nr. 8
[server] received: packet nr. 9
[server] received: packet nr. 10
```

## Introduce packet loss

This time we will do a small change and introduce some packet loss on the
client:

```rust
use netsim::node::Ipv4Node;
use std::time::Duration;

let client_recipe = node::ipv4::machine(|ip| {
    println!("[client] ip = {}", ip);

    let server_addr = server_addr_rx.wait().unwrap();
    println!("[client] Got server addr: {}", server_addr);

    let sock = UdpSocket::bind("0.0.0.0:0").unwrap();
    for i in 1..11 {
        let _ = sock.send_to(&[i], server_addr).unwrap();
    }
}).packet_loss(0.25, Duration::from_millis(500));
```

`.packet_loss()` belongs to the `Ipv4Node` trait, hence it must be in scope.
Packet loss happens in random bursts, to control this behaviour
[`.packet_loss()`](https://docs.rs/netsim/0.2.2/netsim/node/ipv4/trait.Ipv4Node.html#method.packet_loss)
takes two arguments:

1. `loss_rate` - the chance of being in a loss burst at any given time.
2. `mean_loss_duration` - average duration of a loss burst.

So if `mean_loss_duration = 500ms` and `loss_rate = 0.25` then the node will
experience bursts of packet loss with 500ms average duration, and bursts of
connectivity with 1500ms average duration.

Now if we run the example, the output will probably look like this:

```rust
[server] ip = 84.137.233.235
[client] ip = 59.182.14.10
[client] Got server addr: 84.137.233.235:54044
[server] received: packet nr. 1
[server] received: packet nr. 2
[server] received: packet nr. 3
[server] received: packet nr. 4
[server] received: packet nr. 5
[server] received: packet nr. 6
[server] received: packet nr. 7
[server] received: packet nr. 8
[server] received: packet nr. 9
[server] received: packet nr. 10
```

You might be wondering what happened, given that all 10 packets arrived.  In
this case, the client node instantly sent 10 packets which didn't fall into a
packet loss interval. If we run this test several more times we're likely to
have a run where none of the sent packets arrived instead. If we want to see
individual packet loss, we need to pace our packets. This time we will send a
packet every 500ms simulating a more realistic constant packet flow:

```rust
use std::thread::sleep;

let client_recipe = node::ipv4::machine(|ip| {
    println!("[client] ip = {}", ip);

    let server_addr = server_addr_rx.wait().unwrap();
    println!("[client] Got server addr: {}", server_addr);

    let sock = UdpSocket::bind("0.0.0.0:0").unwrap();
    for i in 1..11 {
        let _ = sock.send_to(&[i], server_addr).unwrap();
        sleep(Duration::from_millis(500));
    }
}).packet_loss(0.25, Duration::from_millis(500));
```

Now rerun the example and you should witness some packet loss:

```shell
$ cargo run
[server] ip = 76.147.173.133
[client] ip = 48.213.55.87
[client] Got server addr: 76.147.173.133:47250
[server] received: packet nr. 1
[server] received: packet nr. 2
[server] received: packet nr. 4
[server] received: packet nr. 5
[server] received: packet nr. 7
[server] received: packet nr. 9
[server] received: packet nr. 10
```

## Complete example

See [complete example](../examples/packet_loss.rs) from netsim:

```shell
$ cargo run --example packet_loss
```
