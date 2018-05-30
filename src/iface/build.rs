use priv_prelude::*;
use sys;
use libc;
use get_if_addrs;
use iface;
use ioctl;

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
        MacAddrNotAvailable(e: io::Error) {
            description("the requested MAC address is invalid or already in use")
            display("the requested MAC address is invalid or already in use: {}", e)
            cause(e)
        }
        Ipv4AddrNotAvailable(e: io::Error) {
            description("the requested IPv4 address is invalid or already in use")
            display("the requested IPv4 address is invalid or already in use: {}", e)
            cause(e)
        }
        Ipv6AddrNotAvailable(e: io::Error) {
            description("the requested IPv6 address is invalid or already in use")
            display("the requested IPv6 address is invalid or already in use: {}", e)
            cause(e)
        }
    }
}

#[derive(Debug)]
pub struct IfaceBuilder {
    pub name: String,
    pub ipv4_addr: Option<(Ipv4Addr, u8)>,
    pub ipv6_addr: Option<(Ipv6Addr, u8)>,
    pub ipv4_routes: Vec<Ipv4Route>,
    pub ipv6_routes: Vec<Ipv6Route>,
}

pub fn build(builder: IfaceBuilder, mac_addr: Option<MacAddr>) -> Result<AsyncFd, IfaceBuildError> {
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
            match sys::errno() {
                libc::EACCES => return Err(IfaceBuildError::TunPermissionDenied(os_err)),
                libc::EINTR => continue,
                libc::ELOOP => return Err(IfaceBuildError::TunSymbolicLinks(os_err)),
                libc::EMFILE => return Err(IfaceBuildError::ProcessFileDescriptorLimit(os_err)),
                libc::ENFILE => return Err(IfaceBuildError::SystemFileDescriptorLimit(os_err)),
                libc::ENOENT => return Err(IfaceBuildError::TunDoesntExist(os_err)),
                libc::ENXIO => return Err(IfaceBuildError::TunDeviceNotLoaded(os_err)),
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
        if mac_addr.is_some() {
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
        match os_err {
            libc::EPERM => return Err(IfaceBuildError::CreateIfacePermissionDenied),
            libc::EBUSY => {
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

    if let Some(mac_addr) = mac_addr {
        match iface::set_mac_addr(&real_name, mac_addr) {
            Ok(()) => (),
            Err(SetMacAddrError::UnknownInterface)
                => panic!("the interface we just created doesn't exist?"),
            Err(SetMacAddrError::PermissionDenied(..))
                => panic!("don't have permission to configure the interface we just created?"),
            Err(SetMacAddrError::AddrNotAvailable(e))
                => return Err(IfaceBuildError::MacAddrNotAvailable(e)),
            Err(SetMacAddrError::ProcessFileDescriptorLimit(e))
                => return Err(IfaceBuildError::ProcessFileDescriptorLimit(e)),
            Err(SetMacAddrError::SystemFileDescriptorLimit(e))
                => return Err(IfaceBuildError::SystemFileDescriptorLimit(e)),
        }
    }

    if let Some((ipv4_addr, ipv4_netmask_bits)) = builder.ipv4_addr {
        match iface::set_ipv4_addr(&real_name, ipv4_addr, ipv4_netmask_bits) {
            Ok(()) => (),
            Err(SetIpv4AddrError::UnknownInterface)
                => panic!("the interface we just created doesn't exist?"),
            Err(SetIpv4AddrError::PermissionDenied(..))
                => panic!("don't have permission to configure the interface we just created?"),
            Err(SetIpv4AddrError::AddrNotAvailable(e))
                => return Err(IfaceBuildError::Ipv4AddrNotAvailable(e)),
            Err(SetIpv4AddrError::ProcessFileDescriptorLimit(e))
                => return Err(IfaceBuildError::ProcessFileDescriptorLimit(e)),
            Err(SetIpv4AddrError::SystemFileDescriptorLimit(e))
                => return Err(IfaceBuildError::SystemFileDescriptorLimit(e)),
        }
    }

    if let Some((ipv6_addr, ipv6_netmask_bits)) = builder.ipv6_addr {
        match iface::set_ipv6_addr(&real_name, ipv6_addr, ipv6_netmask_bits) {
            Ok(()) => (),
            Err(SetIpv6AddrError::UnknownInterface)
                => panic!("the interface we just created doesn't exist?"),
            Err(SetIpv6AddrError::PermissionDenied(..))
                => panic!("don't have permission to configure the interface we just created?"),
            Err(SetIpv6AddrError::AddrNotAvailable(e))
                => return Err(IfaceBuildError::Ipv6AddrNotAvailable(e)),
            Err(SetIpv6AddrError::ProcessFileDescriptorLimit(e))
                => return Err(IfaceBuildError::ProcessFileDescriptorLimit(e)),
            Err(SetIpv6AddrError::SystemFileDescriptorLimit(e))
                => return Err(IfaceBuildError::SystemFileDescriptorLimit(e)),
        }
    }

    match iface::put_up(&real_name) {
        Ok(()) => (),
        Err(PutUpError::UnknownInterface)
            => panic!("the interface we just created doesn't exist?"),
        Err(PutUpError::PermissionDenied(..))
            => panic!("don't have permission to configure the interface we just created?"),
        Err(PutUpError::ProcessFileDescriptorLimit(e))
            => return Err(IfaceBuildError::ProcessFileDescriptorLimit(e)),
        Err(PutUpError::SystemFileDescriptorLimit(e))
            => return Err(IfaceBuildError::SystemFileDescriptorLimit(e)),
    }

    for route in builder.ipv4_routes {
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

    for route in builder.ipv6_routes {
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

