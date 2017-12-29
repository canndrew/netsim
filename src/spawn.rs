use std::{mem, thread, panic, ptr};
use std::sync::{Arc, Mutex, Condvar};
use std::panic::AssertUnwindSafe;
use libc::{self, c_void, c_int};
use std::sync::mpsc;
use tokio_core::reactor::Handle;
use tap::{Tap, TapBuilderV4};

const STACK_SIZE: usize = 8 * 1024 * 1024;
const STACK_ALIGN: usize = 16;

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

    type CbData<R> = (Box<FnBox<R>>, Arc<JoinHandleInner<R>>);

    extern "C" fn clone_cb<R>(arg: *mut c_void) -> c_int {
        let data: *mut CbData<R> = arg as *mut _;
        let data: Box<CbData<R>> = unsafe { Box::from_raw(data) };
        let data = *data;
        let (func, inner) = data;

        let func = AssertUnwindSafe(func);
        let r = panic::catch_unwind(move || {
            let AssertUnwindSafe(func) = func;
            func.call_box()
        });

        let mut result = unwrap!(inner.result.lock());
        *result = Some(r);
        drop(result);
        inner.condvar.notify_one();
        0
    }

    let stack_head = ((stack_base as usize + STACK_SIZE + STACK_ALIGN) & !(STACK_ALIGN - 1)) as *mut c_void;
    let func = Box::new(func);
    let arg: Box<CbData<R>> = Box::new((func, inner_cloned));
    let arg = Box::into_raw(arg) as *mut c_void;
    
    let res = unsafe {
        libc::clone(clone_cb::<R>, stack_head, flags, arg)
    };
    assert!(res != -1);

    join_handle
}

impl<R> JoinHandle<R> {
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

pub fn spawn_with_ifaces<F, R>(
    handle: &Handle,
    ifaces: Vec<TapBuilderV4>,
    func: F,
) -> (JoinHandle<R>, Vec<Tap>)
where
    R: Send + 'static,
    F: FnOnce() -> R + Send + 'static,
{
    let remote = handle.remote().clone();
    let (tx, rx) = mpsc::channel();
    let join_handle = new_namespace(move || {
        let handle = unwrap!(remote.handle(), "no core is currently running!");
        let mut taps = Vec::with_capacity(ifaces.len());
        for tap_builder in ifaces {
            taps.push(unwrap!(tap_builder.build(&handle)));
        }
        unwrap!(tx.send(taps));
        func()
    });

    let taps = unwrap!(rx.recv());
    (join_handle, taps)
}

#[cfg(test)]
mod test {
    use super::*;
    use futures::{future, Future, Stream};
    use tokio_core::reactor::Core;
    use get_if_addrs;
    use void::ResultVoidExt;
    use tap::IfaceAddrV4;
    use std;
    use route::{SubnetV4, RouteV4};
    use ethernet::{EtherPayload};
    use ip::Ipv4Payload;
    use std::net::{SocketAddr, SocketAddrV4};
    use rand;
    use future_utils::StreamExt;

    #[test]
    fn test_no_network() {
        let mut core = unwrap!(Core::new());
        let handle = core.handle();
        let res = core.run(future::lazy(|| {
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
        let mut core = unwrap!(Core::new());
        let handle = core.handle();
        let res = core.run(future::lazy(|| {
            let mut tap_builder = TapBuilderV4::new();
            tap_builder.address(IfaceAddrV4::default());
            tap_builder.route(RouteV4 {
                destination: SubnetV4::new(ipv4!("0.0.0.0"), 0),
                gateway: ipv4!("0.0.0.0"),
            });

            let payload: [u8; 8] = rand::random();
            let addr = addrv4!("1.2.3.4:56");
            let (join_handle, mut taps) = spawn_with_ifaces(&handle, vec![tap_builder], move || {
                let socket = unwrap!(std::net::UdpSocket::bind(addr!("0.0.0.0:0")));
                unwrap!(socket.send_to(&payload[..], SocketAddr::V4(addr)));
            });
            let tap = unwrap!(taps.pop());

            tap
            .map_err(|e| panic!("error reading tap: {}", e))
            .filter_map(move |ether_frame| {
                match ether_frame.payload() {
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
                        Some(())
                    },
                    _ => None,
                }
            })
            .first_ok()
            .map_err(|_v| panic!("did not receive expected udp packet!"))
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
