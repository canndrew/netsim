//! contains functions for spawning network environments.
//!
//! `new_namespace` is the most fundamental of these functions though there are other, more
//! higher-level functions which are likely to be more useful for testing.

use priv_prelude::*;
use libc;
use std::sync::mpsc;
use future_utils;
use std::thread::JoinHandle;
use sys;
use void;

const STACK_ALIGN: usize = 16;

trait FnBox<R> {
    fn call_box(self: Box<Self>) -> R;
}

impl<F, R> FnBox<R> for F
where
    F: FnOnce() -> R
{
    fn call_box(self: Box<Self>) -> R {
        (*self)()
    }
}

/// Run the function `func` in its own network namespace. This namespace will not have any network
/// interfaces. You can create virtual interfaces using `Tap`, or use one of the other functions in
/// this module which do this for you.
pub fn new_namespace<F, R>(func: F) -> JoinHandle<R>
where
    F: FnOnce() -> R,
    F: Send + 'static,
    R: Send + 'static,
{
    let stack_size = unsafe { sys::getpagesize() } as usize;
    let stack_size = cmp::max(stack_size, 4096);

    let mut stack = Vec::with_capacity(stack_size + STACK_ALIGN);
    let stack_base = stack.as_mut_ptr();
    mem::forget(stack);

    let flags = 
        libc::CLONE_FILES |
        libc::CLONE_IO |
        libc::CLONE_SIGHAND |
        libc::CLONE_VM |
        libc::CLONE_SYSVSEM |
        //libc::CLONE_THREAD;
        libc::CLONE_NEWNET |
        libc::CLONE_NEWUTS |
        libc::CLONE_NEWUSER;

    //type CbData<R: Send + 'static> = (Box<FnBox<R> + Send>, Arc<JoinHandleInner<R>>);
    //type CbData = (Box<FnBox<R> + Send>, Arc<JoinHandleInner<R>>);
    /*
    struct CbData<R: Send + 'static> {
        func: Box<FnBox<R> + Send + 'static>,
        inner: Arc<JoinHandleInner<R>>,
    }
    */

    struct CbData<R: Send + 'static> {
        func: Box<FnBox<R> + Send + 'static>,
        joiner_tx: mpsc::Sender<JoinHandle<R>>,
        stack_base: *mut u8,
        stack_size: usize,
    }
    
    extern "C" fn clone_cb<R: Send + 'static>(arg: *mut c_void) -> c_int {
        let (drop_tx, drop_rx) = mpsc::channel::<Void>();
        {
            let data: *mut CbData<R> = arg as *mut _;
            let data: Box<CbData<R>> = unsafe { Box::from_raw(data) };
            //let data: *mut CbData = arg as *mut _;
            //let data: Box<CbData> = unsafe { Box::from_raw(data) };
            let data = *data;
            let CbData { func, joiner_tx, stack_base, stack_size } = data;

            struct StackBase(*mut u8);
            unsafe impl Send for StackBase {}

            let stack_base = StackBase(stack_base);

            // WARNING: HACKERY
            // 
            // This should ideally be done without spawning another thread. We're already inside a
            // thread (spawned by clone), but that thread doesn't respect rust's thread-local
            // storage for some reason. So we spawn a thread in a thread in order to get our own
            // local storage keys. There should be a way to do this which doesn't involve spawning
            // two threads and letting one of them die. This would require going back to crafting
            // our own `JoinHandle` though.

            let tid = unsafe {
                sys::syscall(sys::SYS_gettid as libc::c_long)
            };
            let joiner = thread::spawn(move || {
                let ret = func.call_box();

                // This will unblock when the clone_cb thread drops the drop_tx. This should be
                // the last thing that happens before the thread ends, meaning it's now safe to
                // free it's stack.
                match drop_rx.recv() {
                    Ok(v) => void::unreachable(v),
                    Err(_) => (),
                };

                // Try to make sure the clone_cb thread has exited before freeing its stack. It's
                // possible the tid could get reused, so we put a recursion limit on this to avoid
                // going into a busy loop. Assume that if drop_rx fired then it's not going to take
                // long for the clone_cb thread to really finish.
                //
                // TODO: Figure out a non-racy way of implementing this.
                for _ in 0..100 {
                    let res = unsafe {
                        sys::syscall(sys::SYS_tkill as libc::c_long, tid, 0)
                    };
                    if res == 0 {
                        break;
                    }
                    thread::yield_now();
                }

                // Free the stack.
                let StackBase(stack_base) = stack_base;
                let _stack = unsafe { Vec::from_raw_parts(stack_base, 0, stack_size + STACK_ALIGN) };
                ret
            });
            let _ = joiner_tx.send(joiner);
        }
        drop(drop_tx);
        0
    }

    let (joiner_tx, joiner_rx) = mpsc::channel();
    let stack_head = ((stack_base as usize + stack_size + STACK_ALIGN) & !(STACK_ALIGN - 1)) as *mut c_void;
    let func = Box::new(func);
    let arg: Box<CbData<R>> = Box::new(CbData { func, joiner_tx, stack_base, stack_size });
    let arg = Box::into_raw(arg) as *mut c_void;
    
    let res = unsafe {
        libc::clone(clone_cb::<R>, stack_head, flags, arg)
    };
    assert!(res != -1);

    unwrap!(joiner_rx.recv())
}

