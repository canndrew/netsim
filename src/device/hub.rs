use crate::priv_prelude::*;

/// A simple IP network hub.
///
/// Insert any number of interfaces into the hub and any packet
/// received on one will be forwarded to all other interfaces.
pub struct IpHub {
    iface_sender: mpsc::UnboundedSender<Pin<Box<dyn IpSinkStream>>>,
}

struct IpHubTask {
    iface_receiver: mpsc::UnboundedReceiver<Pin<Box<dyn IpSinkStream>>>,
    ifaces: Vec<Pin<Box<dyn IpSinkStream>>>,
}

impl IpHub {
    /// Create a new `IpHub`. Must be called within a `tokio` context.
    #[cfg_attr(feature="cargo-clippy", allow(clippy::new_without_default))]
    pub fn new() -> IpHub {
        let (iface_sender, iface_receiver) = mpsc::unbounded();
        let task = IpHubTask {
            iface_receiver,
            ifaces: Vec::new(),
        };
        tokio::spawn(task);
        IpHub { iface_sender }
    }

    /// Insert a `Sink`/`Stream` of IP packets into the hub. Any packet created by this `Stream`
    /// will be forwarded to all other inserted `Sink`s and this `Sink` will receive any packet
    /// produced by any other inserted `Stream`.
    pub fn insert_iface<S>(&mut self, iface: S)
    where
        S: IpSinkStream,
    {
        let iface = Box::pin(iface);
        self.iface_sender.unbounded_send(iface).unwrap();
    }
}

impl IpHubTask {
    fn poll_flush_outgoing(&mut self, cx: &mut task::Context) -> Poll<()> {
        let mut index = 0;
        let mut any_pending = false;
        while let Some(iface) = self.ifaces.get_mut(index) {
            match iface.as_mut().poll_flush(cx) {
                Poll::Ready(Ok(())) => (),
                Poll::Ready(Err(_)) => {
                    self.ifaces.remove(index);
                    continue;
                },
                Poll::Pending => {
                    any_pending = true;
                },
            }
            index += 1;
        }
        if any_pending {
            Poll::Pending
        } else {
            Poll::Ready(())
        }
    }

    fn poll_ready_outgoing(&mut self, cx: &mut task::Context) -> Poll<()> {
        match self.poll_flush_outgoing(cx) {
            Poll::Ready(()) => return Poll::Ready(()),
            Poll::Pending => (),
        }

        let mut index = 0;
        let mut any_pending = false;
        while let Some(iface) = self.ifaces.get_mut(index) {
            match iface.as_mut().poll_ready(cx) {
                Poll::Ready(Ok(())) => (),
                Poll::Ready(Err(_)) => {
                    self.ifaces.swap_remove(index);
                    continue;
                },
                Poll::Pending => {
                    any_pending = true;
                },
            }
            index += 1;
        }
        if any_pending {
            Poll::Pending
        } else {
            Poll::Ready(())
        }
    }

    fn start_send_outgoing(&mut self, mut recv_index: usize, packet: Box<IpPacket>) {
        let mut send_index = 0;
        loop {
            if send_index == recv_index {
                send_index += 1;
                continue;
            }
            let iface = match self.ifaces.get_mut(send_index) {
                Some(iface) => iface,
                None => break,
            };
            match iface.as_mut().start_send(packet.clone()) {
                Ok(()) => (),
                Err(_) => {
                    self.ifaces.swap_remove(send_index);
                    if recv_index == self.ifaces.len() {
                        recv_index = send_index;
                    }
                    continue;
                },
            }
            send_index += 1;
        }
    }

    fn poll_next_incoming(&mut self, cx: &mut task::Context) -> Poll<(usize, Box<IpPacket>)> {
        let mut index = 0;
        while let Some(iface) = self.ifaces.get_mut(index) {
            match iface.as_mut().poll_next(cx) {
                Poll::Ready(Some(Ok(packet))) => {
                    return Poll::Ready((index, packet));
                },
                Poll::Ready(Some(Err(_))) | Poll::Ready(None) => {
                    self.ifaces.swap_remove(index);
                    continue;
                },
                Poll::Pending => (),
            }
            index += 1;
        }
        Poll::Pending
    }

    fn poll_inner(&mut self, cx: &mut task::Context) -> Poll<()> {
        loop {
            match Pin::new(&mut self.iface_receiver).poll_next(cx) {
                Poll::Ready(Some(iface)) => {
                    self.ifaces.push(iface);
                },
                Poll::Ready(None) => {
                    return Poll::Ready(());
                },
                Poll::Pending => break,
            }
        }

        loop {
            match self.poll_ready_outgoing(cx) {
                Poll::Ready(()) => (),
                Poll::Pending => return Poll::Pending,
            }

            let (recv_index, packet) = match self.poll_next_incoming(cx) {
                Poll::Ready((index, packet)) => (index, packet),
                Poll::Pending => return Poll::Pending,
            };

            if log_enabled!(Level::Debug) {
                debug!("recieved on iface #{} {:?}", recv_index, packet);
            }

            self.start_send_outgoing(recv_index, packet);
        }
    }
}

impl Future for IpHubTask {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut task::Context) -> Poll<()> {
        let this = self.get_mut();
        this.poll_inner(cx)
    }
}

