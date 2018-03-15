use priv_prelude::*;
use tokio_io;

/// A sink for IPv4 packets which writes the packets to a pcap file.
pub struct Ipv4Log {
    fd: PollEvented<AsyncFd>,
    state: LogState,
}

enum LogState {
    WritingHeader {
        header: [u8; 16],
        bytes: Bytes,
        written: usize,
    },
    WritingPayload {
        bytes: Bytes,
        written: usize,
    },
    Ready,
    Invalid,
}

impl Ipv4Log {
    /// Create a new log file.
    pub fn new(handle: &Handle, path: &Path) -> IoFuture<Ipv4Log> {
        const MAGIC: u32 = 0xa1b23c4d;
        const VERSION_MAJOR: u16 = 2;
        const VERSION_MINOR: u16 = 4;
        const LINKTYPE_RAW: u32 = 101;

        let try = || -> io::Result<_> {
            let file = File::open(path)?;
            let raw_fd = file.into_raw_fd();
            let async_fd = AsyncFd::new(raw_fd)?;
            let fd = PollEvented::new(async_fd, handle)?;

            let mut header = [0u8; 24];
            {
                let mut cursor = Cursor::new(&mut header[..]);
                let _ = unwrap!(cursor.write_u32::<NativeEndian>(MAGIC));
                let _ = unwrap!(cursor.write_u16::<NativeEndian>(VERSION_MAJOR));
                let _ = unwrap!(cursor.write_u16::<NativeEndian>(VERSION_MINOR));
                let _ = unwrap!(cursor.write_i32::<NativeEndian>(0));
                let _ = unwrap!(cursor.write_u32::<NativeEndian>(0));
                let _ = unwrap!(cursor.write_u32::<NativeEndian>(65536));
                let _ = unwrap!(cursor.write_u32::<NativeEndian>(LINKTYPE_RAW));
            }

            Ok({
                tokio_io::io::write_all(fd, header)
                .map(|(fd, _header)| {
                    Ipv4Log {
                        fd: fd,
                        state: LogState::Ready,
                    }
                })
            })
        };

        future::result(try()).flatten().into_boxed()
    }
}

impl Sink for Ipv4Log {
    type SinkItem = Ipv4Packet;
    type SinkError = io::Error;

    fn start_send(&mut self, packet: Ipv4Packet) -> io::Result<AsyncSink<Ipv4Packet>> {
        if let Async::NotReady = self.fd.poll_write() {
            return Ok(AsyncSink::NotReady(packet));
        }

        self.poll_complete()?;

        let state = mem::replace(&mut self.state, LogState::Invalid);
        match state {
            LogState::Ready => {
                let bytes = packet.into_bytes();
                let now = unwrap!(SystemTime::now().duration_since(UNIX_EPOCH));
                let mut header = [0u8; 16];
                {
                    let mut cursor = Cursor::new(&mut header[..]);
                    let _ = unwrap!(cursor.write_u32::<NativeEndian>(now.as_secs() as u32));
                    let _ = unwrap!(cursor.write_u32::<NativeEndian>(now.subsec_nanos()));
                    let _ = unwrap!(cursor.write_u32::<NativeEndian>(bytes.len() as u32));
                    let _ = unwrap!(cursor.write_u32::<NativeEndian>(bytes.len() as u32));
                }
                self.state = LogState::WritingHeader {
                    header: header,
                    bytes: bytes,
                    written: 0,
                };
                self.poll_complete()?;
                Ok(AsyncSink::Ready)
            },
            LogState::Invalid => panic!("invalid LogState"),
            _ => {
                self.state = state;
                Ok(AsyncSink::NotReady(packet))
            },
        }
    }

    fn poll_complete(&mut self) -> io::Result<Async<()>> {
        let mut state = mem::replace(&mut self.state, LogState::Invalid);
        loop {
            match state {
                LogState::Invalid => panic!("invalid LogState"),
                LogState::Ready => {
                    self.state = state;
                    return Ok(Async::Ready(()));
                },
                LogState::WritingHeader { header, bytes, written } => {
                    match self.fd.write(&header[written..]) {
                        Ok(n) => {
                            let new_written = written + n;
                            if new_written == header.len() {
                                state = LogState::WritingPayload {
                                    bytes: bytes,
                                    written: 0,
                                };
                            } else {
                                state = LogState::WritingHeader {
                                    header: header,
                                    bytes: bytes,
                                    written: new_written,
                                };
                            }
                            continue;
                        },
                        Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                            self.state = LogState::WritingHeader { header, bytes, written };
                            self.fd.need_write();
                            return Ok(Async::NotReady);
                        }
                        Err(e) => return Err(e),
                    }
                },
                LogState::WritingPayload { bytes, written } => {
                    match self.fd.write(&bytes[written..]) {
                        Ok(n) => {
                            let new_written = written + n;
                            if new_written == bytes.len() {
                                state = LogState::Ready;
                            } else {
                                state = LogState::WritingPayload {
                                    bytes: bytes,
                                    written: new_written,
                                };
                            }
                            continue;
                        },
                        Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                            self.state = LogState::WritingPayload { bytes, written };
                            self.fd.need_write();
                            return Ok(Async::NotReady);
                        }
                        Err(e) => return Err(e),
                    }
                },
            }
        }
    }
}

