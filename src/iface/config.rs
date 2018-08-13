use priv_prelude::*;
use sys;
use ioctl;
use libc;

quick_error! {
    #[derive(Debug)]
    /// Errors raised when configuring network interfaces.
    pub enum SetMacAddrError {
        /// There is no interface with the given name
        UnknownInterface {
            description("there is no interface with the given name")
        }
        /// Process file descriptor limit hit
        ProcessFileDescriptorLimit(e: io::Error) {
            description("process file descriptor limit hit")
            display("process file descriptor limit hit ({})", e)
            cause(e)
        }
        /// System file descriptor limit hit
        SystemFileDescriptorLimit(e: io::Error) {
            description("system file descriptor limit hit")
            display("system file descriptor limit hit ({})", e)
            cause(e)
        }
        /// Permission denied
        PermissionDenied(e: io::Error) {
            description("permission denied")
            display("permission denied: {}", e)
            cause(e)
        }
        /// Address not available
        AddrNotAvailable(e: io::Error) {
            description("the address is invalid or already in use")
            display("the address is invalid or already in use: {}", e)
            cause(e)
        }
    }
}

quick_error! {
    #[derive(Debug)]
    /// Errors raised when configuring network interfaces.
    pub enum GetMacAddrError {
        /// There is no interface with the given name
        UnknownInterface {
            description("there is no interface with the given name")
        }
        /// Process file descriptor limit hit
        ProcessFileDescriptorLimit(e: io::Error) {
            description("process file descriptor limit hit")
            display("process file descriptor limit hit ({})", e)
            cause(e)
        }
        /// System file descriptor limit hit
        SystemFileDescriptorLimit(e: io::Error) {
            description("system file descriptor limit hit")
            display("system file descriptor limit hit ({})", e)
            cause(e)
        }
    }
}

quick_error! {
    #[derive(Debug)]
    /// Errors raised when configuring network interfaces.
    pub enum SetIpv4AddrError {
        /// There is no interface with the given name
        UnknownInterface {
            description("there is no interface with the given name")
        }
        /// Process file descriptor limit hit
        ProcessFileDescriptorLimit(e: io::Error) {
            description("process file descriptor limit hit")
            display("process file descriptor limit hit ({})", e)
            cause(e)
        }
        /// System file descriptor limit hit
        SystemFileDescriptorLimit(e: io::Error) {
            description("system file descriptor limit hit")
            display("system file descriptor limit hit ({})", e)
            cause(e)
        }
        /// Permission denied
        PermissionDenied(e: io::Error) {
            description("permission denied")
            display("permission denied: {}", e)
            cause(e)
        }
        /// Address not available
        AddrNotAvailable(e: io::Error) {
            description("the address is invalid or already in use")
            display("the address is invalid or already in use: {}", e)
            cause(e)
        }
    }
}

quick_error! {
    #[derive(Debug)]
    /// Errors raised when configuring network interfaces.
    pub enum SetIpv6AddrError {
        /// There is no interface with the given name
        UnknownInterface {
            description("there is no interface with the given name")
        }
        /// Process file descriptor limit hit
        ProcessFileDescriptorLimit(e: io::Error) {
            description("process file descriptor limit hit")
            display("process file descriptor limit hit ({})", e)
            cause(e)
        }
        /// System file descriptor limit hit
        SystemFileDescriptorLimit(e: io::Error) {
            description("system file descriptor limit hit")
            display("system file descriptor limit hit ({})", e)
            cause(e)
        }
        /// Permission denied
        PermissionDenied(e: io::Error) {
            description("permission denied")
            display("permission denied: {}", e)
            cause(e)
        }
        /// Address not available
        AddrNotAvailable(e: io::Error) {
            description("the address is invalid or already in use")
            display("the address is invalid or already in use: {}", e)
            cause(e)
        }
    }
}

quick_error! {
    #[derive(Debug)]
    /// Errors raised when configuring network interfaces.
    pub enum PutUpError {
        /// There is no interface with the given name
        UnknownInterface {
            description("there is no interface with the given name")
        }
        /// Process file descriptor limit hit
        ProcessFileDescriptorLimit(e: io::Error) {
            description("process file descriptor limit hit")
            display("process file descriptor limit hit ({})", e)
            cause(e)
        }
        /// System file descriptor limit hit
        SystemFileDescriptorLimit(e: io::Error) {
            description("system file descriptor limit hit")
            display("system file descriptor limit hit ({})", e)
            cause(e)
        }
        /// Permission denied
        PermissionDenied(e: io::Error) {
            description("permission denied")
            display("permission denied: {}", e)
            cause(e)
        }
    }
}

