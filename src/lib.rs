extern crate libc;
extern crate rand;
extern crate byteorder;
extern crate bytes;
#[macro_use]
extern crate unwrap;
extern crate void;
extern crate get_if_addrs;
#[macro_use]
extern crate net_literals;
#[macro_use]
extern crate quick_error;
#[macro_use]
extern crate ioctl_sys;
#[macro_use]
extern crate log;
extern crate mio;
extern crate futures;
extern crate tokio_io;
extern crate tokio_core;
#[macro_use]
extern crate rand_derive;
extern crate future_utils;

#[cfg(test)]
extern crate capabilities;
#[cfg(test)]
extern crate env_logger;


/// Convert a variable-length slice to a fixed-length array
macro_rules! slice_assert_len {
    ($len:tt, $slice:expr) => {{
        use std::ptr;

        union MaybeUninit<T: Copy> {
            init: T,
            uninit: (),
        }
        
        let mut array: MaybeUninit<[_; $len]> = MaybeUninit { uninit: () };
        let slice: &[_] = $slice;
        for i in 0..$len {
            let x = slice[i];
            unsafe {
                ptr::write(&mut array.init[i], x)
            }
        }

        unsafe {
            array.init
        }
    }}
}

mod priv_prelude;
mod util;
mod sys;
mod tap;
mod async_fd;
mod route;
mod subnet;
pub mod veth;
pub mod wire;
pub mod spawn;