/// Spawn a function into a new network namespace with a set of network interfaces described by
/// `ifaces`. Returns a `JoinHandle` which can be used to join the spawned thread, along with a set
/// of `Tap`s, one for each interface, which can be used to read/write network activity from the
/// spawned thread.
pub fn with_ifaces<F, R>(
    handle: &Handle,
    ifaces: Vec<TapBuilderV4>,
    func: F,
) -> (JoinHandle<R>, Vec<EtherBox>)
where
    R: Send + 'static,
    F: FnOnce() -> R + Send + 'static,
{
    let (tx, rx) = mpsc::channel();
    let mut timeouts = Vec::with_capacity(ifaces.len());
    for _ in 0..ifaces.len() {
        timeouts.push(Timeout::new(Duration::from_millis(100), handle));
    }

    let join_handle = new_namespace(move || {
        let mut taps = Vec::with_capacity(ifaces.len());
        let mut drop_txs = Vec::with_capacity(ifaces.len());
        for (tap_builder, timeout) in ifaces.into_iter().zip(timeouts) {
            trace!("building tap {:?}", tap_builder);
            let (drop_tx, drop_rx) = future_utils::drop_notify();
            let drop_rx = drop_rx.and_then(|()| timeout);
            let tap_unbound = unwrap!(tap_builder.build_unbound());
            taps.push((tap_unbound, drop_rx));
            drop_txs.push(drop_tx);
        }
        unwrap!(tx.send(taps));
        let ret = func();
        drop(drop_txs);
        ret
    });

    let taps_unbound = unwrap!(rx.recv());
    let mut taps = Vec::with_capacity(taps_unbound.len());
    for (tap_unbound, drop_rx) in taps_unbound {
        let tap = tap_unbound.bind(handle);
        let tap = WithDisconnect::new(tap, drop_rx, handle);
        let tap = Box::new(tap);
        let tap = tap as EtherBox;
        taps.push(tap);
    }
    
    (join_handle, taps)
}

/// Spawn a function into a new network namespace with a network interface described by `iface`.
/// Returns a `JoinHandle` which can be used to join the spawned thread, along with a `Tap` which
/// can be used to read/write network activity from the spawned thread.
pub fn with_iface<F, R>(
    handle: &Handle,
    iface: TapBuilderV4,
    func: F,
) -> (JoinHandle<R>, EtherBox)
where
    R: Send + 'static,
    F: FnOnce() -> R + Send + 'static,
{
    let (tx, rx) = mpsc::channel();
    let timeout = Timeout::new(Duration::from_millis(100), handle);
    let join_handle = new_namespace(move || {
        trace!("building tap {:?}", iface);
        let (drop_tx, drop_rx) = future_utils::drop_notify();
        let drop_rx = drop_rx.and_then(|()| timeout);
        let tap_unbound = unwrap!(iface.build_unbound());
        unwrap!(tx.send((tap_unbound, drop_rx)));
        let ret = func();
        drop(drop_tx);
        ret
    });

    let (tap_unbound, drop_rx) = unwrap!(rx.recv());
    let tap = tap_unbound.bind(handle);
    let tap = WithDisconnect::new(tap, drop_rx, handle);
    let tap = Box::new(tap);
    let tap = tap as EtherBox;

    (join_handle, tap)
}

