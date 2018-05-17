use priv_prelude::*;

/// Construct a hierarchical network of nodes. The returned plug can be used to write packets to
/// the network and read packets that try to leave the network.
pub fn network_ip<N: IpNode>(
    handle: &Handle,
    ipv4_range: Option<Ipv4Range>,
    ipv6_range: Option<Ipv6Range>,
    node: N,
) -> (SpawnComplete<N::Output>, IpPlug) {
    node.build(handle, ipv4_range, ipv6_range)
}

