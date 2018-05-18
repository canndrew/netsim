use priv_prelude::*;

/// A node representing an Ipv6 machine.
pub struct MachineNode<F> {
    func: F,
}

/// Create a node for an Ipv6 machine. This node will run the given function in a network
/// namespace with a single interface.
pub fn machine<R, F>(func: F) -> MachineNode<F>
where
    R: Send + 'static,
    F: FnOnce(Ipv6Addr) -> R + Send + 'static,
{
    MachineNode { func }
}

impl<R, F> Ipv6Node for MachineNode<F>
where
    R: Send + 'static,
    F: FnOnce(Ipv6Addr) -> R + Send + 'static,
{
    type Output = R;

    fn build(
        self,
        handle: &NetworkHandle,
        ipv6_range: Ipv6Range,
    ) -> (SpawnComplete<R>, Ipv6Plug) {
        let address = ipv6_range.random_client_addr();
        let iface = {
            IpIfaceBuilder::new()
            .ipv6_addr(address, ipv6_range.netmask_prefix_length())
            .ipv6_route(Ipv6Route::new(Ipv6Range::global(), ipv6!("::")))
        };
        let (plug_a, plug_b) = IpPlug::new_pair();

        let spawn_complete = {
            MachineBuilder::new()
            .add_ip_iface(iface, plug_b)
            .spawn(handle, move || (self.func)(address))
        };

        let plug_a = plug_a.into_ipv6_plug(handle);

        (spawn_complete, plug_a)
    }
}

