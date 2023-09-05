use crate::priv_prelude::*;

pub(crate) fn put_up(iface_name: &str) -> io::Result<()> {
    unsafe {
        let mut req = new_req(iface_name);
        let fd = socket(libc::AF_INET, libc::SOCK_DGRAM, 0)?;
        if ioctl::siocgifflags(fd.as_raw_fd(), &mut req) < 0 {
            return Err(io::Error::last_os_error());
        }

        req.ifr_ifru.ifru_flags |= (libc::IFF_UP as u32 | libc::IFF_RUNNING as u32) as i16;

        if ioctl::siocsifflags(fd.as_raw_fd(), &req) < 0 {
            return Err(io::Error::last_os_error());
        }
    }

    Ok(())
}

pub(crate) fn set_ipv4_addr(
    iface_name: &str,
    ipv4_addr: Ipv4Addr,
    subnet_mask_bits: u8,
) -> io::Result<()> {
    let mask = if subnet_mask_bits == 32 { !0u32 } else { !(!0 >> subnet_mask_bits) };
    let mut req = new_req(iface_name);
    let fd = socket(libc::AF_INET, libc::SOCK_DGRAM, 0)?;

    unsafe {
        {
            let addr = &mut req.ifr_ifru.ifru_addr;
            let addr = addr as *mut libc::sockaddr;
            let addr = addr as *mut libc::sockaddr_in;
            let addr = &mut *addr;
            addr.sin_family = libc::AF_INET as libc::sa_family_t;
            addr.sin_port = 0;
            addr.sin_addr.s_addr = u32::from(ipv4_addr).to_be();
        }

        if ioctl::siocsifaddr(fd.as_raw_fd(), &req) < 0 {
            return Err(io::Error::last_os_error());
        }

        {
            let addr = &mut req.ifr_ifru.ifru_addr;
            let addr = addr as *mut libc::sockaddr;
            let addr = addr as *mut libc::sockaddr_in;
            let addr = &mut *addr;
            addr.sin_family = libc::AF_INET as libc::sa_family_t;
            addr.sin_port = 0;
            addr.sin_addr.s_addr = mask.to_be();
        }

        if ioctl::siocsifnetmask(fd.as_raw_fd(), &req) < 0 {
            return Err(io::Error::last_os_error());
        }
    }

    Ok(())
}

fn round_up(
    x: usize,
    multiple: usize,
) -> usize {
    match x % multiple {
        0 => x,
        r => x + (multiple - r),
    }
}

