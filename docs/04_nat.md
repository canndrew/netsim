# Network Address Translation

![NAT](imgs/nat.svg)

When we use routers at home, office, etc. they use
[Network Address Translation](https://en.wikipedia.org/wiki/Network_address_translation)
to change your local IP address to public one.

Peer-to-peer systems employ [NAT traversal](https://en.wikipedia.org/wiki/NAT_traversal)
techniques to expose nodes on Internet, even if they run behind a router.
Testing NAT traversal is hard. Therefore netsim allows us to simulate NAT
and test how our code behaves in such network.

In this tutorial we will leverage [routing example](03_routing.md) and put
client node under NAT.

## Dependencies

As in previous example we need to install some dependency libraries. Put this
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

## Simple network

We will reuse 2 node network from routing tutorial:

```rust
use futures::Future;
use futures::sync::oneshot;
use netsim::{node, spawn, Ipv4Range, Network};
use netsim::device::ipv4::Ipv4NatBuilder;
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

    let client_recipe = node::ipv4::machine(|ip| {
        println!("[client] ip = {}", ip);

        let server_addr = server_addr_rx.wait().unwrap();
        println!("[client] Got server addr: {}", server_addr);

        let sock = UdpSocket::bind("0.0.0.0:0").unwrap();
        let _ = sock.send_to(b"hello world!", server_addr).unwrap();
    });
}
```

Up until this point everything is the same: we have server node listening
for incoming UDP datagrams and client node that will send a message
to server.

## NAT

Now we will put client node under virtual NAT device.

```rust
use netsim::device::ipv4::Ipv4NatBuilder;

let client_under_nat_recipe = node::ipv4::nat(Ipv4NatBuilder::default(), client_recipe);

let router_recipe = node::ipv4::router((server_recipe, client_under_nat_recipe));
let (spawn_complete, _ipv4_plug) =
    spawn::ipv4_tree(&network.handle(), Ipv4Range::global(), router_recipe);
```

`node::ipv4::nat()` creates a recipe to construct virtual NAT device that
manages another virtual networking device. `Ipv4NatBuilder::default()`
configures NAT with convenient default values: NAT has no port forwarding
rules, it picks random local subnet,
[hairpinning](https://en.wikipedia.org/wiki/Hairpinning) disabled, etc. Later
we will see how to use `Ipv4NatBuilder` to setup different NAT types.

If we run this example now, we should see something like this:

```shell
$ cargo run
[server] ip = 65.199.115.13
[client] ip = 10.72.190.150
[client] Got server addr: 65.199.115.13:45577
[server] received: hello world!, from: 59.41.45.45:1000
```

Notice how server nodee has public IP `65.199.115.13` whereas client node
gets private IP `10.72.190.150` since it's behind NAT. Also notice that
server sees client's public IP.

## Complete example

See [complete example](../examples/nat.rs) from netsim:

```shell
cargo run --example nat
```

## Specify private subnet

Default `Ipv4NatBuilder` chooses a random private subnet. We can specify one
manually though:

```rust
use std::net::Ipv4Addr;

let nat_builder = Ipv4NatBuilder::new()
    .subnet(Ipv4Range::new(Ipv4Addr::new(192, 168, 2, 0), 24));
let client_under_nat_recipe = node::ipv4::nat(nat_builder, client_recipe);
```

Try to rerun the example. Client node should always get IP address in the
`192.168.2.0/24` subnet:

```shell
$ cargo run
[server] ip = 65.188.45.29
[client] ip = 192.168.2.49
[client] Got server addr: 65.188.45.29:46165
[server] received: hello world!, from: 36.96.119.102:1000
```

## Different NAT types

netsim is able to simulate different [NAT
types](https://docs.rs/p2p/0.5.1/p2p/#general). By default full cone NAT is
simulated.

### Full cone NAT

```
       72.92.30.39
        +--------+       +---------+
        | server |       | client2 | 187.12.97.136
        +--------+       +---------+
               ^              | send packets to 53.198.141.83:1000
               |   +----------+
               |   |
               |   V
              +-------+               +==============+==========+==========+
53.198.141.83 |  NAT  |-------------- | Int IP       | Int Port | Ext Port |
              +-------+  NAT table    +==============+==========+==========+
                ^  |                  | 192.168.1.46 |   12345  |   1000   |
                |  |                  +--------------+----------+----------+
                |  |
                |  | client2 packets are
                |  | passed through
                |  V
            +---------+
            | client1 | 192.168.1.46
            +---------+
```

A full cone NAT maps a public `IP:port` to a LAN `IP:port` pair. Any external
host can send data to the LAN IP through the mapped NAT IP and port.  For
example when `client1` connects to server using local source address
`192.168.1.46:12345`, NAT assings new port mappings, stores in them in the table
and changes packet's source address to `53.198.141.83:1000` - that's the address
that `server` sees `client1`. Now if `client2` somehow gets to know those
mappings it can use `client1` public IP and external port to send packets to
`client1` behind NAT. So when `client2` sends a packet to `53.198.141.83:1000`,
full conet NAT will forward this packet to `192.168.1.46:12345`.

We can simulate this behavior with netsim. First of all, we will update
`server` node to send `client1`'s address over in-memory channel:

```rust
let (server_addr_tx, server_addr_rx) = oneshot::channel();
let (client1_addr_tx, client1_addr_rx) = oneshot::channel();
let server_recipe = node::ipv4::machine(|ip| {
    println!("[server] ip = {}", ip);

    let bind_addr = SocketAddr::V4(SocketAddrV4::new(ip, 0));
    let sock = UdpSocket::bind(bind_addr).unwrap();
    let _ = server_addr_tx.send(sock.local_addr().unwrap());

    let mut buf = [0; 4096];
    let (_bytes_received, addr) = sock.recv_from(&mut buf).unwrap();
    println!("[server] received: {}, from: {}", String::from_utf8(buf.to_vec()).unwrap(), addr);

    let _ = client1_addr_tx.send(addr);
});
```

Then we will update `client1` to listen for incoming data after it connects
to `server`:

```rust
let client1_recipe = node::ipv4::machine(|ip| {
    println!("[client] ip = {}", ip);

    let server_addr = server_addr_rx.wait().unwrap();
    println!("[client1] Got server addr: {}", server_addr);

    let sock = UdpSocket::bind("0.0.0.0:0").unwrap();
    let _ = sock.send_to(b"hello world!", server_addr).unwrap();

    let mut buf = [0; 4096];
    let (_bytes_received, addr) = sock.recv_from(&mut buf).unwrap();
    println!("[client1] received: {}, from: {}", String::from_utf8(buf.to_vec()).unwrap(), addr);
});
let nat_builder = Ipv4NatBuilder::new()
    .subnet(Ipv4Range::new(Ipv4Addr::new(192, 168, 1, 0), 24));
let client1_under_nat_recipe = node::ipv4::nat(nat_builder, client1_recipe);
```

Then we create a second client which will try to send data to `client1` with
the address obtained from the server:

```rust
let client2_recipe = node::ipv4::machine(|ip| {
    println!("[client2] ip = {}", ip);

    let client1_addr = client1_addr_rx.wait().unwrap();
    println!("[client2] Got client1 addr: {}", client1_addr);

    let sock = UdpSocket::bind("0.0.0.0:0").unwrap();
    let _ = sock.send_to(b"this is client2!", client1_addr).unwrap();
});
```

Finally we build and run virtual network:

```rust
let router_recipe = node::ipv4::router((server_recipe, client1_under_nat_recipe, client2_recipe));
let (spawn_complete, _ipv4_plug) =
    spawn::ipv4_tree(&network.handle(), Ipv4Range::global(), router_recipe);

evloop.run(spawn_complete).unwrap();
```

After we run this example we should see something like this:

```shell
[server] ip = 72.92.30.39
[client1] ip = 192.168.1.46
[client1] Got server addr: 72.92.30.39:35868
[client2] ip = 187.12.97.136
[server] received: hello world!, from: 53.198.141.83:1000
[client2] Got client1 addr: 53.198.141.83:1000
[client1] received: this is client2!, from: 187.12.97.136:44940
```

You can also try [complete example](../examples/full_cone_nat.rs) from netsim:

```shell
cargo run --example full_cone_nat
```

### Port restricted NAT

```
       72.92.30.39
        +--------+       +---------+
        | server |       | client2 | 187.12.97.136
        +--------+       +---------+
               ^              | send packets to 53.198.141.83:1000
               |   +----------+
               |   |
               |   V
              +-------+               +==============+==========+==========+=============+===========+
53.198.141.83 |  NAT  |-------------- | Int IP       | Int Port | Ext Port | Dest IP     | Dest Port |
              +-------+  NAT table    +==============+==========+==========+=============+===========+
                ^  |                  | 192.168.1.46 |   12345  |   1000   | 72.92.30.39 | 35868     |
                |  |                  +--------------+----------+----------+-------------+-----------+
                |  |
                |  X
                |
                |
            +---------+
            | client1 | 192.168.1.46
            +---------+
```

Port restricted NAT in addition stores destination `IP:port` pair.
So when `client1` connects to `server`, NAT will save destination address
and if `client2` tries to send packets to `client1` with address
`53:198:141:83:1000`, this packet will be dropped by port restricted NAT.

To simulate this NAT we use the same example is in "Full cone NAT" except
we choose port restricted NAT with `Ipv4NatBuilder::restrict_endpoints()`:

```rust
let nat_builder = Ipv4NatBuilder::new()
    .restrict_endpoints()
    .subnet(Ipv4Range::new(Ipv4Addr::new(192, 168, 1, 0), 24));
let client1_under_nat_recipe = node::ipv4::nat(nat_builder, client1_recipe);
```

If we run modified NAT example, we should see something like this:

```shell
[server] ip = 72.92.30.39
[client1] ip = 192.168.1.46
[client1] Got server addr: 72.92.30.39:35868
[client2] ip = 187.12.97.136
[server] received: hello world!, from: 53.198.141.83:1000
[client2] Got client1 addr: 53.198.141.83:1000
```

Notice how `client1` doesn't receive anything. That's because NAT dropped
incoming packet from `client2`.

Also, if we turn on logging, along the lines we can see something like:

```shell
 INFO 2018-06-14T13:13:54Z: netsim::iface::tun: TUN emitting frame: Ipv4Packet { source_ip: 187.12.97.136, dest_ip: 53.198.141.83, ttl: 64, payload: UdpPacket { source_port: 46129, dest_port: 1000, payload: b"this is client2!" } }
 INFO 2018-06-14T13:13:54Z: netsim::device::ipv4::router: router 128.0.0.0 routing packet on route Ipv4Route { destination: 32.0.0.0/3, gateway: None } Ipv4Packet { source_ip: 187.12.97.136, dest_ip: 53.198.141.83, ttl: 64, payload: UdpPacket { source_port: 46129, dest_port: 1000, payload: b"this is client2!" } }
TRACE 2018-06-14T13:13:54Z: netsim::device::ipv4::nat: NAT dropping packet from restricted address 187.12.97.136:46129. allowed endpoints: {1000: {72.92.30.39:35868}}
```

More on logging see at [logging tutorial](08_logging.md).

## Other options

netsim supports various other NAT types and options like hairpinning, manual
port forwarding, endpoint blacklisting, etc. For more detailed view consult
[Ipv4NatBuilder](https://docs.rs/netsim/0.2.2/netsim/device/ipv4/struct.Ipv4NatBuilder.html#method.forward_udp_port).
docs.
