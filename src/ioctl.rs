use crate::priv_prelude::*;

ioctl!(bad read siocgifflags with 0x8913; libc::ifreq);
ioctl!(bad write siocsifflags with 0x8914; libc::ifreq);
ioctl!(bad write siocsifaddr with 0x8916; libc::ifreq);
//ioctl!(bad read siocgifnetmask with 0x891b; sys::ifreq);
ioctl!(bad write siocsifnetmask with 0x891c; libc::ifreq);
ioctl!(write tunsetiff with b'T', 202; c_int);
ioctl!(bad write siocsifhwaddr with 0x8924; libc::ifreq);
ioctl!(bad read siocgifhwaddr with 0x8927; libc::ifreq);
ioctl!(bad read siocgifindex with 0x8933; libc::ifreq);

