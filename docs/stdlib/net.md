# net: TCP sockets

**Status in 2.1.0.** Implemented in `stdlib/src/net.ls`. The module provides
plain, blocking TCP over addresses as text: `bind` and `connect` take a
`"host:port"` string. There is no TLS, no UDP, no DNS lookup API beyond the
resolver built into `connect`, and no async I/O in 2.1.0.

Failures are [`io::Error`](io.md) values, never panics. A refused connection, a
reset peer, an address already in use and a timeout are reported with the
matching `ErrorKind`: `ConnectionRefused`, `ConnectionReset`, `AddrInUse`,
`TimedOut`, `WouldBlock`, and so on.

## TcpListener

```rust
pub struct TcpListener {
    handle: i64,
}
```

| Method | Signature | Notes |
| --- | --- | --- |
| `TcpListener::bind` | `(address: &str) -> Result<TcpListener, Error>` | binds and starts listening; a port of `0` in the address picks a free port |
| `local_port` | `(&self) -> i64` | the port the listener is bound to |
| `accept` | `(&self) -> Result<TcpStream, Error>` | waits for the next connection |
| `handle` | `(&self) -> i64` | the raw runtime table index |
| `TcpListener` `Drop` | - | closes the listener |

`bind` is the only constructor and there is no `set_nonblocking` on a listener;
a timeout is set on accepted streams instead.

## TcpStream

```rust
pub struct TcpStream {
    handle: i64,
}
```

| Method | Signature | Notes |
| --- | --- | --- |
| `TcpStream::connect` | `(address: &str) -> Result<TcpStream, Error>` | opens a connection to `"host:port"` |
| `handle` | `(&self) -> i64` | the raw runtime table index |
| `read` | `(&mut self, buffer: &mut [u8]) -> Result<usize, Error>` | bytes read; `0` when the peer closed the connection |
| `write` | `(&mut self, buffer: &[u8]) -> Result<usize, Error>` | bytes written |
| `set_timeout` | `(&self, milliseconds: i64) -> Result<(), Error>` | sets the read and write timeout; `0` disables it |
| `shutdown` | `(&self, how: Shutdown) -> Result<(), Error>` | shuts down the read side, the write side or both |
| `TcpStream` `Drop` | - | shuts both directions down and closes the socket |

`shutdown` takes the `Shutdown` enum added in 2.1.0, replacing the numeric
`how` argument of 2.0.0:

```rust
pub enum Shutdown {
    Read,
    Write,
    Both,
}
```

The byte counts are `usize`, as in [io](io.md). Dropping a stream shuts both
directions down and then closes the socket; dropping a listener closes it. A
peer that closes the connection is not an error: `read` returns `Ok(0)`, and a
later `write` reports `ErrorKind::BrokenPipe`.

Sending and receiving use byte buffers. `Vec<u8>` is the only growable byte
container in 2.1.0 (there are no arrays), and `&mut buffer` coerces to
`&mut [u8]` when passed to `read`:

```rust
let mut buffer: Vec<u8> = Vec::with_capacity(1024);
stream.read(&mut buffer)?;
stream.write(&buffer[0..count])?;
```

## Limits in 2.1.0

The module is TCP only: no UDP, no Unix domain sockets, no TLS, no `SocketAddr`
type, no DNS lookup API, no listener iterator and no async I/O. Addresses are
strings resolved by the runtime. There is no `set_nonblocking` on sockets and no
half-close inspection.

## See also

* [io](io.md) - errors and handles.
* [Standard library chapter](../language/15-standard-library.md#io-and-process).
