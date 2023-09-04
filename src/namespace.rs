use crate::priv_prelude::*;

const STACK_ALIGN: usize = 16;

pub struct JoinHandle<R> {
    stack_ptr: *mut [u8],
    child_tid: *mut libc::pid_t,
    ret_rx_opt: Option<oneshot::Receiver<thread::Result<R>>>,
}

unsafe impl<R: Send> Send for JoinHandle<R> {}
unsafe impl<R> Sync for JoinHandle<R> {}

impl<R> JoinHandle<R> {
    fn join_inner(&mut self, ret_rx: oneshot::Receiver<thread::Result<R>>) -> thread::Result<R> {
        let ret = ret_rx.recv().expect("sender is never dropped without sending");
        loop {
            if 0 == unsafe { ptr::read_volatile(self.child_tid) } {
                break;
            }
            thread::yield_now();
        }
        let _child_tid = unsafe { Box::from_raw(self.child_tid) };
        let _stack_ptr = unsafe { Box::from_raw(self.stack_ptr) };
        ret
    }

    #[allow(unused)]
    pub fn join(mut self) -> thread::Result<R> {
        let ret_rx = self.ret_rx_opt.take().expect("join_inner has not already been called");
        self.join_inner(ret_rx)
    }
}

impl<R> Drop for JoinHandle<R> {
    fn drop(&mut self) {
        if let Some(ret_rx) = self.ret_rx_opt.take() {
            let _ret = self.join_inner(ret_rx);
        }
    }
}

