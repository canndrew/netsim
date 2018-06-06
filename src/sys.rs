#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![cfg_attr(feature="clippy", allow(unreadable_literal))]
#![cfg_attr(feature="clippy", allow(const_static_lifetime))]
#![cfg_attr(feature="clippy", allow(useless_transmute))]
#![cfg_attr(feature="clippy", allow(expl_impl_clone_on_copy))]
#![cfg_attr(feature="clippy", allow(zero_ptr))]

pub use libc::*;

#[repr(C)]
#[derive(Clone, Copy)]
pub union __ifreq_ifr_ifrn {
    pub ifrn_name: [c_char; IFNAMSIZ],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub union __ifreq_ifr_ifru {
    pub ifru_addr: sockaddr,
    pub ifru_dstaddr: sockaddr,
    pub ifru_broadaddr: sockaddr,
    pub ifru_netmask: sockaddr,
    pub ifru_hwaddr: sockaddr,
    pub ifru_flags: c_short,
    pub ifru_ivalue: c_int,
    pub ifru_mtu: c_int,
    pub ifru: ifmap,
    pub ifru_slave: [c_char; IFNAMSIZ],
    pub ifru_newname: [c_char; IFNAMSIZ],
    pub ifru_data: *mut c_void,
}

// TODO: add ifreq to libc as soon as libc supports anonymous unions embedded in structs
#[repr(C)]
#[derive(Clone, Copy)]
pub struct ifreq {
    pub ifr_ifrn: __ifreq_ifr_ifrn,
    pub ifr_ifru: __ifreq_ifr_ifru,
}

pub fn errno() -> ::std::os::raw::c_int {
    unsafe {
        *__errno_location()
    }
}



// NOTE: everything under here should be obsolete as soon as PRs to libc are merged and published



pub const SIOCADDRT: ::std::os::raw::c_uint = 35083;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ifmap {
    pub mem_start: ::std::os::raw::c_ulong,
    pub mem_end: ::std::os::raw::c_ulong,
    pub base_addr: ::std::os::raw::c_ushort,
    pub irq: ::std::os::raw::c_uchar,
    pub dma: ::std::os::raw::c_uchar,
    pub port: ::std::os::raw::c_uchar,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct rtentry {
    pub rt_pad1: c_ulong,
    pub rt_dst: sockaddr,
    pub rt_gateway: sockaddr,
    pub rt_genmask: sockaddr,
    pub rt_flags: c_ushort,
    pub rt_pad2: c_short,
    pub rt_pad3: c_ulong,
    pub rt_tos: c_uchar,
    pub rt_class: c_uchar,
    #[cfg(target_pointer_width = "64")]
    pub rt_pad4: [c_short; 3usize],
    #[cfg(not(target_pointer_width = "64"))]
    pub rt_pad4: c_short,
    pub rt_metric: c_short,
    pub rt_dev: *mut c_char,
    pub rt_mtu: c_ulong,
    pub rt_window: c_ulong,
    pub rt_irtt: c_ushort,
}

pub const RTF_UP: c_ushort = 0x0001;
pub const RTF_GATEWAY: c_ushort = 0x0002;
pub const ARPHRD_ETHER: u16 = 1;

pub const NLMSG_ALIGNTO: usize = 4;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ifaddrmsg {
    pub ifa_family: u8,
    pub ifa_prefixlen: u8,
    pub ifa_flags: u8,
    pub ifa_scope: u8,
    pub ifa_index: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct rtattr {
    pub rta_len: c_ushort,
    pub rta_type: c_ushort,
}

pub const RTM_NEWADDR: u16 = 20;
pub const IFA_ADDRESS: u16 = 1;

