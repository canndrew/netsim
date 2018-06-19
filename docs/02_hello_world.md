# Hello world

Let's take baby steps before we run by doing the hello world of netsim.
We will simulate a networked device and send a "hello world" message to it.

First of all, let's setup a new project and install netsim.

```shell
$ cargo new --bin hello-world
$ cd hello-world
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
yet. So we'll be using the deprecated `tokio-core` crate for now.

Bring the crates into scope in your `main.rs`:

```rust
extern crate netsim;
extern crate tokio_core;
extern crate bytes;
extern crate futures;
```

Check that everything compiles:
```shell
$ cargo run
```

## Server device

Let's create a virtual network with a single networked device on it.

First, some boilerplate:

```rust
use netsim::Network;
use tokio_core::reactor::Core;

fn main() {
    let mut evloop = Core::new().unwrap();
    let network = Network::new(&evloop.handle());
}
```

The `Network` object manages Tokio futures that are running during simulation. When
`network` is dropped, those futures will be dropped too, destroying the network. We use
`network.handle()` to spawn tasks onto the simulated network.

Then let's create an IPv4 networked device:

```rust
use netsim::node;

let server_recipe = node::ipv4::machine(|ip| {
    println!("[server] ip = {}", ip);
});
```

`node::ipv4::machine()` takes single argument which is a function that will
be executed on the simulated machine. This function takes single argument -
the IP address assigned to the machine's network interface. Also note that this function
will run in a separate thread. So anything you do here won't block
the main thread or other simulated devices.
Also, `node::ipv4::machine()` does not create the networked device right away.
It creates a recipe that later will be used to build the device.

Now we'll build and run the simulated device:

```rust
use netsim::{spawn, Ipv4Range};

let (spawn_complete, ipv4_plug) =
    spawn::ipv4_tree(&network.handle(), Ipv4Range::global(), server_recipe);
```

`spawn::ipv4_tree()` takes hierarchical network recipe (in this example it's a
single device), builds it, and runs it on the Tokio event loop.
The returned `spawn_complete` is a future that completes when the callback running on the simulated machine
finishes. `ipv4_plug` is a network plug that we can use to
exchange data with our simulated device. We'll use this next.

## Data exchange

Now let's modify our server device to make it listen for incoming UDP datagrams.

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

When this device starts it binds a UDP socket to the given IP address and waits
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
should be pretty straightforward: we receive the server's address, construct
the IP packet and send it over through the plug.

Finally we will block until server device thread is done:

```rust
evloop.run(spawn_complete).unwrap();
```

If everything went well you should see `[server] received: hello world!`
printed to your screen.

## Complete example

See [complete example](../examples/hello_world.rs) from netsim:

```shell
$ cargo run --example hello_world
```

## Next

Usually we won't be constructing and sending packets to our devices manually.
Instead we want to create multiple network devices and run our Rust code on
them. To communicate between two devices we need some sort of packet routing.
The next example demonstrates how to do that -
[route packets between devices](03_routing.md).
