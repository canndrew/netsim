# Preface

Netsim stands for network simulator. It's a Rust framework to test your Rust
networking code. Netsim allows us to simulate misc networks with different
hierarchies, introduce packet loss, latency, create different types of NAT and
test the robustness of our NAT traversal mechanisms and all within Rust.
Under the hood netsim uses Linux network namespaces and virtual devices
to create and simulate virtual IPv4/IPv6 networks and [Tokio](https://tokio.rs/)
to schedule the work. More on this at [architecture overview](09_architecture.md).

To start with see [hello world](02_hello_world.md) example.

## Why?

At [MaidSafe](https://maidsafe.net/) we're working on autonomous privacy
oriented peer-to-peer data network - [SAFE Network](https://safenetwork.org/).
Every piece of code must be bullet proof so as networking. P2p networks are
even harder to test. That's why we created netsim. It allows us to simulate
p2p networks with misc NATs and test our code on them.

There are already a handful open source network simulators like
[Mininet](http://mininet.org/). Usually such software uses virtual machines or
Linux containers (Docker, LXC, etc.) to run virtual nodes. Constructing
virtual networks and runing deterministic tests on them becomes cumbersome.
Netsim is different in a way that you are able to simulate networks directly
from Rust code and it's lightweight - networks are created in a fracture of
a second. So you can run great number of tests in parallel.
