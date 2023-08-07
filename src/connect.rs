use crate::priv_prelude::*;

pub trait Connect<T> {
    fn connect_to(self, other: T);
}

/*
impl Connect<IpPacketSink> for IpPacketStream {
    fn connect_to(self, other: IpPacketSink) {
        let _ = tokio::spawn(self.forward(other).map(|_res| ()));
    }
}

impl Connect<IpPacketStream> for IpPacketSink {
    fn connect_to(self, other: IpPacketStream) {
        let _ = tokio::spawn(other.forward(self).map(|_res| ()));
    }
}

impl Connect<(IpPacketSink, IpPacketStream)> for (IpPacketSink, IpPacketStream) {
    fn connect_to(self, other: (IpPacketSink, IpPacketStream)) {
        let (stream_0, sink_0) = self;
        let (stream_1, sink_1) = other;
        stream_0.connect_to(sink_1);
        stream_1.connect_to(sink_0);
    }
}
*/

impl<T0, T1> Connect<T1> for T0
where
    T0: Stream<Item = io::Result<Vec<u8>>>,
    T0: Sink<Vec<u8>, Error = io::Error>,
    T0: Send + 'static,
    T1: Stream<Item = io::Result<Vec<u8>>>,
    T1: Sink<Vec<u8>, Error = io::Error>,
    T1: Send + 'static,
{
    fn connect_to(self, other: T1) {
        let (sink_0, stream_0) = self.split();
        let (sink_1, stream_1) = other.split();
        let _ = tokio::spawn(stream_0.forward(sink_1).map(|_res| ()));
        let _ = tokio::spawn(stream_1.forward(sink_0).map(|_res| ()));
    }
}

pub fn connect<T, U>(plug_0: T, plug_1: U)
where
    T: Connect<U>,
{
    plug_0.connect_to(plug_1);
}

