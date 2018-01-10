use priv_prelude::*;
use sys;

quick_error! {
    /// Errors returned by `add_route` and `Route::add`
    #[allow(missing_docs)]
    #[derive(Debug)]
    pub enum AddRouteError {
        CreateControlSocket(e: io::Error) {
            description("failed to create control socket")
            display("failed to create control socket: {}", e)
            cause(e)
        }
        NameContainsNul {
            description("interface name contains interior NUL byte")
        }
        AddRoute(e: io::Error) {
            description("call to SIOCADDRT ioctl to add route failed")
            display("call to SIOCADDRT ioctl to add route failed: {}", e)
            cause(e)
        }
    }
}

/// Represents an IPv4 route.
#[derive(Clone, Copy)]
pub struct RouteV4 {
    /// The destination subnet of the route. All packets to this subnet will use this route.
    pub destination: SubnetV4,
    /// The gateway to route packets through (if any).
    pub gateway: Option<Ipv4Addr>,
}

impl RouteV4 {
    /// Create a new route with the given destination and gateway
    pub fn new(destination: SubnetV4, gateway: Option<Ipv4Addr>) -> RouteV4 {
        RouteV4 {
            destination,
            gateway,
        }
    }

    /// Add the route to the routing table of the current network namespace.
    pub fn add(self, iface_name: &str) -> Result<(), AddRouteError> {
        add_route(self.destination, self.gateway, iface_name)
    }
}

pub fn add_route(
    destination: SubnetV4,
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
        return Err(AddRouteError::CreateControlSocket(io::Error::last_os_error()));
    }

    let mut route: sys::rtentry = unsafe {
        mem::zeroed()
    };

    unsafe {
        let route_destination = &mut route.rt_dst as *mut _ as *mut sys::sockaddr_in;
        (*route_destination).sin_family = sys::AF_INET as u16;
        (*route_destination).sin_addr = sys::in_addr { s_addr: u32::from(destination.base_addr()).to_be() };
    };

    unsafe {
        let route_genmask = &mut route.rt_genmask as *mut _ as *mut sys::sockaddr_in;
        (*route_genmask).sin_family = sys::AF_INET as u16;
        (*route_genmask).sin_addr = sys::in_addr { s_addr: u32::from(destination.netmask()).to_be() };
    };

    route.rt_flags = sys::RTF_UP as u16;
    if let Some(gateway_addr) = gateway {
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
        sys::ioctl(fd, sys::SIOCADDRT as u64, &route)
    };
    if res != 0 {
        return Err(AddRouteError::AddRoute(io::Error::last_os_error()));
    }

    Ok(())
}