enum GetSocketError {
    ProcessFileDescriptorLimit(io::Error),
    SystemFileDescriptorLimit(io::Error),
}

struct UnknownInterface;

fn get_req(iface_name: &str) -> Result<sys::ifreq, UnknownInterface> {
    if iface_name.len() > libc::IF_NAMESIZE as usize {
        return Err(UnknownInterface);
    }
    unsafe {
        let mut req: sys::ifreq = mem::zeroed();
        ptr::copy_nonoverlapping(
            iface_name.as_ptr(),
            req.ifr_ifrn.ifrn_name.as_mut_ptr() as *mut _,
            iface_name.as_bytes().len(),
        );
        Ok(req)
    }
}

fn get_socket() -> Result<c_int, GetSocketError> {
    unsafe {
        let fd = libc::socket(libc::AF_INET as i32, libc::SOCK_DGRAM as i32, 0);
        if fd < 0 {
            let os_err = io::Error::last_os_error();
            match sys::errno() {
                libc::EMFILE => return Err(GetSocketError::ProcessFileDescriptorLimit(os_err)),
                libc::ENFILE => return Err(GetSocketError::SystemFileDescriptorLimit(os_err)),
                _ => {
                    panic!("unexpected error when creating dummy socket: {}", os_err);
                },
            }
        }
        Ok(fd)
    }
}

/// Set an interface MAC address
pub fn set_mac_addr(iface_name: &str, mac_addr: MacAddr) -> Result<(), SetMacAddrError> {
    unsafe {
        let mut req = match get_req(iface_name) {
            Ok(req) => req,
            Err(UnknownInterface) => return Err(SetMacAddrError::UnknownInterface),
        };
        let fd = match get_socket() {
            Ok(fd) => fd,
            Err(GetSocketError::ProcessFileDescriptorLimit(e))
                => return Err(SetMacAddrError::ProcessFileDescriptorLimit(e)),
            Err(GetSocketError::SystemFileDescriptorLimit(e))
                => return Err(SetMacAddrError::SystemFileDescriptorLimit(e)),
        };

        let mac_addr = slice::from_raw_parts(
            mac_addr.as_bytes().as_ptr() as *const _,
            mac_addr.as_bytes().len(),
        );
        {
            let addr = &mut req.ifr_ifru.ifru_hwaddr;
            let addr = addr as *mut libc::sockaddr;
            let addr = &mut *addr;
            addr.sa_family = sys::ARPHRD_ETHER as u16;
            addr.sa_data[0..6].clone_from_slice(mac_addr);
        }

        if ioctl::siocsifhwaddr(fd, &req) < 0 {
            let _ = libc::close(fd);
            let os_err = io::Error::last_os_error();
            match sys::errno() {
                libc::ENODEV => return Err(SetMacAddrError::UnknownInterface),
                libc::EPERM => return Err(SetMacAddrError::PermissionDenied(os_err)),
                libc::EADDRNOTAVAIL => return Err(SetMacAddrError::AddrNotAvailable(os_err)),
                _ => {
                    panic!("unexpected error from SIOCSIFHWADDR ioctl: {}", os_err);
                }
            }
        }

        let _ = libc::close(fd);
    }

    Ok(())
}

