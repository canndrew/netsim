use priv_prelude::*;
use super::*;
use sys;
use libc;

/// Represents an IPv4 route.
#[derive(Debug, Clone, Copy)]
pub struct Ipv4Route {
    destination: Ipv4Range,
    gateway: Option<Ipv4Addr>,
}

impl Ipv4Route {
    /// Create a new route with the given destination and gateway
    pub fn new(destination: Ipv4Range, gateway: Option<Ipv4Addr>) -> Ipv4Route {
        Ipv4Route {
            destination,
            gateway,
        }
    }

    /// Get the destination IP range of the route.
    pub fn destination(&self) -> Ipv4Range {
        self.destination
    }

    /// Get the route's gateway (if any).
    pub fn gateway(&self) -> Option<Ipv4Addr> {
        self.gateway
    }

    /// Add the route to the routing table of the current network namespace.
    pub fn add_to_routing_table(self, iface_name: &str) -> Result<(), AddRouteError> {
        add_route_v4(self.destination, self.gateway, iface_name)
    }
}

pub fn add_route_v4(
    destination: Ipv4Range,
    gateway: Option<Ipv4Addr>,
    iface_name: &str,
) -> Result<(), AddRouteError> {
    let fd = unsafe {
        sys::socket(
            sys::PF_INET as i32,
            sys::__socket_type::SOCK_DGRAM as i32,
            sys::IPPROTO_IP as i32,
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
        let route_destination = &mut route.rt_dst as *mut _ as *mut sys::sockaddr_in;
        (*route_destination).sin_family = sys::AF_INET as u16;
        (*route_destination).sin_addr = sys::in_addr { s_addr: u32::from(destination.base_addr()).to_be() };
    };

    #[cfg_attr(feature="clippy", allow(cast_ptr_alignment))]
    unsafe {
        let route_genmask = &mut route.rt_genmask as *mut _ as *mut sys::sockaddr_in;
        (*route_genmask).sin_family = sys::AF_INET as u16;
        (*route_genmask).sin_addr = sys::in_addr { s_addr: u32::from(destination.netmask()).to_be() };
    };

    route.rt_flags = sys::RTF_UP as u16;
    if let Some(gateway_addr) = gateway {
        #[cfg_attr(feature="clippy", allow(cast_ptr_alignment))]
        unsafe {
            let route_gateway = &mut route.rt_gateway as *mut _ as *mut sys::sockaddr_in;
            (*route_gateway).sin_family = sys::AF_INET as u16;
            (*route_gateway).sin_addr = sys::in_addr { s_addr: u32::from(gateway_addr).to_be() };
        };
    
        route.rt_flags |= sys::RTF_GATEWAY as u16;
    }

    let name = match CString::new(iface_name) {
        Ok(name) => name,
        Err(..) => {
            return Err(AddRouteError::NameContainsNul);
        },
    };

    // TODO: This doesn't *actually* need to mutable yeah?
    route.rt_dev = name.as_ptr() as *mut _;

    let res = unsafe {
        sys::ioctl(fd, u64::from(sys::SIOCADDRT), &route)
    };
    if res != 0 {
        let os_err = io::Error::last_os_error();
        // TODO: there are definitely some errors that should be caught here.
        panic!("unexpected error from SIOCADDRT ioctl: {}", os_err);
    }

    Ok(())
}

