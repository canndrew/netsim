use crate::priv_prelude::*;

pub trait Connect<T> {
    fn connect_to(self, other: T);
}

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

pub fn connect<T, U>(plug_0: T, plug_1: U)
where
    T: Connect<U>,
{
    plug_0.connect_to(plug_1);
}

