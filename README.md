# netsim

`netsim` is a Rust library which allows you to:

* Run tests in network-isolated threads.
* Test networking code on simulated networks.
* Capture and inspect packets produced by your code.
* Inject and meddle with network packets.

## Examples

See the examples directory in this repo.

## Limitations

`netsim` currently only supports Linux since it makes use of the Linux containerization APIs.

## License

MIT or BSD-3-Clause at your option.

