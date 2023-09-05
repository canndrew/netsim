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
    pub rta_len: libc::c_ushort,
    pub rta_type: libc::c_ushort,
}

