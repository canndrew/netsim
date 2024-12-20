use crate::priv_prelude::*;

/// A handle to a network interface of a [`Machine`](crate::Machine).
///
/// This can be dropped to remove the interface. It also implements `Stream`+`Sink` in order to
/// read/write packets to the interface. That is, packets sent by tasks executing on the
/// corresponding machine will arrive on this `IpIface`'s `Stream` implementation (if they left the
/// machine via that interface) and you can write packets to the `IpIface`'s `Sink` implementation
/// to make packets arrive on that interface.
pub struct IpIface {
    fd: AsyncFd<OwnedFd>,
    incoming_bytes: BytesMut,
    send_packet_opt: Option<Box<IpPacket>>,
}

impl IpIface {
    pub(crate) fn new(fd: OwnedFd) -> io::Result<IpIface> {
        let fd = AsyncFd::new(fd)?;
        let iface = IpIface {
            fd,
            incoming_bytes: BytesMut::new(),
            send_packet_opt: None,
        };
        Ok(iface)
    }
}

impl Sink<Box<IpPacket>> for IpIface {
    type Error = io::Error;

    fn poll_ready(
        self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> Poll<io::Result<()>> {
        Self::poll_flush(self, cx)
    }

    fn start_send(self: Pin<&mut Self>, item: Box<IpPacket>) -> io::Result<()> {
        let this = self.get_mut();
        let send_packet_opt = this.send_packet_opt.replace(item);
        assert!(send_packet_opt.is_none());
        Ok(())
    }

    fn poll_flush(
        self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        let packet = match this.send_packet_opt.take() {
            Some(packet) => packet,
            None => return Poll::Ready(Ok(())),
        };
        if packet.len() > 1500 {
            return Poll::Ready(Ok(()));
        }
        loop {
            let mut guard = ready!(this.fd.poll_write_ready(cx))?;
            match guard.try_io(|fd| {
                let res = unsafe {
                    libc::write(
                        fd.as_raw_fd(),
                        packet.as_bytes().as_ptr() as *const libc::c_void,
                        packet.len(),
                    )
                };
                if res < 0 {
                    let err = io::Error::last_os_error();
                    return Err(err);
                }
                Ok(res as usize)
            }) {
                Ok(Ok(n)) => {
                    assert_eq!(n, packet.len());
                    return Poll::Ready(Ok(()));
                },
                Ok(Err(err)) => return Poll::Ready(Err(err)),
                Err(_would_block) => continue,
            }
        }
    }

    fn poll_close(
        self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> Poll<io::Result<()>> {
        Self::poll_flush(self, cx)
    }
}

impl Stream for IpIface {
    type Item = io::Result<Box<IpPacket>>;

    fn poll_next(
        self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> Poll<Option<io::Result<Box<IpPacket>>>> {
        let this = self.get_mut();
        loop {
            let mut guard = ready!(this.fd.poll_read_ready(cx))?;
            this.incoming_bytes.reserve(libc::ETH_FRAME_LEN as usize);
            let buffer = this.incoming_bytes.spare_capacity_mut();
            match guard.try_io(|fd| {
                let res = unsafe {
                    libc::read(
                        fd.as_raw_fd(),
                        buffer.as_mut_ptr() as *mut libc::c_void,
                        buffer.len(),
                    )
                };
                if res < 0 {
                    let err = io::Error::last_os_error();
                    return Err(err);
                }
                Ok(res as usize)
            }) {
                Ok(Ok(n)) => {
                    if n == 0 {
                        return Poll::Ready(None);
                    } else {
                        assert_eq!(this.incoming_bytes.len(), 0);
                        unsafe {
                            this.incoming_bytes.set_len(n);
                        }
                        let data = this.incoming_bytes.split();
                        let packet = IpPacket::new_box(Box::from(&data[..]));
                        return Poll::Ready(Some(Ok(packet)));
                    }
                },
                Ok(Err(err)) => return Poll::Ready(Some(Err(err))),
                Err(_would_block) => continue,
            }
        }
    }
}

/// Helper trait for types which are both `Sink`s and `Stream`s of `Box<IpPacket>`.
pub trait IpSinkStream:
    Stream<Item = io::Result<Box<IpPacket>>> + Sink<Box<IpPacket>, Error = io::Error> + Send + 'static
{}

impl<T> IpSinkStream for T
where
    T: Stream<Item = io::Result<Box<IpPacket>>> + Sink<Box<IpPacket>, Error = io::Error> + Send + 'static,
{}

