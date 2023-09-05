use crate::priv_prelude::*;

/// Convenience method for connecting two [`IpSinkStream`](crate::IpSinkStream)s to each other.
///
/// Packets from either `IpSinkStream` are forwarded to the other. See [`IpHub`](crate::device::IpHub) if
/// you want to connect more than two interfaces.
pub fn connect<T0, T1>(plug_0: T0, plug_1: T1)
where
    T0: IpSinkStream,
    T1: IpSinkStream,
{
    let (sink_0, stream_0) = plug_0.split();
    let (sink_1, stream_1) = plug_1.split();
    let _detach = tokio::spawn(stream_0.forward(sink_1).map(|_res| ()));
    let _detach = tokio::spawn(stream_1.forward(sink_0).map(|_res| ()));
}

