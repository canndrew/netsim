# Hello world

Let's take baby steps before we run and do the hello world of netsim.
We will simulate networking device and send "hello world" message to it.

First of all, let's setup new project and install netsim.

```shell
cargo new --bin hello-world
cd hello-world
```

Add netsim and other dependencies to your `Cargo.toml`:

```toml
[dependencies]
netsim = "~0.2.2"
tokio-core = "0.1.12"
futures = "0.1.18"
bytes = "0.4.8"
```

Note that netsim doesn't work with new Tokio after
[Tokio reform](https://github.com/tokio-rs/tokio-rfcs/blob/master/text/0001-tokio-reform.md)
yet. So we'll be using deprecated `tokio-core` crate for now.

Bring crates into scope in your `main.rs`:

```rust
extern crate netsim;
extern crate tokio_core;
extern crate bytes;
extern crate futures;
```

See if everything compiles:
```shell
cargo run
```

## Server device

We will create a virtual network with a single networking device on it.

First, some boilerplate:

```rust
use netsim::Network;
use tokio_core::reactor::Core;

fn main() {
    let mut evloop = Core::new().unwrap();
    let network = Network::new(&evloop.handle());
}
```

`Network` object manages Tokio futures that are running during simulation. When
`network` is dropped, those futures will be dropped too.  We use
`network.handle()` to spawn any tasks on simulated network.

Then, we'll create IPv4 networking device:

```rust
use netsim::node;

let server_recipe = node::ipv4::machine(|ip| {
    println!("[server] ip = {}", ip);
});
```

`node::ipv4::machine()` takes single argument which is a function that will
be executed once virtual device is ready. This function takes single argument -
IP address assigned to newly created device. Also note that this function
will be run in a separate thread. So anything you do here won't block
the main thread or other virtual devices.
Also, `node::ipv4::machine()` does not create networking device right away.
It creates a recipe that later will be used to build virtual device.

Now we'll build and run simulated device:

```rust
use netsim::{spawn, Ipv4Range};

let (spawn_complete, ipv4_plug) =
    spawn::ipv4_tree(&network.handle(), Ipv4Range::global(), server_recipe);
```

`spawn::ipv4_tree()` takes hierarchical network recipe (in this example it's a
single device), builds and runs it on Tokio event loop.
Returned `spawn_complete` is a future that completes when thread serving
virtual devices finishes. `ipv4_plug` is a network plug that we can use to
exchange data with our virtual device which we'll use next.

## Data exchange

Now we will make our server device to listen for incoming UDP datagrams.

```rust
use futures::sync::oneshot;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket};

let (server_addr_tx, server_addr_rx) = oneshot::channel();
let server_recipe = node::ipv4::machine(|ip| {
    println!("[server] ip = {}", ip);

    let bind_addr = SocketAddr::V4(SocketAddrV4::new(ip, 0));
    let sock = UdpSocket::bind(bind_addr).unwrap();
    let _ = server_addr_tx.send(sock.local_addr().unwrap());

    let mut buf = [0; 4096];
    let _ = sock.recv_from(&mut buf).unwrap();
    println!("[server] received: {}", String::from_utf8(buf.to_vec()).unwrap());
});
let (spawn_complete, ipv4_plug) =
    spawn::ipv4_tree(&network.handle(), Ipv4Range::global(), server_recipe);
```

When server device starts it binds UDP socket to given IP address and waits
for incoming UDP datagrams.
Now we will use `ipv4_plug` to send raw IP packets to this device:

```rust
use netsim::wire::{Ipv4Fields, Ipv4Packet, Ipv4PayloadFields, UdpFields};

let (packet_tx, _packet_rx) = ipv4_plug.split();
let server_addr = match server_addr_rx.wait().unwrap() {
    SocketAddr::V4(addr) => addr,
    _ => panic!("v6 IP was not expected"),
};
let datagram = Ipv4Packet::new_from_fields_recursive(
    Ipv4Fields {
        source_ip: Ipv4Addr::new(78, 1, 2, 3),
        dest_ip: *server_addr.ip(),
        ttl: 10,
    },
    Ipv4PayloadFields::Udp {
        fields: UdpFields {
            source_port: 12345,
            dest_port: server_addr.port(),
        },
        payload: Bytes::from("hello world!"),
    },
);
let _ = packet_tx.unbounded_send(datagram).unwrap();
```

Building UDP datagram by hand is a little cumbersome but other than that it
should be pretty straightforward: we receive servers address, construct
IP packet and send it over the plug.

Finally we will block until server device thread is done:

```rust
evloop.run(spawn_complete).unwrap();
```

## Complete example

See [complete example](../examples/hello_world.rs) from netsim:

```shell
cargo run --example hello_world
```