/*
pub fn on_internet<F, R>(
    handle: &Handle,
    func: F,
) -> (JoinHandle<R>, EtherBox)
where
    R: Send + 'static,
    F: FnOnce(Ipv4Addr) -> R + Send + 'static,
{
    let mut tap_builder = TapBuilderV4::new();
    let ip = Ipv4Addr::random_global();
    tap_builder.address(ip);
    trace!("ip == {}", ip);
    let route = RouteV4::new(SubnetV4::new(ip, 0), None);
    trace!("tap_builder has route {:?}", route);
    //tap_builder.route(RouteV4::new(SubnetV4::new(ipv4!("0.0.0.0"), 0), None));
    tap_builder.route(route);

    let (join_handle, tap) = with_iface(handle, tap_builder, move || func(ip));

    (join_handle, tap)
}
*/

/// Spawn a function into a new network namespace with a single network interface with an address
/// in `subnet`. Returns a `JoinHandle` which can be used to join the spawned thread, along with
/// an `EtherBox` which can be used to read/write network activity from the spawned thread.
pub fn on_subnet<F, R>(
    handle: &Handle,
    subnet: SubnetV4,
    func: F,
) -> (JoinHandle<R>, EtherBox)
where
    R: Send + 'static,
    F: FnOnce(Ipv4Addr) -> R + Send + 'static
{
    let mut tap_builder = TapBuilderV4::new();
    let ip = subnet.random_client_addr();
    tap_builder.address(ip);
    tap_builder.netmask(subnet.netmask());
    tap_builder.route(RouteV4::new(subnet, None));

    let (join_handle, tap) = with_iface(handle, tap_builder, move || func(ip));

    (join_handle, tap)
}

/// Spawn a function into a new network namespace with a single network interface behind a virtual
/// NAT. The returned `Gateway` can be used to read/write network activity from the public side of
/// the NAT.
pub fn behind_gateway<F, R>(
    handle: &Handle,
    func: F,
) -> (JoinHandle<R>, Gateway)
where
    R: Send + 'static,
    F: FnOnce() -> R + Send + 'static,
{
    let subnet = SubnetV4::random_local();
    let mut tap_builder = TapBuilderV4::new();
    tap_builder.address(subnet.random_client_addr());
    tap_builder.netmask(subnet.netmask());
    tap_builder.route(RouteV4::new(SubnetV4::global(), Some(subnet.gateway_ip())));

    let (join_handle, tap) = with_iface(handle, tap_builder, func);
    let gateway = {
        GatewayBuilder::new(subnet)
        .build(Box::new(tap))
    };

    (join_handle, gateway)
}

#[cfg(test)]
mod test {
    use super::*;
    use get_if_addrs;
    use std;
    use rand;
    use std::cell::Cell;
    use env_logger;

    #[test]
    fn respects_thread_local_storage() {
        thread_local! {
            static TEST: Cell<u32> = Cell::new(0);
        };

        TEST.with(|v| v.set(123));
        let join_handle = new_namespace(|| {
            TEST.with(|v| {
                assert_eq!(v.get(), 0);
                v.set(456);
            });
        });
        unwrap!(join_handle.join());
        TEST.with(|v| assert_eq!(v.get(), 123));
    }