/// Get an interface MAC address
pub fn get_mac_addr(iface_name: &str) -> Result<MacAddr, GetMacAddrError> {
    unsafe {
        let mut req = match get_req(iface_name) {
            Ok(req) => req,
            Err(UnknownInterface) => return Err(GetMacAddrError::UnknownInterface),
        };
        let fd = match get_socket() {
            Ok(fd) => fd,
            Err(GetSocketError::ProcessFileDescriptorLimit(e))
                => return Err(GetMacAddrError::ProcessFileDescriptorLimit(e)),
            Err(GetSocketError::SystemFileDescriptorLimit(e))
                => return Err(GetMacAddrError::SystemFileDescriptorLimit(e)),
        };

        if ioctl::siocgifhwaddr(fd, &mut req) < 0 {
            let _ = libc::close(fd);
            let os_err = io::Error::last_os_error();
            match sys::errno() {
                libc::ENODEV => return Err(GetMacAddrError::UnknownInterface),
                _ => {
                    panic!("unexpected error from SIOCGIFHWADDR ioctl: {}", os_err);
                }
            }
        }

        let mac_addr = {
            let addr = &mut req.ifr_ifru.ifru_hwaddr;
            let addr = addr as *mut libc::sockaddr;
            let addr = &mut *addr;
            assert_eq!(addr.sa_family, sys::ARPHRD_ETHER as u16);
            let mac_addr = &addr.sa_data[0..6];
            let mac_addr = slice::from_raw_parts(
                mac_addr.as_ptr() as *const _,
                mac_addr.len(),
            );
            MacAddr::from_bytes(mac_addr)
        };

        let _ = libc::close(fd);

        Ok(mac_addr)
    }
}

/// Set an interface IPv4 address and netmask
pub fn set_ipv4_addr(
    iface_name: &str,
    ipv4_addr: Ipv4Addr,
    netmask_bits: u8,
) -> Result<(), SetIpv4AddrError> {
    let netmask = Ipv4Addr::from_netmask_bits(netmask_bits);
    unsafe {
        let mut req = match get_req(iface_name) {
            Ok(req) => req,
            Err(UnknownInterface) => return Err(SetIpv4AddrError::UnknownInterface),
        };
        let fd = match get_socket() {
            Ok(fd) => fd,
            Err(GetSocketError::ProcessFileDescriptorLimit(e))
                => return Err(SetIpv4AddrError::ProcessFileDescriptorLimit(e)),
            Err(GetSocketError::SystemFileDescriptorLimit(e))
                => return Err(SetIpv4AddrError::SystemFileDescriptorLimit(e)),
        };

        #[cfg_attr(feature="cargo-clippy", allow(cast_ptr_alignment))]
        {
            let addr = &mut req.ifr_ifru.ifru_addr;
            let addr = addr as *mut libc::sockaddr;
            let addr = addr as *mut libc::sockaddr_in;
            let addr = &mut *addr;
            addr.sin_family = libc::AF_INET as libc::sa_family_t;
            addr.sin_port = 0;
            addr.sin_addr.s_addr = u32::from(ipv4_addr).to_be();
        }

        if ioctl::siocsifaddr(fd, &req) < 0 {
            let _ = libc::close(fd);
            // TODO: what errors occur if we
            //  (a) pick an invalid IP.
            //  (b) pick an IP already in use
            let os_err = io::Error::last_os_error();
            match sys::errno() {
                libc::ENODEV => return Err(SetIpv4AddrError::UnknownInterface),
                libc::EPERM => return Err(SetIpv4AddrError::PermissionDenied(os_err)),
                libc::EADDRNOTAVAIL => return Err(SetIpv4AddrError::AddrNotAvailable(os_err)),
                _ => {
                    panic!("unexpected error from SIOCSIFADDR ioctl: {}", os_err);
                },
            }
        }

        #[cfg_attr(feature="cargo-clippy", allow(cast_ptr_alignment))]
        {
            let addr = &mut req.ifr_ifru.ifru_addr;
            let addr = addr as *mut libc::sockaddr;
            let addr = addr as *mut libc::sockaddr_in;
            let addr = &mut *addr;
            addr.sin_family = libc::AF_INET as libc::sa_family_t;
            addr.sin_port = 0;
            addr.sin_addr.s_addr = u32::from(netmask).to_be();
        }

        if ioctl::siocsifnetmask(fd, &req) < 0 {
            let _ = libc::close(fd);
            let os_err = io::Error::last_os_error();
            match sys::errno() {
                libc::ENODEV => return Err(SetIpv4AddrError::UnknownInterface),
                libc::EADDRNOTAVAIL => return Err(SetIpv4AddrError::AddrNotAvailable(os_err)),
                _ => {
                    panic!("unexpected error from SIOCSIFNETMASK ioctl: {}", os_err);
                },
            }
        }
        let _ = libc::close(fd);
    }

    Ok(())
}