pub(crate) fn set_ipv6_addr(
    iface_name: &str,
    ipv6_addr: Ipv6Addr,
    subnet_mask_bits: u8,
) -> io::Result<()> {
    let mut req = new_req(iface_name);
    let fd = socket(libc::AF_INET, libc::SOCK_DGRAM, 0)?;

    unsafe {
        if ioctl::siocgifindex(fd.as_raw_fd(), &mut req) < 0 {
            return Err(io::Error::last_os_error());
        }
    }

    let index = unsafe {
        req.ifr_ifru.ifru_ifindex as u32
    };

    let netlink = socket(libc::AF_NETLINK, libc::SOCK_RAW, libc::NETLINK_ROUTE)?;

    let header_start = 0;
    let header_end = header_start + mem::size_of::<libc::nlmsghdr>();
    let data_start = round_up(header_end, sys::NLMSG_ALIGNTO);
    let data_end = data_start + mem::size_of::<sys::ifaddrmsg>();
    let attr_header_start = round_up(data_end, sys::NLMSG_ALIGNTO);
    let attr_header_end = attr_header_start + mem::size_of::<sys::rtattr>();
    let attr_data_start = round_up(attr_header_end, sys::NLMSG_ALIGNTO);
    let attr_data_end = attr_data_start + mem::size_of::<[u8; 16]>();
    let total_size = attr_data_end;

    let mut buffer: Vec<u8> = Vec::with_capacity(total_size);
    let nlmsghdr = libc::nlmsghdr {
        nlmsg_len: total_size as u32,
        nlmsg_type: libc::RTM_NEWADDR,
        nlmsg_flags: {
            libc::NLM_F_REPLACE |
            libc::NLM_F_CREATE |
            libc::NLM_F_REQUEST |    // TODO: do I need this one?
            libc::NLM_F_ACK
        } as u16,
        nlmsg_seq: 0,
        nlmsg_pid: 0,
    };
    unsafe {
        ptr::write(
            buffer.as_mut_ptr() as *mut _,
            nlmsghdr,
        )
    }

    let ifaddrmsg = sys::ifaddrmsg {
        ifa_family: libc::AF_INET6 as u8,
        ifa_prefixlen: subnet_mask_bits,
        ifa_flags: 0,    // TODO: use IFA_F_PERMANENT here?
        ifa_scope: 0,
        ifa_index: index,
    };
    unsafe {
        ptr::write(
            buffer.as_mut_ptr().add(data_start) as *mut _,
            ifaddrmsg,
        )
    }

    let rtattr = sys::rtattr {
        rta_len: (attr_data_end - attr_header_start) as u16,
        rta_type: libc::IFA_ADDRESS,
    };
    unsafe {
        ptr::write(
            buffer.as_mut_ptr().add(attr_header_start) as *mut _,
            rtattr,
        )
    }

    let addr = ipv6_addr.octets();
    unsafe {
        ptr::write(
            buffer.as_mut_ptr().add(attr_data_start) as *mut _,
            addr,
        )
    }

    let n = unsafe {
        libc::write(netlink.as_raw_fd(), buffer.as_ptr() as *const _, total_size)
    };
    if n < 0 {
        return Err(io::Error::last_os_error());
    }
    assert_eq!(n as usize, total_size);


    let header_start = 0;
    let header_end = header_start + mem::size_of::<libc::nlmsghdr>();
    let error_start = round_up(header_end, sys::NLMSG_ALIGNTO);
    let error_end = error_start + mem::size_of::<libc::nlmsgerr>();
    let total_size = error_end;

    let mut buffer: Vec<u8> = Vec::with_capacity(total_size);
    loop {
        let n = unsafe {
            libc::read(netlink.as_raw_fd(), buffer.as_mut_ptr() as *mut _, total_size)
        };
        if n < 0 {
            return Err(io::Error::last_os_error());
        }
        assert!(n as usize >= header_end);
        let nlmsghdr: *const libc::nlmsghdr = buffer.as_ptr() as *const _;
        let nlmsghdr: &libc::nlmsghdr = unsafe { &*nlmsghdr };
        if nlmsghdr.nlmsg_type == libc::NLMSG_NOOP as u16 {
            continue;
        }
        assert_eq!(nlmsghdr.nlmsg_type, libc::NLMSG_ERROR as u16);
        assert_eq!(n as usize, total_size);

        let nlmsgerr: *const libc::nlmsgerr = unsafe {
            buffer.as_ptr().add(error_start) as *const _
        };
        let nlmsgerr: &libc::nlmsgerr = unsafe { &*nlmsgerr };
        if nlmsgerr.error != 0 {
            return Err(io::Error::from_raw_os_error(-nlmsgerr.error));
        }

        break;
    }
    Ok(())
}

