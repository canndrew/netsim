//! IP devices for creating networks with.

mod channel;
mod hub;
mod nat;

pub use self::{
    channel::{BiChannel, IpChannel},
    hub::IpHub,
    nat::{Nat, NatBuilder},
};
