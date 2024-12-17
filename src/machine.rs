use crate::priv_prelude::*;

/// The entry point for this library.
///
/// A machine has its own tokio runtime running in a separate network-isolated thread. Use
/// [`spawn`](crate::Machine::spawn) to spawn futures on this runtime and wait for their result.
/// Use [`add_ip_iface`](crate::Machine::add_ip_iface) to add virtual network interfaces to the
/// machine and use the returned [`IpIface`](crate::IpIface) to interfere with its packets or build
/// it into a network. Dropping the machine will shutdown its runtime and cancel any futures
/// executing on it, causing any associated [`JoinHandle`](crate::JoinHandle)s to return `None`.
pub struct Machine {
    #[allow(dead_code)]
    join_handle: namespace::JoinHandle<()>,
    task_tx_opt: Option<mpsc::UnboundedSender<Pin<Box<dyn Future<Output = ()> + Send + 'static>>>>,
}

/// A handle to a future executing on a [`Machine`](crate::Machine).
///
/// You can await a `JoinHandle` (or call [`join`](crate::JoinHandle::join)) to get the future's
/// result.
pub struct JoinHandle<R> {
    ret_rx: oneshot::Receiver<thread::Result<R>>,
}

impl Machine {
    /// Create a new machine. A machine initially has no network interfaces, so attempting to (eg.)
    /// make an outgoing TCP connection from a future running on the machine will fail.
    pub fn new() -> io::Result<Machine> {
        let (task_tx, mut task_rx) = mpsc::unbounded::<Pin<Box<dyn Future<Output = ()> + Send + 'static>>>();
        let (startup_tx, startup_rx) = oneshot::channel();
        let join_handle_res = namespace::spawn(move || {
            let runtime_res = tokio::runtime::Runtime::new();
            let runtime = match runtime_res {
                Ok(runtime) => {
                    let _ = startup_tx.send(Ok(()));
                    runtime
                },
                Err(err) => {
                    let _ = startup_tx.send(Err(err));
                    return;
                },
            };
            runtime.block_on(async move {
                while let Some(task) = task_rx.next().await {
                    let _detach = tokio::spawn(task);
                }
            });
            runtime.shutdown_background();
        });
        let join_handle = join_handle_res?;

        let () = startup_rx.recv().unwrap()?;
        let task_tx_opt = Some(task_tx);
        let machine = Machine { join_handle, task_tx_opt };
        Ok(machine)
    }

    /// Executes a future on the machine. The future will start executing immediately. You can use
    /// the returned [`JoinHandle`](crate::JoinHandle) to await the future's result.
    pub fn spawn<F, R>(&self, future: F) -> JoinHandle<R>
    where
        F: Future<Output = R> + Send + 'static,
        R: Send + 'static,
    {
        let (ret_tx, ret_rx) = oneshot::channel();
        let task = Box::pin(async move {
            let ret = panic::AssertUnwindSafe(future).catch_unwind().await;
            let _ = ret_tx.send(ret);
        });
        self.task_tx_opt.as_ref().unwrap().unbounded_send(task).unwrap();
        JoinHandle { ret_rx }
    }

    /// Adds a network interface to the machine. See the [`IpIfaceBuilder`](crate::IpIfaceBuilder)
    /// details.
    pub fn add_ip_iface(&self) -> IpIfaceBuilder<'_> {
        IpIfaceBuilder::new(self)
    }
}

impl Drop for Machine {
    fn drop(&mut self) {
        let _task_tx = self.task_tx_opt.take();
    }
}

impl<R> JoinHandle<R> {
    /// Wait for the future executing on the machine to complete and get its result.
    ///
    /// # Returns
    ///
    /// * `Ok(Some(value))` if the future completed normally and returned `value`.
    /// * `Ok(None)` if the [`Machine`](crate::Machine) was dropped before the future completed.
    /// * `Err(panic_error)` if the future panicked.
    pub async fn join(self) -> thread::Result<Option<R>> {
        match self.ret_rx.await {
            Ok(Ok(val)) => Ok(Some(val)),
            Ok(Err(err)) => Err(err),
            Err(_recv_err) => Ok(None),
        }
    }

    /// Block the current thread, wait for the future executing on the machine the complete and get
    /// its result.
    ///
    /// # Returns
    ///
    /// * `Ok(Some(value))` if the future completed normally and returned `value`.
    /// * `Ok(None)` if the [`Machine`](crate::Machine) was dropped before the future completed.
    /// * `Err(panic_error)` if the future panicked.
    pub fn join_blocking(self) -> thread::Result<Option<R>> {
        match self.ret_rx.recv() {
            Ok(Ok(val)) => Ok(Some(val)),
            Ok(Err(err)) => Err(err),
            Err(_recv_err) => Ok(None),
        }
    }
}

impl<R> IntoFuture for JoinHandle<R>
where
    R: Send + 'static,
{
    type Output = thread::Result<Option<R>>;
    type IntoFuture = Pin<Box<dyn Future<Output = thread::Result<Option<R>>> + Send + 'static>>;

    fn into_future(self) -> Pin<Box<dyn Future<Output = thread::Result<Option<R>>> + Send + 'static>> {
        Box::pin(self.join())
    }
}

