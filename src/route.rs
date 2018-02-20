use priv_prelude::*;
use sys;

quick_error! {
    /// Errors returned by `add_route` and `Route::add`
    #[allow(missing_docs)]
    #[derive(Debug)]
    pub enum AddRouteError {
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
        NameContainsNul {
            description("interface name contains interior NUL byte")
        }
    }
}

/// Represents an IPv4 route.
#[derive(Debug, Clone, Copy)]
pub struct RouteV4 {
    destination: SubnetV4,
    gateway: Option<Ipv4Addr>,
}

impl RouteV4 {
    /// Create a new route with the given destination and gateway
    pub fn new(destination: SubnetV4, gateway: Option<Ipv4Addr>) -> RouteV4 {
        RouteV4 {
            destination,
            gateway,
        }
    }

    pub fn destination(&self) -> SubnetV4 {
        self.destination
    }

    pub fn gateway(&self) -> Option<Ipv4Addr> {
        self.gateway
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
        let os_err = io::Error::last_os_error();
        match (-fd) as u32 {
            sys::EMFILE => return Err(AddRouteError::ProcessFileDescriptorLimit(os_err)),
            sys::ENFILE => return Err(AddRouteError::SystemFileDescriptorLimit(os_err)),
            _ => {
                panic!("unexpected error creating socket: {}", os_err);
            },
        }
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
        let os_err = io::Error::last_os_error();
        // TODO: there are definitely some errors that should be caught here.
        panic!("unexpected error from SIOCADDRT ioctl: {}", os_err);
    }

    Ok(())
}

