use priv_prelude::*;
use sys;
use libc;

/// Represents an IPv6 route.
#[derive(Debug, Clone, Copy)]
pub struct Ipv6Route {
    destination: Ipv6Range,
    next_hop: Ipv6Addr,
}

impl Ipv6Route {
    /// Create a new route with the given destination and next hop.
    pub fn new(destination: Ipv6Range, next_hop: Ipv6Addr) -> Ipv6Route {
        Ipv6Route {
            destination,
            next_hop,
        }
    }

    /// Get the destination IP range of the route.
    pub fn destination(&self) -> Ipv6Range {
        self.destination
    }

    /// Get the route's next hop
    pub fn next_hop(&self) -> Ipv6Addr {
        self.next_hop
    }

    /// Add the route to the routing table of the current network namespace.
    pub fn add_to_routing_table(self, iface_name: &str) -> Result<(), AddRouteError> {
        add_route_v6(self.destination, self.next_hop, iface_name)
    }
}

pub fn add_route_v6(
    destination: Ipv6Range,
    next_hop: Ipv6Addr,
    iface_name: &str,
) -> Result<(), AddRouteError> {
    let fd = unsafe {
        sys::socket(
            sys::PF_INET6 as i32,
            sys::__socket_type::SOCK_DGRAM as i32,
            libc::IPPROTO_IP as i32,
        )
    };
    if fd < 0 {
        let os_err = io::Error::last_os_error();
        match sys::errno() {
            libc::EMFILE => return Err(AddRouteError::ProcessFileDescriptorLimit(os_err)),
            libc::ENFILE => return Err(AddRouteError::SystemFileDescriptorLimit(os_err)),
            _ => {
                panic!("unexpected error creating socket: {}", os_err);
            },
        }
    }

    let mut route: sys::rtentry = unsafe {
        mem::zeroed()
    };

    #[cfg_attr(feature="clippy", allow(cast_ptr_alignment))]
    unsafe {
        let route_destination = &mut route.rt_dst as *mut _ as *mut libc::sockaddr_in6;
        (*route_destination).sin6_family = sys::AF_INET6 as u16;
        (*route_destination).sin6_addr = mem::transmute(u128::from(destination.base_addr()).to_be());
    };

    #[cfg_attr(feature="clippy", allow(cast_ptr_alignment))]
    unsafe {
        let route_genmask = &mut route.rt_genmask as *mut _ as *mut libc::sockaddr_in6;
        (*route_genmask).sin6_family = sys::AF_INET6 as u16;
        (*route_genmask).sin6_addr = mem::transmute(u128::from(destination.netmask()).to_be());
    };

    route.rt_flags = sys::RTF_UP as u16;
    #[cfg_attr(feature="clippy", allow(cast_ptr_alignment))]
    unsafe {
        let route_gateway = &mut route.rt_gateway as *mut _ as *mut libc::sockaddr_in6;
        (*route_gateway).sin6_family = sys::AF_INET6 as u16;
        (*route_gateway).sin6_addr = mem::transmute(u128::from(next_hop).to_be());
    };
    
    let name = match CString::new(iface_name) {
        Ok(name) => name,
        Err(..) => {
            return Err(AddRouteError::NameContainsNul);
        },
    };

    // TODO: This doesn't *actually* need to mutable yeah?
    route.rt_dev = name.as_ptr() as *mut _;

    let res = unsafe {
        libc::ioctl(fd, u64::from(sys::SIOCADDRT), &route)
    };
    if res != 0 {
        let os_err = io::Error::last_os_error();
        // TODO: there are definitely some errors that should be caught here.
        panic!("unexpected error from SIOCADDRT ioctl: {}", os_err);
    }

    Ok(())
}

