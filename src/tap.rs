//! Contains utilites for working with virtual (TAP) network interfaces.

use priv_prelude::*;
use libc;
use sys;
use get_if_addrs;

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
        TunPermissionDenied(e: io::Error) {
            description("permission denied to open /dev/net/tun")
            display("permission denied to open /dev/net/tun ({})", e)
            cause(e)
        }
        TunSymbolicLinks(e: io::Error) {
            description("too many symbolic links when resolving path /dev/net/tun")
            display("too many symbolic links when resolving path /dev/net/tun ({})", e)
            cause(e)
        }
        ProcessFileDescriptorLimit(e: io::Error) {
            description("process file descriptor limit hit")
            display("process file descriptor limit hit ({})", e)
            cause(e)
        }
        SystemFileDescriptorLimit(e: io::Error) {
            description("system file descriptor limit hit")
            display("system file descriptor limit hit ({})", e)
            cause(e)
        }
        TunDoesntExist(e: io::Error) {
            description("/dev/net/tun doesn't exist")
            display("/dev/net/tun doesn't exist ({})", e)
            cause(e)
        }
        TunDeviceNotLoaded(e: io::Error) {
            description("driver for /dev/net/tun not loaded")
            display("driver for /dev/net/tun not loaded ({})", e)
            cause(e)
        }
        CreateTapPermissionDenied {
            description("TUNSETIFF ioctl to create tun interface failed with permission denied")
        }
        InterfaceAlreadyExists {
            description("an interface with the given name already exists")
        }
        SetInterfaceAddress(ip: Ipv4Addr, e: io::Error) {
            description("failed to set interface address")
            display("failed to set interface address: {}", e)
            cause(e)
        }
        SetInterfaceNetmask(netmask: Ipv4Addr, e: io::Error) {
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
#[derive(Debug)]
pub struct TapBuilderV4 {
    name: String,
    //address: Option<IfaceAddrV4>,
    address: Ipv4Addr,
    netmask: Ipv4Addr,
    routes: Vec<RouteV4>,
}

/*
/// Contains interface ipv4 address parameters.
pub struct IfaceAddrV4 {
    /// The interface's address
    pub address: Ipv4Addr,
    /// The interface's netmask
    pub netmask: Ipv4Addr,
}
*/

impl Default for TapBuilderV4 {
    fn default() -> TapBuilderV4 {
        TapBuilderV4 {
            name: String::from("netsim"),
            address: ipv4!("0.0.0.0"),
            netmask: ipv4!("0.0.0.0"),
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
    pub fn address(&mut self, address: Ipv4Addr) -> &mut Self {
        self.address = address;
        self
    }

    pub fn netmask(&mut self, netmask: Ipv4Addr) -> &mut Self {
        self.netmask = netmask;
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
        let name = match CString::new(self.name.clone()) {
            Ok(name) => name,
            Err(..) => {
                return Err(TapBuildError::NameContainsNul);
            },
        };
        if name.as_bytes_with_nul().len() > sys::IF_NAMESIZE as usize {
            return Err(TapBuildError::NameTooLong);
        }

        let fd = loop {
            let raw_fd = unsafe {
                libc::open(b"/dev/net/tun\0".as_ptr() as *const _, libc::O_RDWR)
            };
            if raw_fd < 0 {
                let os_err = io::Error::last_os_error();
                match (-raw_fd) as u32 {
                    sys::EACCES => return Err(TapBuildError::TunPermissionDenied(os_err)),
                    sys::EINTR => continue,
                    sys::ELOOP => return Err(TapBuildError::TunSymbolicLinks(os_err)),
                    sys::EMFILE => return Err(TapBuildError::ProcessFileDescriptorLimit(os_err)),
                    sys::ENFILE => return Err(TapBuildError::SystemFileDescriptorLimit(os_err)),
                    sys::ENOENT => return Err(TapBuildError::TunDoesntExist(os_err)),
                    sys::ENXIO => return Err(TapBuildError::TunDeviceNotLoaded(os_err)),
                    _ => {
                        panic!("unexpected error from open(\"/dev/net/tun\"). {}", os_err);
                    },
                }
            }
            break unwrap!(AsyncFd::new(raw_fd));
        };

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
            let os_err = sys::errno();
            match os_err as u32 {
                sys::EPERM => return Err(TapBuildError::CreateTapPermissionDenied),
                sys::EBUSY => {
                    for iface in unwrap!(get_if_addrs::get_if_addrs()) {
                        if iface.name == self.name {
                            return Err(TapBuildError::InterfaceAlreadyExists);
                        }
                    }
                    panic!("unexpected EBUSY error when creating TAP device");
                },
                // TODO: what error do we get if we try to create two interfaces with the same
                // name?
                _ => {
                    panic!("unexpected error creating TAP device: {}", io::Error::from_raw_os_error(os_err));
                },
            }
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
                let os_err = io::Error::last_os_error();
                match (-fd) as u32 {
                    sys::EMFILE => return Err(TapBuildError::ProcessFileDescriptorLimit(os_err)),
                    sys::ENFILE => return Err(TapBuildError::SystemFileDescriptorLimit(os_err)),
                    _ => {
                        panic!("unexpected error when creating dummy socket: {}", os_err);
                    },
                }
            }

            {
                let addr = &mut req.ifr_ifru.ifru_addr;
                let addr = addr as *mut sys::sockaddr;
                let addr = addr as *mut sys::sockaddr_in;
                let addr = &mut *addr;
                addr.sin_family = sys::AF_INET as sys::sa_family_t;
                addr.sin_port = 0;
                addr.sin_addr.s_addr = u32::from(self.address).to_be();
            }

            if ioctl::siocsifaddr(fd, &req) < 0 {
                let _ = sys::close(fd);
                // TODO: what errors occur if we
                //  (a) pick an invalid IP.
                //  (b) pick an IP already in use
                panic!("unexpected error from SIOCSIFADDR ioctl: {}", io::Error::last_os_error());
            }


            {
                let addr = &mut req.ifr_ifru.ifru_addr;
                let addr = addr as *mut sys::sockaddr;
                let addr = addr as *mut sys::sockaddr_in;
                let addr = &mut *addr;
                addr.sin_family = sys::AF_INET as sys::sa_family_t;
                addr.sin_port = 0;
                addr.sin_addr.s_addr = u32::from(self.netmask).to_be();
            }

            if ioctl::siocsifnetmask(fd, &req) < 0 {
                let _ = sys::close(fd);
                // TODO: what error occurs if we try to use an invalid netmask?
                panic!("unexpected error from SIOCSIFNETMASK ioctl: {}", io::Error::last_os_error());
            }

			if ioctl::siocgifflags(fd, &mut req) < 0 {
                let _ = sys::close(fd);
                panic!("unexpected error from SIOCGIFFLAGS ioctl: {}", io::Error::last_os_error());
			}

			req.ifr_ifru.ifru_flags |= (sys::IFF_UP as u32 | sys::IFF_RUNNING as u32) as i16;

			if ioctl::siocsifflags(fd, &mut req) < 0 {
                let _ = sys::close(fd);
                panic!("unexpected error from SIOCSIFFLAGS ioctl: {}", io::Error::last_os_error());
			}
            let _ = sys::close(fd);
		}

        for route in self.routes {
            trace!("adding route {:?} to {}", route, real_name);
            match route.add(&real_name) {
                Ok(()) => (),
                Err(AddRouteError::ProcessFileDescriptorLimit(e)) => {
                    return Err(TapBuildError::ProcessFileDescriptorLimit(e));
                },
                Err(AddRouteError::SystemFileDescriptorLimit(e)) => {
                    return Err(TapBuildError::SystemFileDescriptorLimit(e));
                },
                Err(AddRouteError::NameContainsNul) => unreachable!(),
            }
        }

        trace!("creating TAP");

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
#[derive(Debug)]
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
    type Item = EthernetFrame<Bytes>;
    type Error = io::Error;
    
    fn poll(&mut self) -> io::Result<Async<Option<EthernetFrame<Bytes>>>> {
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
                let frame = EthernetFrame::new(bytes);
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
    type SinkItem = EthernetFrame<Bytes>;
    type SinkError = io::Error;
    
    fn start_send(&mut self, item: EthernetFrame<Bytes>) -> io::Result<AsyncSink<EthernetFrame<Bytes>>> {
        trace!("sending frame to TAP device");
        if let Async::NotReady = self.fd.poll_write() {
            return Ok(AsyncSink::NotReady(item));
        }

        match self.fd.write(item.as_ref()) {
            Ok(n) => assert_eq!(n, item.as_ref().len()),
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                self.fd.need_write();
                return Ok(AsyncSink::NotReady(item));
            }
            Err(e) => return Err(e),
        }
        trace!("sent: {}", PrettyPrinter::print(&item));
        Ok(AsyncSink::Ready)
    }

    fn poll_complete(&mut self) -> io::Result<Async<()>> {
        Ok(Async::Ready(()))
    }
}

#[cfg(test)]
mod test {
    use priv_prelude::*;
    use spawn;
    use capabilities;
    use env_logger;
    use std;
    use ethernet;

    #[test]
    fn build_tap_name_contains_nul() {
        let mut tap_builder = TapBuilderV4::new();
        tap_builder.address(Ipv4Addr::random_global());
        tap_builder.name("hello\0");
        let res = tap_builder.build_unbound();
        match res {
            Err(TapBuildError::NameContainsNul) => (),
            x => panic!("unexpected result: {:?}", x),
        }
    }

    #[test]
    fn build_tap_duplicate_name() {
        let join_handle = spawn::new_namespace(|| {
            let mut tap_builder = TapBuilderV4::new();
            tap_builder.address(Ipv4Addr::random_global());
            tap_builder.name("hello");
            let _tap = unwrap!(tap_builder.build_unbound());
            
            let mut tap_builder = TapBuilderV4::new();
            tap_builder.address(Ipv4Addr::random_global());
            tap_builder.name("hello");
            match tap_builder.build_unbound() {
                Err(TapBuildError::InterfaceAlreadyExists) => (),
                res => panic!("unexpected result: {:?}", res),
            }
        });
        unwrap!(join_handle.join());
    }

    #[test]
    fn build_tap_permission_denied() {
        let join_handle = spawn::new_namespace(|| {
            unwrap!(unwrap!(capabilities::Capabilities::new()).apply());

            let tap_builder = TapBuilderV4::new();
            match tap_builder.build_unbound() {
                Err(TapBuildError::CreateTapPermissionDenied) => (),
                res => panic!("unexpected result: {:?}", res),
            }
        });
        unwrap!(join_handle.join());
    }

    #[test]
    #[ignore]   // currently fails :(
    fn tap_blocks_on_namespaced_side() {
        let _ = env_logger::init();

        const NUM_PACKETS: usize = 500000;
        let mut core = unwrap!(Core::new());
        let handle = core.handle();
        let addr = addrv4!("10.2.3.4:567");

        let res = core.run(future::lazy(move || {
            let (join_handle, tap) = spawn::on_subnet(&handle, SubnetV4::local_10(), move |_ip| {
                let socket = unwrap!(std::net::UdpSocket::bind(addr!("0.0.0.0:0")));
                unwrap!(socket.set_write_timeout(Some(Duration::from_secs(1))));
                for _ in 0..NUM_PACKETS {
                    match socket.send_to(&[], &SocketAddr::V4(addr)) {
                        Ok(_) => (),
                        Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                            return;
                        },
                        Err(e) => panic!("unexpected io error: {}", e),
                    }
                }
                panic!("all packets got sent");
            });

            tap
            .into_future()
            .map_err(|(e, _)| panic!("tap read error: {}", e))
            .and_then(move |(frame_opt, tap)| {
                let frame = unwrap!(frame_opt);
                let mac_addr = ethernet::random_mac();
                let arp = {
                    let src_mac = frame.src_addr();
                    let arp = match frame.ethertype() {
                        EthernetProtocol::Arp => {
                            let frame_ref = EthernetFrame::new(frame.as_ref());
                            ArpPacket::new(frame_ref.payload())
                        },
                        p => panic!("unexpected payload: {:?}", p),
                    };
                    assert_eq!(arp.source_hardware_addr(), src_mac.as_bytes());
                    let src_ip = Ipv4Addr::from(assert_len!(4, arp.source_protocol_addr()));
                    assert_eq!(arp.target_hardware_addr(), EthernetAddress::BROADCAST.as_bytes());
                    assert_eq!(arp.target_protocol_addr(), &addr.ip().octets());
                    ArpPacket::new_reply(
                        mac_addr,
                        *addr.ip(),
                        src_mac,
                        src_ip,
                    )
                };
                let frame = EthernetFrame::new_arp(
                    mac_addr,
                    frame.src_addr(),
                    &arp,
                );

                tap
                .send(frame)
                .map_err(|e| panic!("tap write error: {}", e))
                .map(move |_tap| {
                    unwrap!(join_handle.join());
                })
            })
        }));
        res.void_unwrap()
    }
}

