use priv_prelude::*;
use spawn_complete;

/// A set of clients that can be attached to a router node.
pub trait RouterClientsV4 {
    /// The output of the nodes attached to the router.
    type Output: Send + 'static;

    /// Build the set of nodes.
    fn build(self, handle: &Handle, subnet: SubnetV4) -> (SpawnComplete<Self::Output>, Ipv4Plug);
}

struct JoinAll<X, T> {
    phantoms: PhantomData<X>,
    children: T,
}

macro_rules! tuple_impl {
    ($($ty:ident,)*) => {
        impl<$($ty),*> RouterClientsV4 for ($($ty,)*)
        where
            $($ty: Ipv4Node + 'static,)*
        {
            type Output = ($($ty::Output,)*);
            
            fn build(self, handle: &Handle, subnet: SubnetV4) -> (SpawnComplete<Self::Output>, Ipv4Plug) {
                #![allow(non_snake_case)]
                #![allow(unused_assignments)]
                #![allow(unused_mut)]
                #![allow(unused_variables)]

                let ($($ty,)*) = self;
                let leading_bits = subnet.netmask_bits();
                let mut next_ip = {
                    let mut n = 0;
                    move || {
                        loop {
                            let mut n_reversed = 0;
                            for i in 0..32 {
                                if n & (1 << i) != 0 {
                                    n_reversed |= 0x8000_0000u32 >> i;
                                }
                            }
                            let base_addr = u32::from(subnet.base_addr());
                            let ip = base_addr | (n_reversed >> leading_bits);
                            n += 1;
                            let ip = Ipv4Addr::from(ip);
                            if !subnet.base_addr().is_private() && !Ipv4AddrExt::is_global(&ip) {
                                // reject ips that take us out of the global IP range
                                continue;
                            }
                            return ip;
                        }
                    }
                };

                let last_ip = next_ip();
                let router_ip = last_ip;
                let mut subnet_ips = Vec::<Ipv4Addr>::new();
                $(
                    let $ty = $ty;
                    let last_ip = next_ip();
                    subnet_ips.push(last_ip);
                )*
                let extra_subnet_bits = 32 - u32::from(last_ip).trailing_zeros() as u8;
                let new_subnet_bits = leading_bits + extra_subnet_bits;

                let router = RouterV4Builder::new(router_ip);
                let mut i = 0;
                $(
                    let subnet = SubnetV4::new(subnet_ips[i], new_subnet_bits);
                    i += 1;
                    let ($ty, plug) = $ty.build(handle, subnet);
                    let router = router.connect(plug, vec![RouteV4::new(subnet, None)]);
                )*
                
                let (plug_0, plug_1) = Ipv4Plug::new_wire();
                let router = router.connect(plug_1, vec![RouteV4::new(SubnetV4::global(), None)]);
                router.spawn(handle);

                let (ret_tx, ret_rx) = oneshot::channel();
                handle.spawn({
                    JoinAll { phantoms: PhantomData::<($($ty,)*)>, children: ($(($ty, None),)*) }
                    .then(|result| {
                        let _ = ret_tx.send(result);
                        Ok(())
                    })
                });

                let spawn_complete = spawn_complete::from_receiver(ret_rx);

                (spawn_complete, plug_0)
            }
        }

        impl<$($ty),*> Future for JoinAll<($($ty,)*), ($((SpawnComplete<$ty::Output>, Option<$ty::Output>),)*)>
        where
            $($ty: Ipv4Node + 'static,)*
        {
            type Item = ($($ty::Output,)*);
            type Error = Box<Any + Send + 'static>;

            fn poll(&mut self) -> thread::Result<Async<Self::Item>> {
                #![allow(non_snake_case)]

                let ($(ref mut $ty,)*) = self.children;
                $({
                    let (ref mut spawn_complete, ref mut result) = *$ty;
                    if result.is_none() {
                        match spawn_complete.poll()? {
                            Async::Ready(val) => {
                                *result = Some(val);
                            },
                            Async::NotReady => {
                                return Ok(Async::NotReady);
                            },
                        }
                    }
                })*

                $(
                    let (_, ref mut result) = *$ty;
                    let $ty = unwrap!(result.take());
                )*

                Ok(Async::Ready(($($ty,)*)))
            }
        }
    }
}

tuple_impl!();
tuple_impl!(T0,);
tuple_impl!(T0,T1,);
tuple_impl!(T0,T1,T2,);
tuple_impl!(T0,T1,T2,T3,);
tuple_impl!(T0,T1,T2,T3,T4,);
tuple_impl!(T0,T1,T2,T3,T4,T5,);
tuple_impl!(T0,T1,T2,T3,T4,T5,T6,);
tuple_impl!(T0,T1,T2,T3,T4,T5,T6,T7,);
tuple_impl!(T0,T1,T2,T3,T4,T5,T6,T7,T8,);
tuple_impl!(T0,T1,T2,T3,T4,T5,T6,T7,T8,T9,);
tuple_impl!(T0,T1,T2,T3,T4,T5,T6,T7,T8,T9,T10,);
tuple_impl!(T0,T1,T2,T3,T4,T5,T6,T7,T8,T9,T10,T11,);
tuple_impl!(T0,T1,T2,T3,T4,T5,T6,T7,T8,T9,T10,T11,T12,);
tuple_impl!(T0,T1,T2,T3,T4,T5,T6,T7,T8,T9,T10,T11,T12,T13,);
tuple_impl!(T0,T1,T2,T3,T4,T5,T6,T7,T8,T9,T10,T11,T12,T13,T14,);
tuple_impl!(T0,T1,T2,T3,T4,T5,T6,T7,T8,T9,T10,T11,T12,T13,T14,T15,);

pub struct ImplNode<C> {
    clients: C,
}

/// Spawns a bunch of sub-nodes and routes packets between them.
pub fn router_v4<C: RouterClientsV4>(clients: C) -> ImplNode<C> {
    ImplNode { clients }
}

impl<C> Ipv4Node for ImplNode<C>
where
    C: RouterClientsV4,
{
    type Output = C::Output;

    fn build(
        self,
        handle: &Handle,
        subnet: SubnetV4,
    ) -> (SpawnComplete<C::Output>, Ipv4Plug) {
        self.clients.build(handle, subnet)
    }
}

