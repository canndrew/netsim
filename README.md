# netsim - A Rust library for network simulation and testing (currently linux-only).

`netsim` is a crate for simulating networks for the sake of testing network-oriented Rust
code. You can use it to run Rust functions in network-isolated containers, and assemble
virtual networks for these functions to communicate over.

# Dependencies

POSIX capabilities library:
```
$ apt install libcap-dev
$ dnf install libcap-devel # Fedora
```

# Spawning threads into isolated network namespaces

Network namespaces are a linux feature which can provide a thread or process with its own view
of the system's network interfaces and routing table. This crate's `spawn` module provides the
`new_namespace` function for spawning threads into their own network namespaces.  In this
demonstration we list the visible network interfaces using the
[`get_if_addrs`](https://crates.io/crates/get_if_addrs) crate.

```rust
extern crate netsim;
extern crate get_if_addrs;
extern crate tokio_core;
use netsim::spawn;
use tokio_core::reactor::Core;
use get_if_addrs::get_if_addrs;

// First, check that there is more than one network interface. This will generally be true
// since there will at least be the loopback interface.
let interfaces = get_if_addrs().unwrap();
assert!(interfaces.len() > 0);

// Now check how many network interfaces we can see inside a fresh network namespace. There
// should be zero.
let spawn_complete = spawn::new_namespace(|| {
    get_if_addrs().unwrap()
});
let mut core = Core::new().unwrap();
let interfaces = core.run(spawn_complete).unwrap();
assert!(interfaces.is_empty());
```

This demonstrates how to launch a thread - perhaps running an automated test - into a sandboxed
environment. However an environment with no network interfaces is pretty useless...

# Creating virtual interfaces

We can create virtual IP and Ethernet interfaces using the types in the `iface` module. For
example, `IpIface` lets you create a new IP (TUN) interface and implements `futures::{Stream,
Sink}` so that you can read/write raw packets to it.

```rust,should_panic
extern crate netsim;
extern crate tokio_core;
extern crate futures;

use std::net::Ipv4Addr;
use tokio_core::reactor::Core;
use futures::{Future, Stream};
use netsim::iface::IpIfaceBuilder;
use netsim::spawn;

let mut core = Core::new().unwrap();
let handle = core.handle();

// Create a network interface named "netsim"
// Note: This will likely fail with "permission denied" unless we run it in a fresh network
// environment
let iface = {
    IpIfaceBuilder::new()
    .name("netsim")
    .ipv4_addr(Ipv4Addr::new(192, 168, 0, 24), 24)
    .build(&handle)
    .unwrap()
};

// Read the first `Ipv4Packet` sent from the interface.
let packet = core.run({
    iface
    .into_future()
    .map_err(|(e, _)| e)
    .map(|(packet_opt, _)| packet_opt.unwrap())
}).unwrap();
```

However, for simply testing network code, you don't need to create interfaces manually like
this.

# Sandboxing network code

Rather than performing the above two steps individually, you can use the `spawn::ipv4_tree`
function along with the `node` module to set up a namespace with an IPv4 interface for you.

```rust
extern crate netsim;
extern crate tokio_core;
extern crate futures;

use std::net::UdpSocket;
use tokio_core::reactor::Core;
use futures::{Future, Stream};
use netsim::{spawn, node, Network, Ipv4Range};
use netsim::wire::Ipv4Payload;

// Create an event loop and a network to bind devices to.
let mut core = Core::new().unwrap();
let network = Network::new(&core.handle());
let handle = network.handle();

// Spawn a network with a single node - a machine with an IPv4 interface in the 10.0.0.0/8
// range, running the given callback.
let (spawn_complete, ipv4_plug) = spawn::ipv4_tree(
    &handle,
    Ipv4Range::local_subnet_10(),
    node::ipv4::machine(|ipv4_addr| {
        // Send a packet out the interface
        let socket = UdpSocket::bind("0.0.0.0:0").unwrap();
        socket.send_to(b"hello world", "10.1.2.3:4567").unwrap();
    }),
);

let (packet_tx, packet_rx) = ipv4_plug.split();

// Inspect the packet sent out the interface.
core.run({
    packet_rx
    .into_future()
    .map(|(packet_opt, _)| {
        let packet = packet_opt.unwrap();
        match packet.payload() {
            Ipv4Payload::Udp(udp) => {
                assert_eq!(&udp.payload()[..], &b"hello world"[..]);
            },
            _ => panic!("unexpected payload"),
        }
    })
}).unwrap()
```

# Simulating networks of communicating nodes

Using the `spawn` and `node` modules you can set up a bunch of nodes connected over a virtual
network.

```rust
extern crate tokio_core;
extern crate future_utils;
extern crate netsim;

use std::net::UdpSocket;
use tokio_core::reactor::Core;
use netsim::{spawn, node, Network, Ipv4Range};

// Create an event loop and a network to bind devices to.
let mut core = Core::new().unwrap();
let network = Network::new(&core.handle());
let handle = network.handle();

let (tx, rx) = std::sync::mpsc::channel();

// Create a machine which will receive a UDP packet and return its contents
let receiver_node = node::ipv4::machine(move |ipv4_addr| {
    let socket = UdpSocket::bind(("0.0.0.0", 1234)).unwrap();
    /// Tell the sending node our IP address
    tx.send(ipv4_addr).unwrap();
    let mut buffer = [0; 1024];
    let (n, _sender_addr) = socket.recv_from(&mut buffer).unwrap();
    buffer[..n].to_owned()
});

// Create the machine which will send the UDP packet
let sender_node = node::ipv4::machine(move |_ipv4_addr| {
    let receiver_ip = rx.recv().unwrap();
    let socket = UdpSocket::bind("0.0.0.0:0").unwrap();
    socket.send_to(b"hello world", (receiver_ip, 1234)).unwrap();
});

// Connect the sending and receiving nodes via a router
let router_node = node::ipv4::router((receiver_node, sender_node));

// Run the network with the router as the top-most node. `_plug` could be used send/receive
// packets from/to outside the network
let (spawn_complete, _plug) = spawn::ipv4_tree(&handle, Ipv4Range::global(), router_node);

// Drive the network on the event loop and get the data returned by the receiving node.
let (received, ()) = core.run(spawn_complete).unwrap();
assert_eq!(&received[..], b"hello world");
```

# All the rest

It's possible to set up more complicated (non-hierarchical) network topologies, ethernet
networks, namespaces with multiple interfaces etc. by directly using the primitives in the
`device` module. Have an explore of the API, and if anything needs clarification or could be
better designed then let us know on the bug tracker :)

## Testing

netsim has it's own unit/integration tests. Dependending on the environment
you run netsim tests, there are different ways to do so.

If you're on Linux host machine, use:

```
$ cargo test
```

If you're inside Linux container, say running tests on travis CI, you can't
use some Linux features like create namespaces, hence some tests won't run.
In such cases disable `linux_host` Rust feature:

```
$ cargo test --no-default-features
```

## License

This library is dual-licensed under the Modified BSD ([LICENSE-BSD](LICENSE-BSD) https://opensource.org/licenses/BSD-3-Clause) or the MIT license ([LICENSE-MIT](LICENSE-MIT) https://opensource.org/licenses/MIT) at your option.
