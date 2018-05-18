use priv_prelude::*;

/// Construct a hierarchical network of nodes. The returned plug can be used to write packets to
/// the network and read packets that try to leave the network.
pub fn network_ipv6<N: Ipv6Node>(
    handle: &NetworkHandle,
    ipv6_range: Ipv6Range,
    node: N,
) -> (SpawnComplete<N::Output>, Ipv6Plug) {
    node.build(handle, ipv6_range)
}

