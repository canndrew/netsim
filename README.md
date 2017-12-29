# netsim - A Rust library for network simulation and testing (currently linux-only).

This crate provides tools for simulating IP networks for use in automated
testing. Here's a brief rundown of the main features:

## Spawning threads into isolated network namespaces

Network namespaces are a linux feature which can provide a thread or process with its own
view of the system's network interfaces and routing table. This crate's `spawn` module
provides functions for spawning threads into their own network namespaces. The most primitive
of these functions is `new_namespace`, which is demonstrated below. In this example we list the
visible network interfaces using the [`get_if_addrs`](https://crates.io/crates/get_if_addrs)
library.

```rust
extern crate netsim;
extern crate get_if_addrs;
use netsim::spawn;

// First, check that there is more than one network interface. This will generally be true
// since there will at least be the loopback interface.
let interfaces = get_if_addrs::get_if_addrs().unwrap();
assert!(interfaces.len() > 0);

// Now check how many network interfaces we can see inside a fresh network namespace. There
// should be zero.
let join_handle = spawn::new_namespace(|| {
    get_if_addrs::get_if_addrs().unwrap()
});
let interfaces = join_handle.join().unwrap();
assert!(interfaces.is_empty());
```

This demonstrates how to launch a thread - perhaps running an automated test -
into a clean environment. However an environment with no network interfaces is
pretty useless...

## Creating virtual interfaces

We can create virtual (TAP) interfaces using the `Tap` type. A `Tap` is a
[`futures`](https://crates.io/crates/futures) `Stream + Sink` which can be used
to read/write raw ethernet frames to the interface. Here's an example using
[`tokio`](https://crates.io/crates/tokio-core).

```rust
extern crate netsim;
#[macro_use]
extern crate net_literals;
extern crate tokio_core;
extern crate futures;
use netsim::tap::{TapBuilder, Tap, IfaceAddrV4};

let core = Core::new().unwrap();
let handle = core.handle();

// Create a network interface named "netsim"
let tap = {
    TapBuilderV4::new()
    .name("netsim")
    .address(IfaceAddrV4 {
        netmask: ipv4!("255.255.255.0"),
        address: ipv4!("192.168.0.23"),
    })
    .route(RouteV4::default())
    .build(&handle)
};

// Read the first `EtherFrame` sent out the interface.
let frame = core.run({
    tap
    .into_future()
    .and_then(|(frame, _)| frame.unwrap())
}).unwrap();
```

## More, higher-level examples

TODO

