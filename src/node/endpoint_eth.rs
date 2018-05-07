use priv_prelude::*;
use spawn;

/// A node representing an ethernet endpoint
pub struct EndpointEthNode<F> {
    func: F,
}

/// Create a node for an Ipv4 endpoint. This node will run the given function in a network
/// namespace with a single interface.
pub fn endpoint_eth<R, F>(func: F) -> EndpointEthNode<F>
where
    R: Send + 'static,
    F: FnOnce() -> R + Send + 'static,
{
    EndpointEthNode { func }
}

impl<R, F> EtherNode for EndpointEthNode<F>
where
    R: Send + 'static,
    F: FnOnce() -> R + Send + 'static,
{
    type Output = R;

    fn build(
        self,
        handle: &Handle,
        subnet_v4: Option<SubnetV4>,
    ) -> (SpawnComplete<R>, EtherPlug) {
        let mut iface = {
            EtherIfaceBuilder::new()
            .route(RouteV4::new(SubnetV4::global(), None))
        };
        if let Some(subnet) = subnet_v4 {
            let address = subnet.random_client_addr();
            iface = {
                iface
                .address(address)
                .netmask(subnet.netmask())
            };
        }
        spawn::with_ether_iface(
            handle,
            iface,
            self.func,
        )
    }
}


