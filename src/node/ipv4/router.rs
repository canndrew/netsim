use priv_prelude::*;
use spawn_complete;

/// A set of clients that can be attached to a router node.
pub trait Ipv4RouterClients {
    /// The output of the nodes attached to the router.
    type Output: Send + 'static;

    /// Build the set of nodes.
    fn build(self, handle: &NetworkHandle, ipv4_range: Ipv4Range) -> (SpawnComplete<Self::Output>, Ipv4Plug);
}

struct JoinAll<X, T> {
    phantoms: PhantomData<X>,
    children: T,
}

macro_rules! tuple_impl {
    ($($ty:ident,)*) => {
        impl<$($ty),*> Ipv4RouterClients for ($($ty,)*)
        where
            $($ty: Ipv4Node + 'static,)*
        {
            type Output = ($($ty::Output,)*);
            
            fn build(self, handle: &NetworkHandle, ipv4_range: Ipv4Range) -> (SpawnComplete<Self::Output>, Ipv4Plug) {
                #![allow(non_snake_case)]
                #![allow(unused_assignments)]
                #![allow(unused_mut)]
                #![allow(unused_variables)]

                let ($($ty,)*) = self;

                let mut i = 0;
                $(
                    let $ty = $ty;
                    i += 1;
                )*
                let ranges = ipv4_range.split(i + 1);

                let router = Ipv4RouterBuilder::new(ranges[0].base_addr());
                let mut i = 1;
                $(
                    let ($ty, plug) = $ty.build(handle, ranges[i]);
                    let router = router.connect(plug, vec![Ipv4Route::new(ranges[i], None)]);
                    i += 1;
                )*
                
                let (plug_0, plug_1) = Ipv4Plug::new_pair();
                let router = router.connect(plug_1, vec![Ipv4Route::new(Ipv4Range::global(), None)]);
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

impl<N> Ipv4RouterClients for Vec<N>
where
    N: Ipv4Node + 'static,
{
    type Output = Vec<N::Output>;

    fn build(
        self,
        handle: &NetworkHandle,
        ipv4_range: Ipv4Range,
    ) -> (SpawnComplete<Vec<N::Output>>, Ipv4Plug) {
        let ranges = ipv4_range.split(self.len() as u32 + 1);
        let mut router = Ipv4RouterBuilder::new(ranges[0].base_addr());
        let mut spawn_completes = FuturesOrdered::new();

        for (i, node) in self.into_iter().enumerate() {
            let (spawn_complete, plug) = node.build(handle, ranges[i + 1]);
            router = router.connect(plug, vec![Ipv4Route::new(ranges[i + 1], None)]);
            spawn_completes.push(spawn_complete);
        }

        let (plug_0, plug_1) = Ipv4Plug::new_pair();
        let router = router.connect(plug_1, vec![Ipv4Route::new(Ipv4Range::global(), None)]);
        router.spawn(handle);

        let (tx, rx) = oneshot::channel();
        handle.spawn({
            spawn_completes
            .collect()
            .then(|result| {
                let _ = tx.send(result);
                Ok(())
            })
        });

        let spawn_complete = spawn_complete::from_receiver(rx);

        (spawn_complete, plug_0)
    }
}

/// A node representing an Ipv4 router
pub struct RouterNode<C> {
    clients: C,
}

/// Spawns a bunch of sub-nodes and routes packets between them.
pub fn router<C: Ipv4RouterClients>(clients: C) -> RouterNode<C> {
    RouterNode { clients }
}

impl<C> Ipv4Node for RouterNode<C>
where
    C: Ipv4RouterClients,
{
    type Output = C::Output;

    fn build(
        self,
        handle: &NetworkHandle,
        ipv4_range: Ipv4Range,
    ) -> (SpawnComplete<C::Output>, Ipv4Plug) {
        self.clients.build(handle, ipv4_range)
    }
}

