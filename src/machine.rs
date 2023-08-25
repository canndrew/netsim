use crate::priv_prelude::*;

pub struct Machine {
    #[allow(dead_code)]
    join_handle: namespace::JoinHandle<()>,
    task_tx_opt: Option<mpsc::UnboundedSender<Pin<Box<dyn Future<Output = ()> + Send + 'static>>>>,
}

pub struct JoinHandle<R> {
    ret_rx: oneshot::Receiver<thread::Result<R>>,
}

impl Machine {
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
                    let _ = tokio::spawn(task);
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

    pub async fn spawn<F, R>(&self, future: F) -> JoinHandle<R>
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
    pub async fn join(self) -> thread::Result<Option<R>> {
        match self.ret_rx.await {
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

