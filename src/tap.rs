use std::{io, mem, ptr, slice};
use std::io::{Read, Write};
use std::ffi::CString;
use std::os::unix::io::AsRawFd;
use std::net::Ipv4Addr;
use libc::{self, c_int};
use sys;
use bytes::{Bytes, BytesMut};
use futures::{Async, AsyncSink, Stream, Sink};
use tokio_core::reactor::{Handle, PollEvented};
use fd::AsyncFd;
use route::{RouteV4, AddRouteError};
use std::str;
use ethernet::EtherFrame;

quick_error! {
    #[derive(Debug)]
    pub enum TapBuildError {
        NameContainsNul {
            description("interface name contains interior NUL byte")
        }
        NameTooLong {
            description("interface name too long")
        }
        OpenTunController(e: io::Error) {
            description("failed to open /dev/net/tun")
            display("failed to open /dev/net/tun: {}", e)
            cause(e)
        }
        SetControllerAsync(e: io::Error) {
            description("failed to make controller file descriptor non-blocking")
            display("failed to make controller file descriptor non-blocking: {}", e)
            cause(e)
        }
        CreateTun(e: io::Error) {
            description("TUNSETIFF ioctl to create tun interface failed")
            display("TUNSETIFF ioctl to create tun interface failed: {}", e)
            cause(e)
        }
        CreateRoute(e: AddRouteError) {
            description("failed to create route")
            display("failed to create route: {}", e)
            cause(e)
        }
        CreateDummySocket(e: io::Error) {
            description("failed to create dummy socket")
            display("failed to create dummy socket: {}", e)
            cause(e)
        }
        GetInterfaceFlags(e: io::Error) {
            description("failed to get newly created interface's flags")
            display("failed to get newly created interface's flags: {}", e)
            cause(e)
        }
        SetInterfaceFlags(e: io::Error) {
            description("failed to set newly created interface's flags")
            display("failed to set newly created interface's flags: {}", e)
            cause(e)
        }
    }
}

pub struct TapBuilderV4 {
    pub name: String,
    pub address: Option<IfaceAddrV4>,
    pub routes: Vec<RouteV4>,
}

pub struct IfaceAddrV4 {
    pub address: Ipv4Addr,
    pub netmask: Ipv4Addr,
}

impl Default for TapBuilderV4 {
    fn default() -> TapBuilderV4 {
        TapBuilderV4 {
            name: String::from("netsim"),
            address: None,
            routes: Vec::new(),
        }
    }
}

impl Default for IfaceAddrV4 {
    fn default() -> IfaceAddrV4 {
        IfaceAddrV4 {
            address: ipv4!("192.168.0.2"),
            netmask: ipv4!("255.255.255.0"),
        }
    }
}

ioctl!(bad read siocgifflags with 0x8913; sys::ifreq);
ioctl!(bad write siocsifflags with 0x8914; sys::ifreq);

impl TapBuilderV4 {
    pub fn new() -> TapBuilderV4 {
        Default::default()
    }

    pub fn name<S: Into<String>>(&mut self, name: S) -> &mut Self {
        self.name = name.into();
        self
    }

    pub fn address(&mut self, address: IfaceAddrV4) -> &mut Self {
        self.address = Some(address);
        self
    }

    pub fn route(&mut self, route: RouteV4) -> &mut Self {
        self.routes.push(route);
        self
    }

