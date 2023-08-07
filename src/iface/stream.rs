use crate::priv_prelude::*;

pub struct IpIface {
    fd: AsyncFd<OwnedFd>,
    send_packet_opt: Option<Vec<u8>>,
}

impl IpIface {
    pub(crate) fn new(fd: OwnedFd) -> io::Result<IpIface> {
        let fd = AsyncFd::new(fd)?;
        let iface = IpIface {
            fd,
            send_packet_opt: None,
        };
        Ok(iface)
    }
}

impl Sink<Vec<u8>> for IpIface {
    type Error = io::Error;

    fn poll_ready(
        self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> Poll<io::Result<()>> {
        Self::poll_flush(self, cx)
    }

    fn start_send(self: Pin<&mut Self>, item: Vec<u8>) -> io::Result<()> {
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
                        packet.as_slice().as_ptr() as *const libc::c_void,
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
    type Item = io::Result<Vec<u8>>;

    fn poll_next(
        self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> Poll<Option<io::Result<Vec<u8>>>> {
        let this = self.get_mut();
        loop {
            let mut guard = ready!(this.fd.poll_read_ready(cx))?;
            // TODO: don't initialize the buffer once MaybeUninit features are stable
            let mut buffer = [0u8; libc::ETH_FRAME_LEN as usize];
            match guard.try_io(|fd| {
                let res = unsafe {
                    libc::read(
                        fd.as_raw_fd(),
                        buffer.as_mut_slice().as_mut_ptr() as *mut libc::c_void,
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
                        return Poll::Ready(Some(Ok(buffer[..n].to_vec())));
                    }
                },
                Ok(Err(err)) => return Poll::Ready(Some(Err(err))),
                Err(_would_block) => continue,
            }
        }
    }
}