pub(crate) fn add_ipv4_route(
    iface_name: &str,
    destination: Ipv4Network,
    gateway_opt: Option<Ipv4Addr>,
) -> io::Result<()> {
    let fd = socket(libc::AF_INET, libc::SOCK_DGRAM, 0)?;

    let mut route: libc::rtentry = unsafe {
        mem::zeroed()
    };

    unsafe {
        let route_destination = &mut route.rt_dst as *mut _ as *mut libc::sockaddr_in;
        (*route_destination).sin_family = libc::AF_INET as u16;
        (*route_destination).sin_addr = libc::in_addr { s_addr: u32::from(destination.base_addr()).to_be() };
    };

    let netmask = Ipv4Addr::from(!((!0u32).checked_shr(u32::from(destination.subnet_mask_bits())).unwrap_or(0)));
    unsafe {
        let route_genmask = &mut route.rt_genmask as *mut _ as *mut libc::sockaddr_in;
        (*route_genmask).sin_family = libc::AF_INET as u16;
        (*route_genmask).sin_addr = libc::in_addr { s_addr: u32::from(netmask).to_be() };
    };

    route.rt_flags = libc::RTF_UP;
    if let Some(gateway_addr) = gateway_opt {
        unsafe {
            let route_gateway = &mut route.rt_gateway as *mut _ as *mut libc::sockaddr_in;
            (*route_gateway).sin_family = libc::AF_INET as u16;
            (*route_gateway).sin_addr = libc::in_addr { s_addr: u32::from(gateway_addr).to_be() };
        };
    
        route.rt_flags |= libc::RTF_GATEWAY;
    }

    let c_iface_name = CString::new(iface_name).unwrap();

    // TODO: This doesn't *actually* need to mutable yeah?
    route.rt_dev = c_iface_name.as_ptr() as *mut _;

    let res = unsafe {
        libc::ioctl(fd.as_raw_fd(), libc::SIOCADDRT, &route)
    };
    if res < 0 {
        return Err(io::Error::last_os_error());
    }

    Ok(())
}

pub(crate) fn add_ipv6_route(
    iface_name: &str,
    destination: Ipv6Network,
    next_hop: Ipv6Addr,
) -> io::Result<()> {
    let fd = socket(libc::PF_INET6, libc::SOCK_DGRAM, libc::IPPROTO_IP)?;
    let mut route: libc::rtentry = unsafe {
        mem::zeroed()
    };

    fn as_sockaddr_in6(ptr: &mut libc::sockaddr) -> &mut libc::sockaddr_in6 {
        unsafe { mem::transmute(ptr) }
    }

    let rt_dst = as_sockaddr_in6(&mut route.rt_dst);
    rt_dst.sin6_family = libc::AF_INET6 as u16;
    rt_dst.sin6_addr = libc::in6_addr {
        s6_addr: destination.base_addr().octets(),
    };

    let netmask = Ipv6Addr::from(!((!0u128).checked_shr(u32::from(destination.subnet_mask_bits())).unwrap_or(0)));
    let rt_genmask = as_sockaddr_in6(&mut route.rt_genmask);
    rt_genmask.sin6_family = libc::AF_INET6 as u16;
    rt_genmask.sin6_addr = libc::in6_addr {
        s6_addr: netmask.octets(),
    };

    route.rt_flags = libc::RTF_UP;
    let rt_gateway = as_sockaddr_in6(&mut route.rt_gateway);
    rt_gateway.sin6_family = libc::AF_INET6 as u16;
    rt_gateway.sin6_addr = libc::in6_addr {
        s6_addr: next_hop.octets(),
    };

    let c_iface_name = CString::new(iface_name).unwrap();

    // TODO: This doesn't *actually* need to mutable yeah?
    route.rt_dev = c_iface_name.as_ptr() as *mut _;

    let res = unsafe {
        libc::ioctl(fd.as_raw_fd(), libc::SIOCADDRT, &route)
    };
    if res < 0 {
        return Err(io::Error::last_os_error());
    }

    Ok(())
}


fn new_req(iface_name: &str) -> libc::ifreq {
    if iface_name.len() >= libc::IF_NAMESIZE {
        panic!("interface name too long");
    }
    unsafe {
        let mut req: libc::ifreq = mem::zeroed();
        ptr::copy_nonoverlapping(
            iface_name.as_ptr(),
            req.ifr_name.as_mut_ptr() as *mut _,
            iface_name.as_bytes().len(),
        );
        req
    }
}

fn socket(domain: libc::c_int, ty: libc::c_int, protocol: libc::c_int) -> io::Result<OwnedFd> {
    let raw_fd = unsafe {
        libc::socket(domain, ty, protocol)
    };
    if raw_fd < 0 {
        return Err(io::Error::last_os_error());
    }
    let fd = unsafe { OwnedFd::from_raw_fd(raw_fd) };
    Ok(fd)
}

