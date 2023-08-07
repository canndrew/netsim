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
    unsafe {
        let mut req = new_req(iface_name);
        let fd = new_socket()?;

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

        if ioctl::siocsifaddr(fd.as_raw_fd(), &req) < 0 {
            return Err(io::Error::last_os_error());
        }

        #[cfg_attr(feature="cargo-clippy", allow(cast_ptr_alignment))]
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