    #[test]
    fn test_no_network() {
        let mut core = unwrap!(Core::new());
        let handle = core.handle();
        let res = core.run(future::lazy(|| {
            let handle = handle.remote().handle().unwrap();
            let (join_handle, taps) = with_ifaces(&handle, vec![], || {
                unwrap!(get_if_addrs::get_if_addrs())
            });
            assert!(taps.is_empty());
            let if_addrs = unwrap!(join_handle.join());
            assert!(if_addrs.is_empty());
            Ok(())
        }));
        res.void_unwrap()
    }

    #[test]
    fn test_one_interface_send_udp() {
        let _ = env_logger::init();
        let mut core = unwrap!(Core::new());
        let handle = core.handle();
        let res = core.run(future::lazy(|| {
            trace!("starting");
            let subnet = SubnetV4::random_local();
            let mut tap_builder = TapBuilderV4::new();
            tap_builder.address(subnet.random_client_addr());
            tap_builder.netmask(subnet.netmask());
            tap_builder.route(RouteV4::new(
                SubnetV4::new(ipv4!("0.0.0.0"), 0),
                Some(subnet.gateway_ip()),
            ));

            let payload: [u8; 8] = rand::random();
            let addr = addrv4!("1.2.3.4:56");
            trace!("spawning thread");
            let (join_handle, tap) = with_iface(&handle, tap_builder, move || {
                let socket = unwrap!(std::net::UdpSocket::bind(addr!("0.0.0.0:0")));
                unwrap!(socket.send_to(&payload[..], SocketAddr::V4(addr)));
                trace!("sent udp packet");
            });

            let mac_addr = rand::random();
            let (tap_tx, tap_rx) = tap.split();
            tap_rx
            .map_err(|e| panic!("error reading tap: {}", e))
            .into_future()
            .map_err(|(v, _)| v)
            .and_then(move |(frame_opt, tap_rx)| {
                let mut frame = unwrap!(frame_opt);
                match frame.payload() {
                    EtherPayload::Arp(arp) => {
                        let src_mac = frame.source();
                        frame.set_destination(src_mac);
                        frame.set_source(mac_addr);
                        frame.set_payload(EtherPayload::Arp(arp.response(mac_addr)));

                        tap_tx
                        .send(frame)
                        .map_err(|e| panic!("error sending arp reply: {}", e))
                        .and_then(|_tap_tx| {
                            tap_rx
                            .into_future()
                            .map_err(|(v, _)| v)
                        })
                    },
                    _ => panic!("unexpected frame {:?}", frame),
                }
            })
            .map(move |(frame_opt, _tap_rx)| {
                let frame = unwrap!(frame_opt);
                match frame.payload() {
                    EtherPayload::Ipv4(ipv4_packet) => {
                        let dest_ip = ipv4_packet.destination();
                        let udp_packet = match ipv4_packet.payload() {
                            Ipv4Payload::Udp(udp_packet) => udp_packet,
                            x => panic!("unexpected packet type: {:?}", x),
                        };
                        let dest_port = udp_packet.destination_port();
                        let dest = SocketAddrV4::new(dest_ip, dest_port);
                        assert_eq!(dest, addr);
                        assert_eq!(udp_packet.payload(), &payload[..]);
                    }
                    _ => panic!("unexpected frame {:?}", frame),
                }
            })
            .map(move |()| unwrap!(join_handle.join()))
        }));
        res.void_unwrap()
    }