/// Set an interface IPv6 address and prefixlen
pub fn set_ipv6_addr(
    iface_name: &str,
    ipv6_addr: Ipv6Addr,
    netmask_bits: u8,
) -> Result<(), SetIpv6AddrError> {
    unsafe {
        let mut req = match get_req(iface_name) {
            Ok(req) => req,
            Err(UnknownInterface) => return Err(SetIpv6AddrError::UnknownInterface),
        };
        let fd = match get_socket() {
            Ok(fd) => fd,
            Err(GetSocketError::ProcessFileDescriptorLimit(e))
                => return Err(SetIpv6AddrError::ProcessFileDescriptorLimit(e)),
            Err(GetSocketError::SystemFileDescriptorLimit(e))
                => return Err(SetIpv6AddrError::SystemFileDescriptorLimit(e)),
        };

        if ioctl::siocgifindex(fd, &mut req) < 0 {
            let _ = libc::close(fd);
            let os_err = io::Error::last_os_error();
            match sys::errno() {
                libc::ENODEV => return Err(SetIpv6AddrError::UnknownInterface),
                _ => {
                    panic!("unexpected error from SIOGIFINDEX ioctl: {}", os_err);
                },
            };
        }
        let index = req.ifr_ifru.ifru_ivalue as u32;

        let netlink = libc::socket(libc::AF_NETLINK as i32, libc::SOCK_RAW, libc::NETLINK_ROUTE as i32);
        if netlink < 0 {
            let os_err = io::Error::last_os_error();
            match sys::errno() {
                libc::EMFILE => return Err(SetIpv6AddrError::ProcessFileDescriptorLimit(os_err)),
                libc::ENFILE => return Err(SetIpv6AddrError::SystemFileDescriptorLimit(os_err)),
                _ => {
                    panic!("unexpected error when creating netlink socket: {}", os_err);
                },
            }
        }

        let round_up = |x| (x + sys::NLMSG_ALIGNTO as usize - 1) & !(sys::NLMSG_ALIGNTO as usize - 1);

        let header_start = 0;
        let header_end = header_start + mem::size_of::<libc::nlmsghdr>();
        let data_start = round_up(header_end);
        let data_end = data_start + mem::size_of::<sys::ifaddrmsg>();
        let attr_header_start = round_up(data_end);
        let attr_header_end = attr_header_start + mem::size_of::<sys::rtattr>();
        let attr_data_start = round_up(attr_header_end);
        let attr_data_end = attr_data_start + mem::size_of::<Ipv6Addr>();
        let total_size = attr_data_end;

        let mut buffer: Vec<u8> = Vec::with_capacity(total_size);
        {
            let nlmsghdr: *mut libc::nlmsghdr = {
                #[cfg_attr(feature="cargo-clippy", allow(cast_ptr_alignment))]
                { buffer.as_mut_ptr() as *mut _ }
            };
            let nlmsghdr: &mut libc::nlmsghdr = &mut *nlmsghdr;

            nlmsghdr.nlmsg_len = total_size as u32;
            nlmsghdr.nlmsg_type = sys::RTM_NEWADDR as u16;
            nlmsghdr.nlmsg_flags = {
                libc::NLM_F_REPLACE |
                libc::NLM_F_CREATE |
                libc::NLM_F_REQUEST |    // TODO: do I need this one?
                libc::NLM_F_ACK
            } as u16;
            nlmsghdr.nlmsg_seq = 0;
            nlmsghdr.nlmsg_pid = 0;
        }

        {
            let ifaddrmsg: *mut sys::ifaddrmsg = {
                #[cfg_attr(feature="cargo-clippy", allow(cast_ptr_alignment))]
                { buffer.as_mut_ptr().offset(data_start as isize) as *mut _ }
            };
            let ifaddrmsg: &mut sys::ifaddrmsg = &mut *ifaddrmsg;
            ifaddrmsg.ifa_family = libc::AF_INET6 as u8;
            ifaddrmsg.ifa_prefixlen = netmask_bits;
            ifaddrmsg.ifa_flags = 0;    // TODO: what is IFA_F_PERMANENT here?
            ifaddrmsg.ifa_scope = 0;
            ifaddrmsg.ifa_index = index;
        }

        {
            let rtattr: *mut sys::rtattr = {
                #[cfg_attr(feature="cargo-clippy", allow(cast_ptr_alignment))]
                { buffer.as_mut_ptr().offset(attr_header_start as isize) as *mut _ }
            };
            let rtattr: &mut sys::rtattr = &mut *rtattr;
            rtattr.rta_len = (attr_data_end - attr_header_start) as u16;
            rtattr.rta_type = sys::IFA_ADDRESS as u16;
        }

        {
            let addr: *mut [u8; 16] = {
                buffer.as_mut_ptr().offset(attr_data_start as isize) as *mut _
            };
            let addr: &mut [u8; 16] = &mut *addr;
            addr.clone_from_slice(&ipv6_addr.octets());
        }

        let n = libc::write(netlink, buffer.as_ptr() as *const _, total_size);
        if n < 0 {
            panic!("unexpected error writing to netlink socket: {}", io::Error::last_os_error());
        }
        assert_eq!(n as usize, total_size);

        let header_start = 0;
        let header_end = header_start + mem::size_of::<libc::nlmsghdr>();
        let error_start = round_up(header_end);
        let error_end = error_start + mem::size_of::<libc::nlmsgerr>();
        let total_size = error_end;

        let mut buffer: Vec<u8> = Vec::with_capacity(total_size);
        loop {
            let n = libc::read(netlink, buffer.as_mut_ptr() as *mut _, total_size);
            if n < 0 {
                panic!(
                    "unexpected error reading from netlink socket: {}",
                    io::Error::last_os_error(),
                );
            }
            assert!(n as usize >= header_end);

            {
                let nlmsghdr: *const libc::nlmsghdr = {
                    #[cfg_attr(feature="cargo-clippy", allow(cast_ptr_alignment))]
                    { buffer.as_ptr() as *const _ }
                };
                let nlmsghdr: &libc::nlmsghdr = &*nlmsghdr;
                if nlmsghdr.nlmsg_type == libc::NLMSG_NOOP as u16 {
                    continue;
                }
                assert_eq!(n as usize, total_size);
                assert_eq!(nlmsghdr.nlmsg_type, libc::NLMSG_ERROR as u16);
            }

            {
                let nlmsgerr: *const libc::nlmsgerr = {
                    #[cfg_attr(feature="cargo-clippy", allow(cast_ptr_alignment))]
                    { buffer.as_ptr().offset(error_start as isize) as *const _ }
                };
                let nlmsgerr: &libc::nlmsgerr = &*nlmsgerr;
                if nlmsgerr.error != 0 {
                    let os_err = io::Error::from_raw_os_error(-nlmsgerr.error);
                    match -nlmsgerr.error {
                        libc::EPERM
                            => return Err(SetIpv6AddrError::PermissionDenied(os_err)),
                        libc::EADDRNOTAVAIL
                            => return Err(SetIpv6AddrError::AddrNotAvailable(os_err)),
                        _ => {
                            panic!(
                                "unexpected error from netlink when setting IPv6 address: {}",
                                os_err,
                            );
                        }
                    }
                }
            }

            break;
        }

        let _ = libc::close(netlink);
        let _ = libc::close(fd);
    }

    Ok(())
}

