//! contains functions for spawning network environments.
//!
//! `new_namespace` is the most fundamental of these functions though there are other, more
//! higher-level functions which are likely to be more useful for testing.

use priv_prelude::*;
use libc;
use std::sync::mpsc;

const STACK_SIZE: usize = 8 * 1024 * 1024;
const STACK_ALIGN: usize = 16;

/// A join handle for a thread.
pub struct JoinHandle<R> {
    inner: Arc<JoinHandleInner<R>>,
    stack_base: *mut u8,
}

struct JoinHandleInner<R> {
    result: Mutex<Option<thread::Result<R>>>,
    condvar: Condvar,
}

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
    let mut stack = Vec::with_capacity(STACK_SIZE + STACK_ALIGN);
    let stack_base = stack.as_mut_ptr();
    mem::forget(stack);

    let inner = Arc::new(JoinHandleInner {
        result: Mutex::new(None),
        condvar: Condvar::new(),
    });

    let inner_cloned = inner.clone();

    let join_handle = JoinHandle {
        inner: inner,
        stack_base: stack_base,
    };

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
    struct CbData<R: Send + 'static> {
        func: Box<FnBox<R> + Send + 'static>,
        inner: Arc<JoinHandleInner<R>>,
    }
    
    extern "C" fn clone_cb<R: Send + 'static>(arg: *mut c_void) -> c_int {
        let data: *mut CbData<R> = arg as *mut _;
        let data: Box<CbData<R>> = unsafe { Box::from_raw(data) };
        //let data: *mut CbData = arg as *mut _;
        //let data: Box<CbData> = unsafe { Box::from_raw(data) };
        let data = *data;
        let CbData { func, inner } = data;

        // WARNING: HACKERY
        // 
        // This should ideally be done without spawning another thread. We're already inside a
        // thread (spawned by clone), but that thread doesn't respect rust's thread-local storage
        // for some reason. So we spawn a thread in a thread in order to get our own local storage
        // keys. There should be a way to do this which doesn't involve spawning two threads and
        // letting one of them die.
        //
        // Additionally, if we do want to spawn a seperate thread then we should be able to use
        // its JoinHandle rather than crafting our own.

        thread::spawn(move || {
            let func = AssertUnwindSafe(func);
            let r = panic::catch_unwind(move || {
                let AssertUnwindSafe(func) = func;
                func.call_box()
            });

            let mut result = unwrap!(inner.result.lock());
            *result = Some(r);
            drop(result);
            inner.condvar.notify_one();
        });
        0
    }

    let stack_head = ((stack_base as usize + STACK_SIZE + STACK_ALIGN) & !(STACK_ALIGN - 1)) as *mut c_void;
    let func = Box::new(func);
    //let arg: Box<CbData<R>> = Box::new((func, inner_cloned));
    let arg: Box<CbData<R>> = Box::new(CbData { func: func, inner: inner_cloned, });
    //let arg: Box<CbData> = Box::new((func, inner_cloned));
    let arg = Box::into_raw(arg) as *mut c_void;
    
    let res = unsafe {
        libc::clone(clone_cb::<R>, stack_head, flags, arg)
    };
    assert!(res != -1);

    join_handle
}

impl<R> JoinHandle<R> {
    /// Join a thread, returning its result.
    pub fn join(mut self) -> thread::Result<R> {
        let mut result = unwrap!(self.inner.result.lock());
        loop {
            if let Some(r) = result.take() {
                let _v = unsafe { Vec::from_raw_parts(self.stack_base, 0, STACK_SIZE) };
                self.stack_base = ptr::null_mut();
                return r;
            }
            result = unwrap!(self.inner.condvar.wait(result));
        }
    }
}

impl<R> Drop for JoinHandle<R> {
    fn drop(&mut self) {
        if self.stack_base.is_null() {
            return
        }

        let mut result = unwrap!(self.inner.result.lock());
        loop {
            if let Some(..) = result.take() {
                let _v = unsafe { Vec::from_raw_parts(self.stack_base, 0, STACK_SIZE) };
                return;
            }
            result = unwrap!(self.inner.condvar.wait(result));
        }
    }
}

/// Spawn a function into a new network namespace with a set of network interfaces described by
/// `ifaces`. Returns a `JoinHandle` which can be used to join the spawned thread, along with a set
/// of `Tap`s, one for each interface, which can be used to read/write network activity from the
/// spawned thread.
pub fn spawn_with_ifaces<F, R>(
    handle: &Handle,
    ifaces: Vec<TapBuilderV4>,
    func: F,
) -> (JoinHandle<R>, Vec<Tap>)
where
    R: Send + 'static,
    F: FnOnce() -> R + Send + 'static,
{
    let (tx, rx) = mpsc::channel();
    let join_handle = new_namespace(move || {
        let mut taps = Vec::with_capacity(ifaces.len());
        for tap_builder in ifaces {
            taps.push(unwrap!(tap_builder.build_unbound()));
        }
        unwrap!(tx.send(taps));
        func()
    });

    let taps_unbound = unwrap!(rx.recv());
    let mut taps = Vec::with_capacity(taps_unbound.len());
    for tap_unbound in taps_unbound {
        taps.push(tap_unbound.bind(handle));
    }
    (join_handle, taps)
}

/// Spawn a function into a new network namespace with a single network interface behind a virtual
/// NAT. The returned `Gateway` can be used to read/write network activity from the public side of
/// the NAT.
pub fn spawn_behind_gateway<F, R>(
    handle: &Handle,
    func: F,
) -> (JoinHandle<R>, Gateway)
where
    R: Send + 'static,
    F: FnOnce() -> R + Send + 'static,
{
    let subnet = SubnetV4::random_local();
    let addr = subnet.random_client_addr();
    //let subnet = SubnetV4::local_10();
    //let addr = ipv4!("10.0.0.2");
    let mut tap_builder = TapBuilderV4::new();
    tap_builder.address(IfaceAddrV4 {
        address: addr,
        netmask: subnet.netmask(),
    });
    tap_builder.route(RouteV4::new(SubnetV4::global(), Some(subnet.gateway_addr())));

    let (join_handle, taps) = spawn_with_ifaces(handle, vec![tap_builder], func);
    let tap = unwrap!(taps.into_iter().next());
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
            let (join_handle, taps) = spawn_with_ifaces(&handle, vec![], || {
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
            tap_builder.address(IfaceAddrV4 {
                address: subnet.random_client_addr(),
                netmask: subnet.netmask(),
            });
            tap_builder.route(RouteV4 {
                destination: SubnetV4::new(ipv4!("0.0.0.0"), 0),
                gateway: subnet.gateway_addr(),
                use_gateway: true,
            });

            let payload: [u8; 8] = rand::random();
            let addr = addrv4!("1.2.3.4:56");
            trace!("spawning thread");
            let (join_handle, mut taps) = spawn_with_ifaces(&handle, vec![tap_builder], move || {
                let socket = unwrap!(std::net::UdpSocket::bind(addr!("0.0.0.0:0")));
                unwrap!(socket.send_to(&payload[..], SocketAddr::V4(addr)));
                trace!("sent udp packet");
            });
            let tap = unwrap!(taps.pop());

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