    /*
    #[test]
    fn can_talk_to_self_on_internet() {
        use tokio_core::net::UdpSocket;

        let _ = env_logger::init();
        let mut core = unwrap!(Core::new());
        let handle = core.handle();
        let res = core.run(future::lazy(move || {
            let (join_handle, tap) = on_internet(&handle, move || {
                trace!("our pid is {:?}", unsafe { ::sys::getpid() });
                thread::sleep(Duration::from_secs(10));

                ::std::process::Command::new("route").status().unwrap();

                let mut core = unwrap!(Core::new());
                let handle = core.handle();

                let if_addrs = unwrap!(get_if_addrs::get_if_addrs());
                let ip = if_addrs.into_iter().filter_map(|if_addr| {
                    match if_addr.addr {
                        get_if_addrs::IfAddr::V4(ifv4_addr) => Some(ifv4_addr.ip),
                        _ => None,
                    }
                }).next().unwrap();

                let addr0 = SocketAddr::V4(SocketAddrV4::new(ip, 45666));
                let socket0 = unwrap!(UdpSocket::bind(&addr0, &handle));
                let addr1 = SocketAddr::V4(SocketAddrV4::new(ip, 45667));
                let socket1 = unwrap!(UdpSocket::bind(&addr1, &handle));

                trace!("addr0 == {}", addr0);
                let res = core.run({
                    socket1
                    .send_dgram([123], addr0)
                    .and_then(|_| {
                        trace!("sent packet");
                        socket0.recv_dgram([0; 32])
                    })
                    .map(|_| trace!("got data"))
                    .map_err(|e| panic!(e))
                });
                res.void_unwrap()
            });
            join_handle.join();
            Ok(())
        }));
        res.void_unwrap()
    }
    */

    /*
    #[test]
    fn connect_over_link() {
        const DATA_LEN: usize = 1024 * 1024;
        let port = 1234;

        let listener_thread = move |ip0, _ip1| {
            println!("listening get_if_addrs: {:?}", get_if_addrs::get_if_addrs());
            thread::sleep(Duration::from_secs(1));
            unwrap!(Command::new("route").status());
            thread::sleep(Duration::from_secs(2));
            unwrap!(Command::new("ifconfig").status());
            thread::sleep(Duration::from_secs(2));

            let mut core = unwrap!(Core::new());
            let handle = core.handle();
            let res = core.run({
                let addr = SocketAddr::V4(SocketAddrV4::new(ip0, port));
                let listener = unwrap!(TcpListener::bind(&addr, &handle));

                listener
                .incoming()
                .into_future()
                .map_err(|(e, _incoming)| e)
                .and_then(|(stream_opt, _incoming)| {
                    let (stream, _addr) = unwrap!(stream_opt);
                    let buf = Vec::from(&[0u8; DATA_LEN][..]);
                    tokio_io::io::read_exact(stream, buf)
                    .and_then(|(stream, buf)| {
                        tokio_io::io::write_all(stream, buf)
                        .map(|(_stream, _buf)| ())
                    })
                })
                .with_timeout(Duration::from_secs(5), &handle)
                .map(|opt| unwrap!(opt))
            });
            unwrap!(res)
        };

        let connecting_thread = move |ip0, _ip1| {
            println!("connecting get_if_addrs: {:?}", get_if_addrs::get_if_addrs());
            thread::sleep(Duration::from_secs(2));
            unwrap!(Command::new("route").status());
            thread::sleep(Duration::from_secs(2));
            unwrap!(Command::new("ifconfig").status());
            thread::sleep(Duration::from_secs(1));

            let addr = SocketAddr::V4(SocketAddrV4::new(ip0, port));
            let mut out_buf = Vec::from(&[0u8; DATA_LEN][..]);
            let in_buf = Vec::from(&[0u8; DATA_LEN][..]);
            rand::thread_rng().fill_bytes(&mut out_buf[..]);

            let mut core = unwrap!(Core::new());
            let handle0 = core.handle();
            let handle1 = core.handle();
            let res = core.run({
                TcpStream::connect(&addr, &handle0)
                .map_err(|e| panic!("connect failed: {}", e))
                .and_then(move |stream| {
                    tokio_io::io::write_all(stream, out_buf)
                    .map_err(|e| panic!("write failed: {}", e))
                    .and_then(move |(stream, out_buf)| {
                        tokio_io::io::read_exact(stream, in_buf)
                        .map_err(|e| panic!("read failed: {}", e))
                        .map(move |(_stream, in_buf)| {
                            assert!(in_buf == out_buf);
                        })
                    })
                })
                .with_timeout(Duration::from_secs(5), &handle1)
                .map(|opt| unwrap!(opt))
            });
            unwrap!(res)
        };

        let ((), ()) = direct_link(listener_thread, connecting_thread);
    }
    */
}
