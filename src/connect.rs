use crate::priv_prelude::*;

pub trait Connect<T> {
    fn connect_to(self, other: T);
}

impl<T0, T1> Connect<T1> for T0
where
    T0: IpSinkStream,
    T1: IpSinkStream,
{
    fn connect_to(self, other: T1) {
        let (sink_0, stream_0) = self.split();
        let (sink_1, stream_1) = other.split();
        let _detach = tokio::spawn(stream_0.forward(sink_1).map(|_res| ()));
        let _detach = tokio::spawn(stream_1.forward(sink_0).map(|_res| ()));
    }
}

pub fn connect<T, U>(plug_0: T, plug_1: U)
where
    T: Connect<U>,
{
    plug_0.connect_to(plug_1);
}