/// Put an interface up.
pub fn put_up(iface_name: &str) -> Result<(), PutUpError> {
    unsafe {
        let mut req = match get_req(iface_name) {
            Ok(req) => req,
            Err(UnknownInterface) => return Err(PutUpError::UnknownInterface),
        };
        let fd = match get_socket() {
            Ok(fd) => fd,
            Err(GetSocketError::ProcessFileDescriptorLimit(e))
                => return Err(PutUpError::ProcessFileDescriptorLimit(e)),
            Err(GetSocketError::SystemFileDescriptorLimit(e))
                => return Err(PutUpError::SystemFileDescriptorLimit(e)),
        };

        if ioctl::siocgifflags(fd, &mut req) < 0 {
            let _ = libc::close(fd);
            let os_err = io::Error::last_os_error();
            match sys::errno() {
                libc::ENODEV => return Err(PutUpError::UnknownInterface),
                _ => {
                    panic!("unexpected error from SIOCGIFFLAGS ioctl: {}", os_err);
                },
            };
        }

        req.ifr_ifru.ifru_flags |= (libc::IFF_UP as u32 | libc::IFF_RUNNING as u32) as i16;

        if ioctl::siocsifflags(fd, &req) < 0 {
            let _ = libc::close(fd);
            let os_err = io::Error::last_os_error();
            match sys::errno() {
                libc::ENODEV => return Err(PutUpError::UnknownInterface),
                libc::EPERM => return Err(PutUpError::PermissionDenied(os_err)),
                _ => {
                    panic!("unexpected error from SIOCSIFFLAGS ioctl: {}", os_err);
                },
            }
        }
        let _ = libc::close(fd);
    }

    Ok(())
}

