use priv_prelude::*;
use tokio;

/// A sink for IP packets which writes the packets to a pcap file.
pub struct IpLog {
    fd: PollEvented2<AsyncFd>,
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

impl IpLog {
    /// Create a new log file.
    pub fn new(path: &Path) -> IoFuture<IpLog> {
        const MAGIC: u32 = 0xa1b2_3c4d;
        const VERSION_MAJOR: u16 = 2;
        const VERSION_MINOR: u16 = 4;
        const LINKTYPE_RAW: u32 = 101;

        let try = || -> io::Result<_> {
            let file = File::open(path)?;
            let raw_fd = file.into_raw_fd();
            let async_fd = AsyncFd::new(raw_fd)?;
            let fd = PollEvented2::new(async_fd);

            let mut header = [0u8; 24];
            {
                let mut cursor = Cursor::new(&mut header[..]);
                unwrap!(cursor.write_u32::<NativeEndian>(MAGIC));
                unwrap!(cursor.write_u16::<NativeEndian>(VERSION_MAJOR));
                unwrap!(cursor.write_u16::<NativeEndian>(VERSION_MINOR));
                unwrap!(cursor.write_i32::<NativeEndian>(0));
                unwrap!(cursor.write_u32::<NativeEndian>(0));
                unwrap!(cursor.write_u32::<NativeEndian>(65_536));
                unwrap!(cursor.write_u32::<NativeEndian>(LINKTYPE_RAW));
            }

            Ok({
                tokio::io::write_all(fd, header)
                .map(|(fd, _header)| {
                    IpLog {
                        fd,
                        state: LogState::Ready,
                    }
                })
            })
        };

        future::result(try()).flatten().into_boxed()
    }
}

impl Sink for IpLog {
    type SinkItem = IpPacket;
    type SinkError = io::Error;

    fn start_send(&mut self, packet: IpPacket) -> io::Result<AsyncSink<IpPacket>> {
        if let Async::NotReady = self.fd.poll_write_ready()? {
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
                    unwrap!(cursor.write_u32::<NativeEndian>(now.as_secs() as u32));
                    unwrap!(cursor.write_u32::<NativeEndian>(now.subsec_nanos()));
                    unwrap!(cursor.write_u32::<NativeEndian>(bytes.len() as u32));
                    unwrap!(cursor.write_u32::<NativeEndian>(bytes.len() as u32));
                }
                self.state = LogState::WritingHeader {
                    header,
                    bytes,
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
                                    bytes,
                                    written: 0,
                                };
                            } else {
                                state = LogState::WritingHeader {
                                    header,
                                    bytes,
                                    written: new_written,
                                };
                            }
                            continue;
                        },
                        Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                            self.state = LogState::WritingHeader { header, bytes, written };
                            self.fd.clear_write_ready()?;
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
                                    bytes,
                                    written: new_written,
                                };
                            }
                            continue;
                        },
                        Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                            self.state = LogState::WritingPayload { bytes, written };
                            self.fd.clear_write_ready()?;
                            return Ok(Async::NotReady);
                        }
                        Err(e) => return Err(e),
                    }
                },
            }
        }
    }
}