    pub fn build(self, handle: &Handle) -> Result<Tap, TapBuildError> {
        let name = match CString::new(self.name) {
            Ok(name) => name,
            Err(..) => {
                return Err(TapBuildError::NameContainsNul);
            },
        };
        if name.as_bytes_with_nul().len() > sys::IF_NAMESIZE as usize {
            return Err(TapBuildError::NameTooLong);
        }

        let raw_fd = unsafe {
            libc::open(b"/dev/net/tun\0".as_ptr() as *const _, libc::O_RDWR)
        };
        if raw_fd < 0 {
            return Err(TapBuildError::OpenTunController(io::Error::last_os_error()));
        }
		let fd = AsyncFd::new(raw_fd).map_err(TapBuildError::SetControllerAsync)?;

        let mut req = unsafe {
            let mut req: sys::ifreq = mem::zeroed();
            ptr::copy_nonoverlapping(
                name.as_ptr(),
                req.ifr_ifrn.ifrn_name.as_mut_ptr(),
                name.as_bytes().len(),
            );
            //req.ifr_ifru.ifru_flags = (sys::IFF_TAP | sys::IFF_NO_PI | sys::IFF_UP as u32 | sys::IFF_RUNNING as u32) as i16;
            req.ifr_ifru.ifru_flags = (sys::IFF_TAP | sys::IFF_NO_PI) as i16;
            req
        };

        let res = unsafe {
            tunsetiff(fd.as_raw_fd(), &mut req as *mut _ as *mut _)
        };
        if res < 0 {
            return Err(TapBuildError::CreateTun(io::Error::last_os_error()));
        }

        let real_name = {
            let name = unsafe {
                &req.ifr_ifrn.ifrn_name
            };
            let name = match name.iter().position(|b| *b == 0) {
                Some(p) => &name[..p],
                None => name,
            };
            let name = unsafe {
                slice::from_raw_parts(name.as_ptr() as *const _, name.len())
            };
            let name = unwrap!(str::from_utf8(name));
            name.to_owned()
        };

		unsafe {
            let fd = sys::socket(sys::AF_INET as i32, sys::__socket_type::SOCK_DGRAM as i32, 0);
            if fd < 0 {
                return Err(TapBuildError::CreateDummySocket(io::Error::last_os_error()));
            }
			if siocgifflags(fd, &mut req) < 0 {
				return Err(TapBuildError::GetInterfaceFlags(io::Error::last_os_error()));
			}

			req.ifr_ifru.ifru_flags |= (sys::IFF_UP as u32 | sys::IFF_RUNNING as u32) as i16;

			if siocsifflags(fd, &mut req) < 0 {
				return Err(TapBuildError::SetInterfaceFlags(io::Error::last_os_error()));
			}
		}

        for route in self.routes {
            route.add(&real_name).map_err(TapBuildError::CreateRoute)?;
        }

        let fd = unwrap!(PollEvented::new(fd, handle));

        Ok(Tap { fd })
    }
}

pub struct Tap {
    fd: PollEvented<AsyncFd>,
}

ioctl!(write tunsetiff with b'T', 202; c_int);

impl Stream for Tap {
    type Item = EtherFrame;
    type Error = io::Error;
    
    fn poll(&mut self) -> io::Result<Async<Option<EtherFrame>>> {
        if let Async::NotReady = self.fd.poll_read() {
            return Ok(Async::NotReady);
        }

        let mut buffer: [u8; sys::ETH_FRAME_LEN as usize] = unsafe {
            mem::uninitialized()
        };
        match self.fd.read(&mut buffer[..]) {
            Ok(0) => Ok(Async::Ready(None)),
            Ok(n) => {

                'out: for i in 0.. {
                    println!("");
                    for j in 0..4 {
                        let pos = i * 4 + j;
                        if pos < n {
                            print!("{:02x}", buffer[pos]);
                        } else {
                            break 'out;
                        }
                    }
                }
                println!("");

                let bytes = Bytes::from(&buffer[..n]);
                let frame = EtherFrame::from_bytes(bytes);
                Ok(Async::Ready(Some(frame)))
            },
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                self.fd.need_read();
                Ok(Async::NotReady)
            },
            Err(e) => Err(e),
        }
    }
}

impl Sink for Tap {
    type SinkItem = EtherFrame;
    type SinkError = io::Error;
    
    fn start_send(&mut self, item: EtherFrame) -> io::Result<AsyncSink<EtherFrame>> {
        if let Async::NotReady = self.fd.poll_write() {
            return Ok(AsyncSink::NotReady(item));
        }

        match self.fd.write(&item.data()[..]) {
            Ok(n) => assert_eq!(n, item.data().len()),
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                self.fd.need_write();
                return Ok(AsyncSink::NotReady(item));
            }
            Err(e) => return Err(e),
        }
        Ok(AsyncSink::Ready)
    }

    fn poll_complete(&mut self) -> io::Result<Async<()>> {
        Ok(Async::Ready(()))
    }
}

