use priv_prelude::*;
use sys;
use libc;
use libc::{c_int, c_void};
use spawn_complete;

const STACK_ALIGN: usize = 16;

trait FnBox<R> {
    fn call_box(self: Box<Self>) -> R;
}

impl<F, R> FnBox<R> for F
where
    F: FnOnce() -> R
{
    #[cfg_attr(feature="clippy", allow(boxed_local))]
    fn call_box(self: Box<Self>) -> R {
        (*self)()
    }
}

/// Run the function `func` in its own network namespace. This namespace will not have any network
/// interfaces. You can create virtual interfaces using `Tap`, or use one of the other functions in
/// the `spawn` module which do this for you.
pub fn new_namespace<F, R>(func: F) -> SpawnComplete<R>
where
    F: FnOnce() -> R,
    F: Send + 'static,
    R: Send + 'static,
{
    let stack_size = unsafe { sys::getpagesize() } as usize;
    let stack_size = cmp::max(stack_size, 4096);

    let mut stack = Vec::<u8>::with_capacity(stack_size + STACK_ALIGN);
    let stack_base = stack.as_mut_ptr();

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

    struct CbData<R: Send + 'static> {
        func: Box<FnBox<R> + Send + 'static>,
        ret_tx: oneshot::Sender<thread::Result<R>>,
        uid: u32,
        gid: u32,
    }
    
    extern "C" fn clone_cb<R: Send + 'static>(arg: *mut c_void) -> c_int {
        let data: *mut CbData<R> = arg as *mut _;
        let data: Box<CbData<R>> = unsafe { Box::from_raw(data) };
        //let data: *mut CbData = arg as *mut _;
        //let data: Box<CbData> = unsafe { Box::from_raw(data) };
        let data = *data;
        let CbData { func, ret_tx, uid, gid } = data;

        // WARNING: HACKERY
        // 
        // This should ideally be done without spawning another thread. We're already inside a
        // thread (spawned by clone), but that thread doesn't respect rust's thread-local
        // storage for some reason. So we spawn a thread in a thread in order to get our own
        // local storage keys. There should be a way to do this which doesn't involve spawning
        // two threads and letting one of them die. This would require going back to crafting
        // our own `JoinHandle` though.

        let mut f = unwrap!(File::create("/proc/self/uid_map"));
        let s = format!("0 {} 1", uid);
        unwrap!(f.write(s.as_bytes()));

        // TODO: set gids correctly in the namespace
        let _gid = gid;
        //let mut f = unwrap!(File::create("/proc/self/gid_map"));
        //let s = format!("0 {} 1", gid);
        //unwrap!(f.write(s.as_bytes()));

        let _joiner = thread::spawn(move || {
            let func = panic::AssertUnwindSafe(func);
            let ret = panic::catch_unwind(move || {
                let panic::AssertUnwindSafe(func) = func;
                func.call_box()
            });
            let _ = ret_tx.send(ret);
        });
        0
    }

    let uid = unsafe { sys::geteuid() };
    let gid = unsafe { sys::getegid() };
    let (ret_tx, ret_rx) = oneshot::channel();
    let stack_head = ((stack_base as usize + stack_size + STACK_ALIGN) & !(STACK_ALIGN - 1)) as *mut c_void;
    let func = Box::new(func);
    let arg: Box<CbData<R>> = Box::new(CbData { func, ret_tx, uid, gid });
    let arg = Box::into_raw(arg) as *mut c_void;
    
    let pid = unsafe {
        libc::clone(clone_cb::<R>, stack_head, flags, arg)
    };
    if pid == -1 {
        let err = io::Error::last_os_error();
        if err.kind() == io::ErrorKind::PermissionDenied {
            let mut utsname: sys::utsname = unsafe { mem::zeroed() };
            let res = unsafe {
                sys::uname(&mut utsname)
            };
            assert_eq!(res, 0);
            let version = unsafe {
                CStr::from_ptr(utsname.release.as_ptr())
            };
            let version = unwrap!(version.to_str());
            let passwd = unsafe {
                sys::getpwuid(uid)
            };
            let user_name = unsafe {
                CStr::from_ptr((*passwd).pw_name)
            };
            let user_name = unwrap!(user_name.to_str());
            let group = unsafe {
                sys::getgrgid(gid)
            };
            let group_name = unsafe {
                CStr::from_ptr((*group).gr_name)
            };
            let group_name = unwrap!(group_name.to_str());

            panic!(
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
        }
        panic!("failed to spawn thread: {}", err);
    }
    let res = unsafe {
        sys::waitpid(pid, ptr::null_mut(), 0)
    };
    if res != -1 {
        panic!("unexpected result from waitpid(): {}", res);
    }
    let err = io::Error::last_os_error();
    if err.raw_os_error() != Some(sys::ECHILD as i32) {
        panic!("unexpected error from waitpid(): {}", err);
    }

    spawn_complete::from_receiver(ret_rx)
}

#[cfg(test)]
mod test {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn respects_thread_local_storage() {
        run_test(3, || {
            thread_local! {
                static TEST: Cell<u32> = Cell::new(0);
            };

            TEST.with(|v| v.set(123));
            let spawn_complete = new_namespace(|| {
                TEST.with(|v| {
                    assert_eq!(v.get(), 0);
                    v.set(456);
                });
            });
            let mut core = unwrap!(Core::new());
            unwrap!(core.run(spawn_complete));
            TEST.with(|v| assert_eq!(v.get(), 123));
        })
    }
}

