use priv_prelude::*;

/// Construct a hierarchical network of nodes. The returned plug can be used to write frames to
/// the network and read frames that try to leave the network.
pub fn network_eth<N: EtherNode>(
    handle: &Handle,
    ipv4_range: Option<Ipv4Range>,
    ipv6_range: Option<Ipv6Range>,
    node: N,
) -> (SpawnComplete<N::Output>, EtherPlug) {
    node.build(handle, ipv4_range, ipv6_range)
}

