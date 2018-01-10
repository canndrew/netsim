use priv_prelude::*;
use mio::{Evented, Poll, Token, PollOpt, Ready};
use mio::unix::EventedFd;
use libc;

pub struct AsyncFd(RawFd);

impl AsyncFd {
    pub fn new(fd: RawFd) -> io::Result<AsyncFd> {
        if fd < 0 {
            return Err(io::ErrorKind::InvalidInput.into());
        }

        let flags = unsafe {
            libc::fcntl(fd, libc::F_GETFL, 0)
        };
        if flags < 0 {
            return Err(io::Error::last_os_error());
        }
    
        let res = unsafe {
            libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK)
        };
        if res < 0 {
            return Err(io::Error::last_os_error());
        }

        Ok(AsyncFd(fd))
    }
}

impl Drop for AsyncFd {
    fn drop(&mut self) {
        unsafe {
            let AsyncFd(fd) = *self;
            libc::close(fd);
        }
    }
}

impl Read for AsyncFd {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let res = unsafe {
            libc::read(self.as_raw_fd(), buf.as_mut_ptr() as *mut _, buf.len())
        };
        if res < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(res as usize)
    }
}

impl Write for AsyncFd {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let res = unsafe {
            libc::write(self.as_raw_fd(), buf.as_ptr() as *mut _, buf.len())
        };
        if res < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(res as usize)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl AsyncRead for AsyncFd {}

impl AsyncWrite for AsyncFd {
    fn shutdown(&mut self) -> io::Result<Async<()>> {
        Ok(Async::Ready(()))
    }
}

impl AsRawFd for AsyncFd {
    fn as_raw_fd(&self) -> RawFd {
        let AsyncFd(ret) = *self;
        ret
    }
}

impl Evented for AsyncFd {
    fn register(
        &self, 
        poll: &Poll, 
        token: Token, 
        interest: Ready, 
        opts: PollOpt
    ) -> io::Result<()> {
        let fd = self.as_raw_fd();
        let evented_fd = EventedFd(&fd);
        evented_fd.register(poll, token, interest, opts)
    }

    fn reregister(
        &self, 
        poll: &Poll, 
        token: Token, 
        interest: Ready, 
        opts: PollOpt
    ) -> io::Result<()> {
        let fd = self.as_raw_fd();
        let evented_fd = EventedFd(&fd);
        evented_fd.reregister(poll, token, interest, opts)
    }
    
    fn deregister(&self, poll: &Poll) -> io::Result<()> {
        let fd = self.as_raw_fd();
        let evented_fd = EventedFd(&fd);
        evented_fd.deregister(poll)
    }
}

