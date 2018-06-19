# Network performance

`netsim` runs entirely in memory - no packets leave our machines, hence it's
super fast compared to real computer networks. However sometimes we want to
test how our code reacts in a more realistic environments. For this, netsim
allows us to simulate [packet loss](07_packet_loss.md) and
[latency](06_latency.md).
