use {
    std::{
        thread,
        net::{TcpStream, TcpListener},
    },
    net_literals::addrv4,
};

#[netsim::isolate]
fn main() {
    let addr = addrv4!("127.0.0.1:80");

    let listener = TcpListener::bind(addr).unwrap();
    let join_handle_inner_0 = thread::spawn(move || {
        let _stream = listener.accept().unwrap();
    });
    let join_handle_inner_1 = thread::spawn(move || {
        let _stream = TcpStream::connect(addr).unwrap();
    });
    join_handle_inner_0.join().unwrap();
    join_handle_inner_1.join().unwrap();
}

