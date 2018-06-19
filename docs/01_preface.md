# Preface

Netsim is a network simulator and a Rust framework to test your Rust networking
code. Netsim allows you to simulate miscellaneous networks with different
topologies, introduce packet loss, latency, simulate different types of NAT and
test the robustness of your NAT traversal mechanisms, all within Rust. Under
the hood netsim uses Linux network namespaces and virtual TUN/TAP interfaces to
create and simulate virtual IPv4/IPv6 networks and [`tokio`](https://tokio.rs/)
to schedule the work. See the [architecture
overview](https://github.com/canndrew/netsim/blob/master/docs/09_architecture.md)
for more details.

For a quick start, have a look at the [hello
world](https://github.com/canndrew/netsim/blob/master/docs/02_hello_world.md)
example.

## Why?

At [MaidSafe](https://maidsafe.net/) we're working on autonomous privacy
oriented peer-to-peer data-sharing network - the [SAFE
Network](https://safenetwork.org/). Every piece of code must be bullet proof.
However p2p networks are hard to test which is why we created netsim.  Netsim
allows us to simulate networks under various conditions with various kinds of
network-address-translation and test our code on them.

There are already a handful open source network simulators (such as
[Mininet](http://mininet.org/)).  Usually such software uses virtual machines
or Linux containers (Docker, LXC, etc.) to run virtual nodes. Constructing
virtual networks and running deterministic tests on them is cumbersome and
difficult to automate within Rust's testing framework. Netsim is different by
allowing you to simulate networks directly from Rust code. And it's lightweight
- networks are created in a fracture of a second, so you can run a great number
of tests in parallel.
