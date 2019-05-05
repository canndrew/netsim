use crate::priv_prelude::*;
use rand;

#[derive(PartialEq, Eq, Clone, Copy, PartialOrd, Ord, Hash)]
pub enum Ipv4AddrClass {
    Unspecified,
    CurrentNetwork,
    Private,
    CarrierNat,
    Loopback,
    LinkLocal,
    ProtocolAssignments,
    Testnet,
    Ipv6Relay,
    BenchmarkTests,
    Multicast,
    Reserved,
    Broadcast,
    Global,
}

/// Extension methods for IPv4 addresses
pub trait Ipv4AddrExt {
    /// Get a random, global IPv4 address.
    fn random_global() -> Ipv4Addr;
    /// Returns `true` if this is a global IPv4 address
    fn is_global(&self) -> bool;
    /// Returns `true` if this is a reserved IPv4 address.
    fn is_reserved(&self) -> bool;
    /// Clasify the address.
    fn class(&self) -> Ipv4AddrClass;
    /// Create an `Ipv4Addr` representing a netmask
    fn from_netmask_bits(bits: u8) -> Ipv4Addr;
}

impl Ipv4AddrExt for Ipv4Addr {
    fn random_global() -> Ipv4Addr {
        loop {
            let x: u32 = rand::random();
            let ip = Ipv4Addr::from(x);
            if Ipv4AddrExt::is_global(&ip) {
                return ip;
            }
        }
    }

    fn is_global(&self) -> bool {
        !(  self.is_loopback()
        ||  self.is_private()
        ||  self.is_link_local()
        ||  self.is_multicast()
        ||  self.is_broadcast()
        ||  self.is_documentation()
        ||  self.is_reserved()
        )
    }

    fn is_reserved(&self) -> bool {
        u32::from(*self) & 0xf000_0000 == 0xf000_0000
    }

    fn class(&self) -> Ipv4AddrClass {
        let ip = u32::from(*self);
        /*
         * needs feature(exclusive_range_patterns)
        match ip {
            0x00000000 .. 0x01000000 => Ipv4AddrClass::CurrentNetwork,
            0x0a000000 .. 0x0b000000 => Ipv4AddrClass::Private,
            0x64400000 .. 0x64800000 => Ipv4AddrClass::CarrierNat,
            0x7f000000 .. 0x80000000 => Ipv4AddrClass::Loopback,
            0xa9fe0000 .. 0xa9ff0000 => Ipv4AddrClass::LinkLocal,
            0xac100000 .. 0xac200000 => Ipv4AddrClass::Private,
            0xc0000000 .. 0xc0000100 => Ipv4AddrClass::ProtocolAssignments,
            0xc0000200 .. 0xc0000300 => Ipv4AddrClass::Testnet,
            0xc0586300 .. 0xc0586400 => Ipv4AddrClass::Ipv6Relay,
            0xc0a80000 .. 0xc0a90000 => Ipv4AddrClass::Private,
            0xc6120000 .. 0xc6140000 => Ipv4AddrClass::BenchmarkTests,
            0xc6336400 .. 0xc6336500 => Ipv4AddrClass::Testnet,
            0xcb007100 .. 0xcb007200 => Ipv4AddrClass::Testnet,
            0xe0000000 .. 0xf0000000 => Ipv4AddrClass::Multicast,
            0xf0000000 .. 0xffffffff => Ipv4AddrClass::Reserved,
            0xffffffff               => Ipv4AddrClass::Broadcast,
            _ => Ipv4AddrClass::Global,
        }
        */

        if ip == 0x00_00_00_00 { return Ipv4AddrClass::Unspecified };
        if ip >  0x00_00_00_00 && ip < 0x01_00_00_00 { return Ipv4AddrClass::CurrentNetwork };
        if ip >= 0x0a_00_00_00 && ip < 0x0b_00_00_00 { return Ipv4AddrClass::Private };
        if ip >= 0x64_40_00_00 && ip < 0x64_80_00_00 { return Ipv4AddrClass::CarrierNat };
        if ip >= 0x7f_00_00_00 && ip < 0x80_00_00_00 { return Ipv4AddrClass::Loopback };
        if ip >= 0xa9_fe_00_00 && ip < 0xa9_ff_00_00 { return Ipv4AddrClass::LinkLocal };
        if ip >= 0xac_10_00_00 && ip < 0xac_20_00_00 { return Ipv4AddrClass::Private };
        if ip >= 0xc0_00_00_00 && ip < 0xc0_00_01_00 { return Ipv4AddrClass::ProtocolAssignments };
        if ip >= 0xc0_00_02_00 && ip < 0xc0_00_03_00 { return Ipv4AddrClass::Testnet };
        if ip >= 0xc0_58_63_00 && ip < 0xc0_58_64_00 { return Ipv4AddrClass::Ipv6Relay };
        if ip >= 0xc0_a8_00_00 && ip < 0xc0_a9_00_00 { return Ipv4AddrClass::Private };
        if ip >= 0xc6_12_00_00 && ip < 0xc6_14_00_00 { return Ipv4AddrClass::BenchmarkTests };
        if ip >= 0xc6_33_64_00 && ip < 0xc6_33_65_00 { return Ipv4AddrClass::Testnet };
        if ip >= 0xcb_00_71_00 && ip < 0xcb_00_72_00 { return Ipv4AddrClass::Testnet };
        if ip >= 0xe0_00_00_00 && ip < 0xf0_00_00_00 { return Ipv4AddrClass::Multicast };
        if ip >= 0xf0_00_00_00 && ip < 0xff_ff_ff_ff { return Ipv4AddrClass::Reserved };
        if ip == 0xff_ff_ff_ff { return Ipv4AddrClass::Broadcast };
        Ipv4AddrClass::Global
    }

    fn from_netmask_bits(bits: u8) -> Ipv4Addr {
        Ipv4Addr::from(!((!0u32) >> bits))
    }
}

