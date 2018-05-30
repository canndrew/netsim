#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(unused)]
#![cfg_attr(feature="clippy", allow(unreadable_literal))]
#![cfg_attr(feature="clippy", allow(const_static_lifetime))]
#![cfg_attr(feature="clippy", allow(useless_transmute))]
#![cfg_attr(feature="clippy", allow(expl_impl_clone_on_copy))]
#![cfg_attr(feature="clippy", allow(zero_ptr))]

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

pub fn errno() -> ::std::os::raw::c_int {
    use libc;
    unsafe {
        *libc::__errno_location()
    }
}