pub fn spawn<F, R>(func: F) -> io::Result<JoinHandle<R>>
where
    F: FnOnce() -> R,
    F: Send + 'static,
    R: Send + 'static,
{
    let stack_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) } as usize;
    let stack_size = cmp::max(stack_size, 4096);

    let mut stack = vec![0u8; stack_size + STACK_ALIGN];
    let stack_base = stack.as_mut_ptr();
    let stack = stack.into_boxed_slice();
    let stack_ptr = Box::into_raw(stack);

    let flags =
        libc::CLONE_CHILD_CLEARTID |
        libc::CLONE_FILES |
        libc::CLONE_IO |
        libc::CLONE_SIGHAND |
        libc::CLONE_VM |
        libc::CLONE_SYSVSEM |
        //libc::CLONE_THREAD;
        libc::CLONE_NEWNET |
        libc::CLONE_NEWUTS |
        libc::CLONE_NEWUSER;

    struct CbData<R: Send + 'static> {
        func: Box<dyn FnOnce() -> R + Send + 'static>,
        ret_rx_tx: oneshot::Sender<io::Result<oneshot::Receiver<thread::Result<R>>>>,
        uid: u32,
        gid: u32,
    }

    extern "C" fn clone_cb<R: Send + 'static>(arg: *mut c_void) -> c_int {
        let res = panic::catch_unwind(panic::AssertUnwindSafe(move || {
            let data: *mut CbData<R> = arg as *mut _;
            let data: Box<CbData<R>> = unsafe { Box::from_raw(data) };
            let data = *data;
            let CbData { func, ret_rx_tx, uid, gid } = data;

            // WARNING: HACKERY
            //
            // This should ideally be done without spawning another thread. We're already inside a
            // thread (spawned by clone), but that thread doesn't respect rust's thread-local
            // storage. Making it do so would require passing CLONE_SETTLS to clone(), but it's not
            // clear what tls argument to pass in because it's (I think?) dependent on the
            // implementation of std::thread::spawn. So instead we use std::thread::spawn to spawn
            // a new sub-thread inside our clone thread in order to get thread-local storage
            // working again.
            //
            // Note that until we're inside the sub-thread things are pretty fragile. For instance
            // println! can randomly crash in flames if called outside the sub-thread. But the code
            // currently here seems to work.

            let setup = move || {
                let res = unsafe {
                    #[cfg_attr(feature="cargo-clippy", allow(clippy::unnecessary_cast))]
                    libc::prctl(libc::PR_SET_PDEATHSIG as i32, libc::SIGTERM, 0, 0, 0)
                };
                if res == -1 {
                    let err = io::Error::last_os_error();
                    return Err(err);
                }

                let mut f = File::create("/proc/self/uid_map")?;
                let s = format!("0 {} 1\n", uid);
                let n = f.write(s.as_bytes())?;
                assert_eq!(n, s.len());

                let mut f = File::create("/proc/self/setgroups")?;
                let s = "deny\n";
                let n = f.write(s.as_bytes())?;
                assert_eq!(n, s.len());

                let mut f = File::create("/proc/self/gid_map")?;
                let s = format!("0 {} 1\n", gid);
                let n = f.write(s.as_bytes())?;
                assert_eq!(n, s.len());

                Ok(())
            };
            let ret_tx = match setup() {
                Ok(()) => {
                    let (ret_tx, ret_rx) = oneshot::channel();
                    let _ = ret_rx_tx.send(Ok(ret_rx));
                    ret_tx
                },
                Err(err) => {
                    let _ = ret_rx_tx.send(Err(err));
                    return;
                },
            };

            let joiner = thread::spawn(move || {
                let ret = panic::catch_unwind(panic::AssertUnwindSafe(func));
                let _ = ret_tx.send(ret);
            });
            let _ = joiner.join();
        }));
        match res {
            Ok(()) => 0,
            Err(_err) => std::process::exit(1),
        }
    }

    let uid = unsafe { libc::geteuid() };
    let gid = unsafe { libc::getegid() };
    let (ret_rx_tx, ret_rx_rx) = oneshot::channel();
    let stack_head = ((stack_base as usize + stack_size + STACK_ALIGN) & !(STACK_ALIGN - 1)) as *mut c_void;
    let func = Box::new(func);
    let arg: Box<CbData<R>> = Box::new(CbData { func, ret_rx_tx, uid, gid });
    let arg = Box::into_raw(arg) as *mut c_void;
    let child_tid = Box::new(!0);
    let child_tid = Box::into_raw(child_tid);

    let pid = unsafe {
        libc::clone(
            clone_cb::<R>,
            stack_head,
            flags,
            arg,
            ptr::null::<pid_t>(),
            ptr::null::<c_void>(),
            child_tid,
        )
    };
    if pid == -1 {
        let err = io::Error::last_os_error();
        if err.kind() == io::ErrorKind::PermissionDenied {
            let utsname = {
                let mut utsname = MaybeUninit::uninit();
                let res = unsafe {
                    libc::uname(utsname.as_mut_ptr())
                };
                assert_eq!(res, 0);
                unsafe { utsname.assume_init() }
            };
            let version = unsafe {
                CStr::from_ptr(utsname.release.as_ptr())
            };
            let version = version.to_str().unwrap();
            let passwd = unsafe {
                libc::getpwuid(uid)
            };
            let user_name = unsafe {
                CStr::from_ptr((*passwd).pw_name)
            };
            let user_name = user_name.to_str().unwrap();
            let group = unsafe {
                libc::getgrgid(gid)
            };
            let group_name = unsafe {
                CStr::from_ptr((*group).gr_name)
            };
            let group_name = group_name.to_str().unwrap();

            let msg = format!(
                "\
                Failed to call clone(CLONE_NEWUSER | CLONE_NEWNET) (permission denied). \
                Your kernel may be too old. \
                Version >= 3.8 is required, your version is {}. \
                Your user/group must also be valid (not nobody). \
                Your user == {}, group == {}. \
                You cannot use netsim in a chroot.\
                ",
                version, user_name, group_name,
            );
            let err = io::Error::new(io::ErrorKind::PermissionDenied, msg);
            return Err(err);
        }
        return Err(err);
    }
    let ret_rx_res = ret_rx_rx.recv().expect("spawned thread sends its return channel");
    let ret_rx_opt = Some(ret_rx_res?);

    Ok(JoinHandle {
        stack_ptr, child_tid, ret_rx_opt,
    })
}

