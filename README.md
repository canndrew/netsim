# netsim

`netsim` is a Rust library which allows you to:

* Run tests in network-isolated threads.
* Test networking code on simulated networks.
* Capture and inspect packets produced by your code.
* Inject and meddle with network packets.

[Documentation](https://docs.rs/netsim/)

## Examples

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
```

### More, longer examples.

Check out the [examples](https://github.com/canndrew/netsim/tree/master/examples) directory in
this repo.

## Limitations

`netsim` currently only supports Linux since it makes use of the Linux containerization APIs.

## License

MIT or BSD-3-Clause at your option.

