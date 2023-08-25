pub(crate) use {
    std::{
        cmp, fmt, io, mem, panic, ptr, slice, task, thread,
        collections::{hash_map, HashMap, BTreeMap, HashSet},
        ffi::{CStr, CString},
        future::{Future, IntoFuture},
        fs::File,
        io::Write,
        mem::MaybeUninit,
        net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddrV4},
        os::fd::{OwnedFd, FromRawFd, AsRawFd},
        pin::Pin,
        sync::Arc,
        task::Poll,
        time::{Duration, Instant},
    },
    bytes::BytesMut,
    libc::{c_int, c_void, pid_t},
    tokio::io::unix::AsyncFd,
    futures::{
        ready, FutureExt, Sink, Stream, StreamExt,
        channel::mpsc,
        //stream::FuturesUnordered,
    },
    ioctl_sys::ioctl,
    net_literals::ipv4,
    pin_project::pin_project,
    rand::Rng,
    log::{log_enabled, debug, Level},
    crate::{
        namespace, ioctl, iface, adapter,
        device::IpChannel,
        machine::Machine,
        iface::{
            create::IpIfaceBuilder,
            stream::{IpIface, IpSinkStream},
        },
        network::Ipv4Network,
        packet::{
            IpPacket, IpPacketVersion, Ipv4PacketProtocol,
        },
    },
};

#[cfg(test)]
pub(crate) use {
    net_literals::addrv4,
    tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::{TcpStream, TcpListener},
    },
    crate::device::{IpHub, NatBuilder},
};

