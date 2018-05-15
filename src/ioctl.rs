use priv_prelude::*;
use sys;

ioctl!(bad read siocgifflags with 0x8913; sys::ifreq);
ioctl!(bad write siocsifflags with 0x8914; sys::ifreq);
ioctl!(bad write siocsifaddr with 0x8916; sys::ifreq);
//ioctl!(bad read siocgifnetmask with 0x891b; sys::ifreq);
ioctl!(bad write siocsifnetmask with 0x891c; sys::ifreq);
ioctl!(write tunsetiff with b'T', 202; c_int);
ioctl!(bad write siocsifhwaddr with 0x8924; sys::ifreq);
ioctl!(bad read siocgifhwaddr with 0x8927; sys::ifreq);
ioctl!(bad read siocgifindex with 0x8933; sys::ifreq);

