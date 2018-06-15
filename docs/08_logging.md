# Logging

Sometimes we need to debug something or to better understand what is going on
under the hood. netsim uses [Rust log](https://docs.rs/log/) and writes some
helpful messages.

The easiest way to start with logger is to use
[env_logger](https://crates.io/crates/env_logger) which is configured via
environment variable `RUST_LOG` and by default logs to stderr.

Add this to you Cargo.toml:

```toml
[dependencies]
env_logger = "0.4.0"
```

and this to your main.rs/lib.rs:

```rust
extern crate env_logger;

fn main() {
    ::env_logger::init().unwrap();

    //...
}
```

When you run the example prefix the command with `RUST_LOG`:

```shell
$ RUST_LOG=netsim=debug cargo run --example full_cone_nat
```

This will only log messages coming from netsim crate. You can also specify the
verbosity level. For possible values see
[log::Level](https://docs.rs/log/0.4.2/log/enum.Level.html).
