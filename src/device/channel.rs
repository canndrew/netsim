use crate::priv_prelude::*;

pub struct IpChannel {
    packet_sender: mpsc::Sender<Box<IpPacket>>,
    packet_receiver: mpsc::Receiver<Box<IpPacket>>,
}

impl IpChannel {
    pub fn new(capacity: usize) -> (IpChannel, IpChannel) {
        let (packet_sender_0, packet_receiver_0) = mpsc::channel(capacity);
        let (packet_sender_1, packet_receiver_1) = mpsc::channel(capacity);
        let channel_0 = IpChannel {
            packet_sender: packet_sender_0,
            packet_receiver: packet_receiver_1,
        };
        let channel_1 = IpChannel {
            packet_sender: packet_sender_1,
            packet_receiver: packet_receiver_0,
        };
        (channel_0, channel_1)
    }
}

impl Stream for IpChannel {
    type Item = io::Result<Box<IpPacket>>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut task::Context) -> Poll<Option<io::Result<Box<IpPacket>>>> {
        let this = self.get_mut();
        match Pin::new(&mut this.packet_receiver).poll_next(cx) {
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
        match Pin::new(&mut this.packet_sender).poll_flush(cx) {
            Poll::Ready(Ok(())) => Poll::Ready(Ok(())),
            Poll::Ready(Err(_)) => Poll::Ready(Err(io::ErrorKind::NotConnected.into())),
            Poll::Pending => Poll::Pending,
        }
    }

    fn poll_ready(self: Pin<&mut Self>, cx: &mut task::Context) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        match Pin::new(&mut this.packet_sender).poll_ready(cx) {
            Poll::Ready(Ok(())) => Poll::Ready(Ok(())),
            Poll::Ready(Err(_)) => Poll::Ready(Err(io::ErrorKind::NotConnected.into())),
            Poll::Pending => Poll::Pending,
        }
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut task::Context) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        match Pin::new(&mut this.packet_sender).poll_close(cx) {
            Poll::Ready(Ok(())) => Poll::Ready(Ok(())),
            Poll::Ready(Err(_)) => Poll::Ready(Err(io::ErrorKind::NotConnected.into())),
            Poll::Pending => Poll::Pending,
        }
    }

    fn start_send(self: Pin<&mut Self>, packet: Box<IpPacket>) -> io::Result<()> {
        let this = self.get_mut();
        match Pin::new(&mut this.packet_sender).start_send(packet) {
            Ok(()) => Ok(()),
            Err(_) => Err(io::ErrorKind::NotConnected.into()),
        }
    }
}
