use priv_prelude::*;

/// Construct a hierarchical network of nodes. The returned plug can be used to write packets to
/// the network and read packets that try to leave the network.
pub fn network_ipv4<N: Ipv4Node>(
    handle: &Handle,
    ipv4_range: Ipv4Range,
    node: N,
) -> (SpawnComplete<N::Output>, Ipv4Plug) {
    node.build(handle, ipv4_range)
}

