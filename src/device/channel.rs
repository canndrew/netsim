use crate::priv_prelude::*;
pub use futures::channel::mpsc::SendError;

/// A bi directional channel for both sending and receiving items.
pub struct BiChannel<T> {
    sender: mpsc::Sender<T>,
    receiver: mpsc::Receiver<T>,
}

impl<T> BiChannel<T> {
    /// Creates a connected pair of `BiChannel`s.
    pub fn new(capacity: usize) -> (BiChannel<T>, BiChannel<T>) {
        let (sender_0, receiver_0) = mpsc::channel(capacity);
        let (sender_1, receiver_1) = mpsc::channel(capacity);
        let bi_channel_0 = BiChannel {
            sender: sender_0,
            receiver: receiver_1,
        };
        let bi_channel_1 = BiChannel {
            sender: sender_1,
            receiver: receiver_0,
        };
        (bi_channel_0, bi_channel_1)
    }
}

impl<T> Stream for BiChannel<T> {
    type Item = T;

    fn poll_next(self: Pin<&mut Self>, cx: &mut task::Context) -> Poll<Option<T>> {
        let this = self.get_mut();
        Pin::new(&mut this.receiver).poll_next(cx)
    }
}

impl<T> Sink<T> for BiChannel<T> {
    type Error = SendError;

    fn poll_flush(self: Pin<&mut Self>, cx: &mut task::Context) -> Poll<Result<(), SendError>> {
        let this = self.get_mut();
        Pin::new(&mut this.sender).poll_flush(cx)
    }

    fn poll_ready(self: Pin<&mut Self>, cx: &mut task::Context) -> Poll<Result<(), SendError>> {
        let this = self.get_mut();
        Pin::new(&mut this.sender).poll_ready(cx)
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut task::Context) -> Poll<Result<(), SendError>> {
        let this = self.get_mut();
        Pin::new(&mut this.sender).poll_close(cx)
    }

    fn start_send(self: Pin<&mut Self>, packet: T) -> Result<(), SendError> {
        let this = self.get_mut();
        Pin::new(&mut this.sender).start_send(packet)
    }
}

/// A two-way channel for sending/receiving IP packets.
pub struct IpChannel {
    packet_channel: BiChannel<Box<IpPacket>>,
}

impl IpChannel {
    /// Creates a connected pair of `IpChannel`s. Packets sent on one will be received on the
    /// other.
    pub fn new(capacity: usize) -> (IpChannel, IpChannel) {
        let (packet_channel_0, packet_channel_1) = BiChannel::new(capacity);
        let channel_0 = IpChannel {
            packet_channel: packet_channel_0,
        };
        let channel_1 = IpChannel {
            packet_channel: packet_channel_1,
        };
        (channel_0, channel_1)
    }
}

impl Stream for IpChannel {
    type Item = io::Result<Box<IpPacket>>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut task::Context) -> Poll<Option<io::Result<Box<IpPacket>>>> {
        let this = self.get_mut();
        match Pin::new(&mut this.packet_channel).poll_next(cx) {
            Poll::Ready(Some(packet)) => Poll::Ready(Some(Ok(packet))),
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}

impl Sink<Box<IpPacket>> for IpChannel {
    type Error = io::Error;

    fn poll_flush(self: Pin<&mut Self>, cx: &mut task::Context) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        match Pin::new(&mut this.packet_channel).poll_flush(cx) {
            Poll::Ready(Ok(())) => Poll::Ready(Ok(())),
            Poll::Ready(Err(err)) => Poll::Ready(Err(as_io_error(err))),
            Poll::Pending => Poll::Pending,
        }
    }

    fn poll_ready(self: Pin<&mut Self>, cx: &mut task::Context) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        match Pin::new(&mut this.packet_channel).poll_ready(cx) {
            Poll::Ready(Ok(())) => Poll::Ready(Ok(())),
            Poll::Ready(Err(err)) => Poll::Ready(Err(as_io_error(err))),
            Poll::Pending => Poll::Pending,
        }
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut task::Context) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        match Pin::new(&mut this.packet_channel).poll_close(cx) {
            Poll::Ready(Ok(())) => Poll::Ready(Ok(())),
            Poll::Ready(Err(err)) => Poll::Ready(Err(as_io_error(err))),
            Poll::Pending => Poll::Pending,
        }
    }

    fn start_send(self: Pin<&mut Self>, packet: Box<IpPacket>) -> io::Result<()> {
        let this = self.get_mut();
        match Pin::new(&mut this.packet_channel).start_send(packet) {
            Ok(()) => Ok(()),
            Err(err) => Err(as_io_error(err)),
        }
    }
}

fn as_io_error(err: SendError) -> io::Error {
    let err = if err.is_disconnected() {
        io::ErrorKind::NotConnected
    } else if err.is_full() {
        io::ErrorKind::WouldBlock
    } else {
        io::ErrorKind::Other
    };
    err.into()
}

impl<T> FusedStream for BiChannel<T> {
    fn is_terminated(&self) -> bool {
        self.receiver.is_terminated()
    }
}

impl FusedStream for IpChannel {
    fn is_terminated(&self) -> bool {
        self.packet_channel.is_terminated()
    }
}

