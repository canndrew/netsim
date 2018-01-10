//! Contains utilites for working with virtual (TAP) network interfaces.

use priv_prelude::*;
use libc;
use sys;

quick_error! {
    /// Error returned by `TapBuilderV4::build`
    #[allow(missing_docs)]
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
        SetInterfaceAddress(e: io::Error) {
            description("failed to set interface address")
            display("failed to set interface address: {}", e)
            cause(e)
        }
        SetInterfaceNetmask(e: io::Error) {
            description("failed to set interface netmask")
            display("failed to set interface netmask: {}", e)
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

/// This object can be used to set the configuration options for a `Tap` before creating the `Tap`
/// using `build`.
pub struct TapBuilderV4 {
    name: String,
    address: Option<IfaceAddrV4>,
    routes: Vec<RouteV4>,
}

// TODO: don't really need this type.
/// Contains interface ipv4 address parameters.
pub struct IfaceAddrV4 {
    /// The interface's address
    pub address: Ipv4Addr,
    /// The interface's netmask
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

mod ioctl {
    use priv_prelude::*;
    use sys;

    ioctl!(bad read siocgifflags with 0x8913; sys::ifreq);
    ioctl!(bad write siocsifflags with 0x8914; sys::ifreq);
    ioctl!(bad write siocsifaddr with 0x8916; sys::ifreq);
    //ioctl!(bad read siocgifnetmask with 0x891b; sys::ifreq);
    ioctl!(bad write siocsifnetmask with 0x891c; sys::ifreq);
    ioctl!(write tunsetiff with b'T', 202; c_int);
}

impl TapBuilderV4 {
    /// Start building a new `Tap` with the default configuration options.
    pub fn new() -> TapBuilderV4 {
        Default::default()
    }

    /// Set the interface name.
    pub fn name<S: Into<String>>(&mut self, name: S) -> &mut Self {
        self.name = name.into();
        self
    }

    /// Set the interface address and netmask.
    pub fn address(&mut self, address: IfaceAddrV4) -> &mut Self {
        self.address = Some(address);
        self
    }

    /// Add a route to the set of routes that will be created and directed through this interface.
    pub fn route(&mut self, route: RouteV4) -> &mut Self {
        self.routes.push(route);
        self
    }

    /// Consume this `TapBuilderV4` and build a `PreTap`. This creates the TAP device but does not
    /// bind it to a tokio event loop. This is useful if the event loop lives in a different thread
    /// to where you need to create the device. You can send a `PreTap` to another thread then
    /// `bind` it to create your `Tap`.
    pub fn build_unbound(self) -> Result<PreTap, TapBuildError> {
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
            ioctl::tunsetiff(fd.as_raw_fd(), &mut req as *mut _ as *mut _)
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

            if let Some(address) = self.address {
                {
                    let addr = &mut req.ifr_ifru.ifru_addr;
                    let addr = addr as *mut sys::sockaddr;
                    let addr = addr as *mut sys::sockaddr_in;
                    let addr = &mut *addr;
                    addr.sin_family = sys::AF_INET as sys::sa_family_t;
                    addr.sin_port = 0;
                    addr.sin_addr.s_addr = u32::from(address.address).to_be();
                }

                if ioctl::siocsifaddr(fd, &req) < 0 {
                    return Err(TapBuildError::SetInterfaceAddress(io::Error::last_os_error()));
                }


                {
                    let addr = &mut req.ifr_ifru.ifru_addr;
                    let addr = addr as *mut sys::sockaddr;
                    let addr = addr as *mut sys::sockaddr_in;
                    let addr = &mut *addr;
                    addr.sin_family = sys::AF_INET as sys::sa_family_t;
                    addr.sin_port = 0;
                    addr.sin_addr.s_addr = u32::from(address.netmask).to_be();
                }

                if ioctl::siocsifnetmask(fd, &req) < 0 {
                    return Err(TapBuildError::SetInterfaceNetmask(io::Error::last_os_error()));
                }
            }

			if ioctl::siocgifflags(fd, &mut req) < 0 {
				return Err(TapBuildError::GetInterfaceFlags(io::Error::last_os_error()));
			}

			req.ifr_ifru.ifru_flags |= (sys::IFF_UP as u32 | sys::IFF_RUNNING as u32) as i16;

			if ioctl::siocsifflags(fd, &mut req) < 0 {
				return Err(TapBuildError::SetInterfaceFlags(io::Error::last_os_error()));
			}
		}

        for route in self.routes {
            route.add(&real_name).map_err(TapBuildError::CreateRoute)?;
        }


        Ok(PreTap { fd })
    }

    /// Consume this `TapBuilderV4` and build the TAP interface. The returned `Tap` object can be
    /// used to read/write ethernet frames from this interface. `handle` is a handle to a tokio
    /// event loop which will be used for reading/writing.
    pub fn build(self, handle: &Handle) -> Result<Tap, TapBuildError> {
        Ok(self.build_unbound()?.bind(handle))
    }
}

/// Represents a TAP device which has been built but not bound to a tokio event loop.
pub struct PreTap {
    fd: AsyncFd,
}

impl PreTap {
    /// Bind the tap device to the event loop, creating a `Tap` which you can read/write ethernet
    /// frames with.
    pub fn bind(self, handle: &Handle) -> Tap {
        let PreTap { fd } = self;
        let fd = unwrap!(PollEvented::new(fd, handle));
        Tap { fd }
    }
}

/// A handle to a virtual (TAP) network interface. Can be used to read/write ethernet frames
/// directly to the device.
pub struct Tap {
    fd: PollEvented<AsyncFd>,
}

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

                /*
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
                */

                let bytes = Bytes::from(&buffer[..n]);
                let frame = EtherFrame::from_bytes(bytes);
                trace!("reading frame: {:?}", frame);
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
        trace!("sending frame to TAP device");
        if let Async::NotReady = self.fd.poll_write() {
            return Ok(AsyncSink::NotReady(item));
        }

        match self.fd.write(&item.as_bytes()[..]) {
            Ok(n) => assert_eq!(n, item.as_bytes().len()),
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                self.fd.need_write();
                return Ok(AsyncSink::NotReady(item));
            }
            Err(e) => return Err(e),
        }
        trace!("sent: {:?}", item);
        Ok(AsyncSink::Ready)
    }

    fn poll_complete(&mut self) -> io::Result<Async<()>> {
        Ok(Async::Ready(()))
    }
}

