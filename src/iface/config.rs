use priv_prelude::*;
use sys;
use ioctl;
use libc;

quick_error! {
    #[derive(Debug)]
    /// Errors raised when configuring network interfaces.
    pub enum IfaceConfigError {
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

fn get_req(iface_name: &str) -> Result<sys::ifreq, IfaceConfigError> {
    if iface_name.len() > sys::IF_NAMESIZE as usize {
        return Err(IfaceConfigError::UnknownInterface)
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

fn get_socket() -> Result<c_int, IfaceConfigError> {
    unsafe {
        let fd = sys::socket(sys::AF_INET as i32, sys::__socket_type::SOCK_DGRAM as i32, 0);
        if fd < 0 {
            let os_err = io::Error::last_os_error();
            match sys::errno() as u32 {
                sys::EMFILE => return Err(IfaceConfigError::ProcessFileDescriptorLimit(os_err)),
                sys::ENFILE => return Err(IfaceConfigError::SystemFileDescriptorLimit(os_err)),
                _ => {
                    panic!("unexpected error when creating dummy socket: {}", os_err);
                },
            }
        }
        Ok(fd)
    }
}

/// Set an interface MAC address
pub fn set_mac_addr(iface_name: &str, mac_addr: MacAddr) -> Result<(), IfaceConfigError> {
    unsafe {
        let mut req = get_req(iface_name)?;
        let fd = get_socket()?;

        let mac_addr = slice::from_raw_parts(
            mac_addr.as_bytes().as_ptr() as *const _,
            mac_addr.as_bytes().len(),
        );
        {
            let addr = &mut req.ifr_ifru.ifru_hwaddr;
            let addr = addr as *mut sys::sockaddr;
            let addr = &mut *addr;
            addr.sa_family = sys::ARPHRD_ETHER as u16;
            addr.sa_data[0..6].clone_from_slice(mac_addr);
        }

        if ioctl::siocsifhwaddr(fd, &req) < 0 {
            let _ = sys::close(fd);
            panic!("unexpected error from SIOCSIFHWADDR ioctl: {}", io::Error::last_os_error());
        }

        let _ = sys::close(fd);
    }

    Ok(())
}

/// Get an interface MAC address
pub fn get_mac_addr(iface_name: &str) -> Result<MacAddr, IfaceConfigError> {
    unsafe {
        let mut req = get_req(iface_name)?;
        let fd = get_socket()?;

        if ioctl::siocgifhwaddr(fd, &mut req) < 0 {
            let _ = sys::close(fd);
            panic!("unexpected error from SIOCGIFHWADDR ioctl: {}", io::Error::last_os_error());
        }

        let mac_addr = {
            let addr = &mut req.ifr_ifru.ifru_hwaddr;
            let addr = addr as *mut sys::sockaddr;
            let addr = &mut *addr;
            assert_eq!(addr.sa_family, sys::ARPHRD_ETHER as u16);
            let mac_addr = &addr.sa_data[0..6];
            let mac_addr = slice::from_raw_parts(
                mac_addr.as_ptr() as *const _,
                mac_addr.len(),
            );
            MacAddr::from_bytes(mac_addr)
        };

        let _ = sys::close(fd);

        Ok(mac_addr)
    }
}

/// Set an interface IPv4 address and netmask
pub fn set_ipv4_addr(
    iface_name: &str,
    ipv4_addr: Ipv4Addr,
    netmask_bits: u8,
) -> Result<(), IfaceConfigError> {
    let netmask = Ipv4Addr::from_netmask_bits(netmask_bits);
    unsafe {
        let mut req = get_req(iface_name)?;
        let fd = get_socket()?;

        {
            let addr = &mut req.ifr_ifru.ifru_addr;
            let addr = addr as *mut sys::sockaddr;
            let addr = addr as *mut sys::sockaddr_in;
            let addr = &mut *addr;
            addr.sin_family = sys::AF_INET as sys::sa_family_t;
            addr.sin_port = 0;
            addr.sin_addr.s_addr = u32::from(ipv4_addr).to_be();
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
            addr.sin_addr.s_addr = u32::from(netmask).to_be();
        }

        if ioctl::siocsifnetmask(fd, &req) < 0 {
            let _ = sys::close(fd);
            // TODO: what error occurs if we try to use an invalid netmask?
            panic!("unexpected error from SIOCSIFNETMASK ioctl: {}", io::Error::last_os_error());
        }
        let _ = sys::close(fd);
    }

    Ok(())
}

/// Set an interface IPv6 address and prefixlen
pub fn set_ipv6_addr(
    iface_name: &str,
    ipv6_addr: Ipv6Addr,
    netmask_bits: u8,
) -> Result<(), IfaceConfigError> {
    unsafe {
        let mut req = get_req(iface_name)?;
        let fd = get_socket()?;

        if ioctl::siocgifindex(fd, &mut req) < 0 {
            let _ = sys::close(fd);
            panic!("unexpected error from SIOGIFINDEX ioctl: {}", io::Error::last_os_error());
        }
        let index = req.ifr_ifru.ifru_ivalue as u32;

        let netlink = sys::socket(sys::AF_NETLINK as i32, libc::SOCK_RAW, sys::NETLINK_ROUTE as i32);
        if netlink < 0 {
            let os_err = io::Error::last_os_error();
            match sys::errno() as u32 {
                sys::EMFILE => return Err(IfaceConfigError::ProcessFileDescriptorLimit(os_err)),
                sys::ENFILE => return Err(IfaceConfigError::SystemFileDescriptorLimit(os_err)),
                _ => {
                    panic!("unexpected error when creating netlink socket: {}", os_err);
                },
            }
        }

        let round_up = |x| (x + sys::NLMSG_ALIGNTO as usize - 1) & !(sys::NLMSG_ALIGNTO as usize - 1);

        let header_start = 0;
        let header_end = header_start + mem::size_of::<sys::nlmsghdr>();
        let data_start = round_up(header_end);
        let data_end = data_start + mem::size_of::<sys::ifaddrmsg>();
        let attr_header_start = round_up(data_end);
        let attr_header_end = attr_header_start + mem::size_of::<sys::rtattr>();
        let attr_data_start = round_up(attr_header_end);
        let attr_data_end = attr_data_start + mem::size_of::<Ipv6Addr>();
        let total_size = attr_data_end;

        let mut buffer: Vec<u8> = Vec::with_capacity(total_size);
        {
            let nlmsghdr: *mut sys::nlmsghdr = buffer.as_mut_ptr() as *mut _;
            let nlmsghdr: &mut sys::nlmsghdr = &mut *nlmsghdr;

            nlmsghdr.nlmsg_len = total_size as u32;
            nlmsghdr.nlmsg_type = sys::RTM_NEWADDR as u16;
            nlmsghdr.nlmsg_flags = {
                sys::NLM_F_REPLACE |
                sys::NLM_F_CREATE |
                sys::NLM_F_REQUEST |    // TODO: do I need this one?
                sys::NLM_F_ACK
            } as u16;
            nlmsghdr.nlmsg_seq = 0;
            nlmsghdr.nlmsg_pid = 0;
        }

        {
            let ifaddrmsg: *mut sys::ifaddrmsg = {
                buffer.as_mut_ptr().offset(data_start as isize) as *mut _
            };
            let ifaddrmsg: &mut sys::ifaddrmsg = &mut *ifaddrmsg;
            ifaddrmsg.ifa_family = sys::AF_INET6 as u8;
            ifaddrmsg.ifa_prefixlen = netmask_bits;
            ifaddrmsg.ifa_flags = 0;    // TODO: what is IFA_F_PERMANENT here?
            ifaddrmsg.ifa_scope = 0;
            ifaddrmsg.ifa_index = index;
        }

        {
            let rtattr: *mut sys::rtattr = {
                buffer.as_mut_ptr().offset(attr_header_start as isize) as *mut _
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

        let n = sys::write(netlink, buffer.as_ptr() as *const _, total_size);
        if n < 0 {
            panic!("unexpected error writing to netlink socket: {}", io::Error::last_os_error());
        }
        assert_eq!(n as usize, total_size);

        let header_start = 0;
        let header_end = header_start + mem::size_of::<sys::nlmsghdr>();
        let error_start = round_up(header_end);
        let error_end = error_start + mem::size_of::<sys::nlmsgerr>();
        let total_size = error_end;

        let mut buffer: Vec<u8> = Vec::with_capacity(total_size);
        loop {
            let n = sys::read(netlink, buffer.as_mut_ptr() as *mut _, total_size);
            if n < 0 {
                panic!(
                    "unexpected error reading from netlink socket: {}",
                    io::Error::last_os_error(),
                );
            }
            assert!(n as usize >= header_end);

            {
                let nlmsghdr: *const sys::nlmsghdr = buffer.as_ptr() as *const _;
                let nlmsghdr: &sys::nlmsghdr = &*nlmsghdr;
                if nlmsghdr.nlmsg_type == sys::NLMSG_NOOP as u16 {
                    continue;
                }
                assert_eq!(n as usize, total_size);
                assert_eq!(nlmsghdr.nlmsg_type, sys::NLMSG_ERROR as u16);
            }

            {
                let nlmsgerr: *const sys::nlmsgerr = {
                    buffer.as_ptr().offset(error_start as isize) as *const _
                };
                let nlmsgerr: &sys::nlmsgerr = &*nlmsgerr;
                assert_eq!(nlmsgerr.error, 0);
            }

            break;
        }

        let _ = sys::close(netlink);
        let _ = sys::close(fd);
    }

    Ok(())
}

/// Put an interface up.
pub fn put_up(iface_name: &str) -> Result<(), IfaceConfigError> {
    unsafe {
        let mut req = get_req(iface_name)?;
        let fd = get_socket()?;

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

    Ok(())
}

