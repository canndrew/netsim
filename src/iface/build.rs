use priv_prelude::*;
use sys;
use libc;
use get_if_addrs;

quick_error! {
    /// Error raised when `netsim` fails to build an interface.
    #[allow(missing_docs)]
    #[derive(Debug)]
    pub enum IfaceBuildError {
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
        CreateIfacePermissionDenied {
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

#[derive(Debug)]
pub struct IfaceBuilder {
    pub name: String,
    pub address: Ipv4Addr,
    pub netmask: Ipv4Addr,
    pub routes: Vec<RouteV4>,
}

pub fn build(builder: IfaceBuilder, is_tap: bool) -> Result<AsyncFd, IfaceBuildError> {
    let name = match CString::new(builder.name.clone()) {
        Ok(name) => name,
        Err(..) => {
            return Err(IfaceBuildError::NameContainsNul);
        },
    };
    if name.as_bytes_with_nul().len() > sys::IF_NAMESIZE as usize {
        return Err(IfaceBuildError::NameTooLong);
    }

    let fd = loop {
        let raw_fd = unsafe {
            libc::open(b"/dev/net/tun\0".as_ptr() as *const _, libc::O_RDWR)
        };
        if raw_fd < 0 {
            let os_err = io::Error::last_os_error();
            match (-raw_fd) as u32 {
                sys::EACCES => return Err(IfaceBuildError::TunPermissionDenied(os_err)),
                sys::EINTR => continue,
                sys::ELOOP => return Err(IfaceBuildError::TunSymbolicLinks(os_err)),
                sys::EMFILE => return Err(IfaceBuildError::ProcessFileDescriptorLimit(os_err)),
                sys::ENFILE => return Err(IfaceBuildError::SystemFileDescriptorLimit(os_err)),
                sys::ENOENT => return Err(IfaceBuildError::TunDoesntExist(os_err)),
                sys::ENXIO => return Err(IfaceBuildError::TunDeviceNotLoaded(os_err)),
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
        req.ifr_ifru.ifru_flags = sys::IFF_NO_PI as i16;
        if is_tap {
            req.ifr_ifru.ifru_flags |= sys::IFF_TAP as i16;
        } else {
            req.ifr_ifru.ifru_flags |= sys::IFF_TUN as i16;
        }
        req
    };

    let res = unsafe {
        ioctl::tunsetiff(fd.as_raw_fd(), &mut req as *mut _ as *mut _)
    };
    if res < 0 {
        let os_err = sys::errno();
        match os_err as u32 {
            sys::EPERM => return Err(IfaceBuildError::CreateIfacePermissionDenied),
            sys::EBUSY => {
                for iface in unwrap!(get_if_addrs::get_if_addrs()) {
                    if iface.name == builder.name {
                        return Err(IfaceBuildError::InterfaceAlreadyExists);
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
                sys::EMFILE => return Err(IfaceBuildError::ProcessFileDescriptorLimit(os_err)),
                sys::ENFILE => return Err(IfaceBuildError::SystemFileDescriptorLimit(os_err)),
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
            addr.sin_addr.s_addr = u32::from(builder.address).to_be();
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
            addr.sin_addr.s_addr = u32::from(builder.netmask).to_be();
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

        if ioctl::siocsifflags(fd, &req) < 0 {
            let _ = sys::close(fd);
            panic!("unexpected error from SIOCSIFFLAGS ioctl: {}", io::Error::last_os_error());
        }
        let _ = sys::close(fd);
    }

    for route in builder.routes {
        trace!("adding route {:?} to {}", route, real_name);
        match route.add_to_routing_table(&real_name) {
            Ok(()) => (),
            Err(AddRouteError::ProcessFileDescriptorLimit(e)) => {
                return Err(IfaceBuildError::ProcessFileDescriptorLimit(e));
            },
            Err(AddRouteError::SystemFileDescriptorLimit(e)) => {
                return Err(IfaceBuildError::SystemFileDescriptorLimit(e));
            },
            Err(AddRouteError::NameContainsNul) => unreachable!(),
        }
    }

    Ok(fd)
}

