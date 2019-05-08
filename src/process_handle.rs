use crate::priv_prelude::*;
use libc;

pub struct ProcessHandle {
    stack: Vec<u8>,
    child_tid: Box<libc::pid_t>,
}

impl ProcessHandle {
    pub fn new(stack: Vec<u8>, child_tid: Box<libc::pid_t>) -> ProcessHandle {
        ProcessHandle {
            stack,
            child_tid,
        }
    }

    pub fn busy_wait_for_exit(&mut self) {
        loop {
            if 0 == unsafe { ptr::read_volatile(&*self.child_tid) } {
                break;
            }
            thread::yield_now();
        }
        self.stack.clear();
    }
}

impl Drop for ProcessHandle {
    fn drop(&mut self) {
        // We should have called busy_wait_for_exit so the stack should already be deallocated.
        // This is just a safety net in case something goes badly wrong and we end up calling this
        // destuctor without exiting the subprocess somehow.
        let stack = mem::replace(&mut self.stack, Vec::new());
        mem::forget(stack);
    }
}

