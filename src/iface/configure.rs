use crate::priv_prelude::*;

pub(crate) fn put_up(iface_name: &str) -> io::Result<()> {
    unsafe {
        let mut req = new_req(iface_name);
        let fd = new_socket()?;
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
    let fd = new_socket()?;

    unsafe {
        #[cfg_attr(feature="cargo-clippy", allow(clippy::cast_ptr_alignment))]
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

        #[cfg_attr(feature="cargo-clippy", allow(clippy::cast_ptr_alignment))]
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

#[cfg_attr(feature="cargo-clippy", allow(clippy::cast_ptr_alignment, clippy::unnecessary_cast))]
pub(crate) fn add_ipv4_route(
    iface_name: &str,
    destination: Ipv4Network,
    gateway_opt: Option<Ipv4Addr>,
) -> io::Result<()> {
    let fd = new_socket()?;

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

    route.rt_flags = libc::RTF_UP as u16;
    if let Some(gateway_addr) = gateway_opt {
        unsafe {
            let route_gateway = &mut route.rt_gateway as *mut _ as *mut libc::sockaddr_in;
            (*route_gateway).sin_family = libc::AF_INET as u16;
            (*route_gateway).sin_addr = libc::in_addr { s_addr: u32::from(gateway_addr).to_be() };
        };
    
        route.rt_flags |= libc::RTF_GATEWAY as u16;
    }

    let c_iface_name = CString::new(iface_name).unwrap();

    // TODO: This doesn't *actually* need to mutable yeah?
    route.rt_dev = c_iface_name.as_ptr() as *mut _;

    let res = unsafe {
        libc::ioctl(fd.as_raw_fd(), libc::SIOCADDRT as u64, &route)
    };
    if res < 0 {
        return Err(io::Error::last_os_error());
    }

    Ok(())
}

#[cfg_attr(feature="cargo-clippy", allow(clippy::cast_ptr_alignment, clippy::unnecessary_cast))]
fn new_req(iface_name: &str) -> libc::ifreq {
    if iface_name.len() >= libc::IF_NAMESIZE as usize {
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

#[cfg_attr(feature="cargo-clippy", allow(clippy::cast_ptr_alignment, clippy::unnecessary_cast))]
fn new_socket() -> io::Result<OwnedFd> {
    let raw_fd = unsafe {
        libc::socket(libc::AF_INET as i32, libc::SOCK_DGRAM as i32, 0)
    };
    if raw_fd < 0 {
        return Err(io::Error::last_os_error());
    }
    let fd = unsafe { OwnedFd::from_raw_fd(raw_fd) };
    Ok(fd)
}