#[cfg(feature = "linux_host")]
#[cfg(test)]
mod test {
    use super::*;
    use rand;
    use spawn;
    use capabilities;

    #[test]
    fn configure_tap() {
        run_test(1, || {
            let spawn_complete = spawn::new_namespace(|| {
                let name = format!("foo{:x}", rand::random::<u32>());

                match set_mac_addr(&name, MacAddr::random()) {
                    Err(SetMacAddrError::UnknownInterface) => (),
                    res => panic!("unexpected result: {:?}", res),
                };
                match get_mac_addr(&name) {
                    Err(GetMacAddrError::UnknownInterface) => (),
                    res => panic!("unexpected result: {:?}", res),
                };
                match set_ipv4_addr(&name, Ipv4Addr::random_global(), 3) {
                    Err(SetIpv4AddrError::UnknownInterface) => (),
                    res => panic!("unexpected result: {:?}", res),
                };
                match set_ipv6_addr(&name, Ipv6Addr::random_global(), 3) {
                    Err(SetIpv6AddrError::UnknownInterface) => (),
                    res => panic!("unexpected result: {:?}", res),
                };

                let tap_builder = {
                    EtherIfaceBuilder::new()
                    .ipv4_addr(Ipv4Addr::random_global(), 0)
                    .ipv6_addr(Ipv6Addr::random_global(), 0)
                    .name(name.clone())
                };
                let tap = unwrap!(tap_builder.build_unbound());

                let mac_addr_0 = MacAddr::random();
                unwrap!(set_mac_addr(&name, mac_addr_0));
                let mac_addr_1 = unwrap!(get_mac_addr(&name));
                assert_eq!(mac_addr_0, mac_addr_1);

                unwrap!(set_ipv4_addr(&name, Ipv4Addr::random_global(), 3));
                unwrap!(set_ipv6_addr(&name, Ipv6Addr::random_global(), 3));

                match set_mac_addr(&name, MacAddr::from_bytes(&[0, 0, 0, 0, 0, 0])) {
                    Err(SetMacAddrError::AddrNotAvailable(..)) => (),
                    res => panic!("unexpected result: {:?}", res),
                };
                match set_ipv4_addr(&name, ipv4!("0.0.0.0"), 0) {
                    Err(SetIpv4AddrError::AddrNotAvailable(..)) => (),
                    res => panic!("unexpected result: {:?}", res),
                };
                match set_ipv6_addr(&name, ipv6!("::"), 0) {
                    Err(SetIpv6AddrError::AddrNotAvailable(..)) => (),
                    res => panic!("unexpected result: {:?}", res),
                };

                unwrap!(unwrap!(capabilities::Capabilities::new()).apply());

                match set_mac_addr(&name, MacAddr::random()) {
                    Err(SetMacAddrError::PermissionDenied(..)) => (),
                    res => panic!("unexpected result: {:?}", res),
                };
                let mac_addr_1 = unwrap!(get_mac_addr(&name));
                assert_eq!(mac_addr_0, mac_addr_1);

                match set_ipv4_addr(&name, Ipv4Addr::random_global(), 3) {
                    Err(SetIpv4AddrError::PermissionDenied(..)) => (),
                    res => panic!("unexpected result: {:?}", res),
                };
                match set_ipv6_addr(&name, Ipv6Addr::random_global(), 3) {
                    Err(SetIpv6AddrError::PermissionDenied(..)) => (),
                    res => panic!("unexpected result: {:?}", res),
                };

                drop(tap);
            });
            let mut core = unwrap!(Core::new());
            unwrap!(core.run(spawn_complete))
        })
    }
}

