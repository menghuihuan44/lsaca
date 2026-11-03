# net: TCP networking

**Status in 2.0.0.** Implemented. The module is implemented in `stdlib/src/net.ls`
and provides plain TCP only: no TLS, no UDP, no Unix-domain sockets and no
async I/O.

Addresses are owned `String` values in the form `"host:port"`, for example
`"127.0.0.1:8080"`. A port of `0` asks the operating system for an ephemeral
port, which `local_port` reports. Failures are `io::Error` values with an
`ErrorKind`, never panics.

## TcpListener

```rust
pub struct TcpListener {
    handle: i64,
}
```

| Method | Signature | Meaning |
| --- | --- | --- |
| `TcpListener::bind` | `(String) -> Result<TcpListener, io::Error>` | binds a listening socket to `"host:port"` |
| `local_port` | `(&self) -> i64` | the bound port (useful after binding port `0`) |
| `accept` | `(&self) -> Result<TcpStream, io::Error>` | waits for and returns the next connection |
| `handle` | `(&self) -> i64` | the raw runtime handle |
| `Drop` | - | closes the listening socket |

`accept` blocks the calling thread. On a non-blocking listener with no pending
connection the error kind is `ErrorKind::WouldBlock`; make the listener
non-blocking through the platform handle if you need polling.

## TcpStream

```rust
pub struct TcpStream {
    handle: i64,
}
```

| Method | Signature | Meaning |
| --- | --- | --- |
| `TcpStream::connect` | `(String) -> Result<TcpStream, io::Error>` | connects to `"host:port"` |
| `handle` | `(&self) -> i64` | the raw runtime handle |
| `read` | `(&mut self, &mut [u8]) -> Result<i64, io::Error>` | reads bytes; `0` at end of stream |
| `write` | `(&mut self, &[u8]) -> Result<i64, io::Error>` | writes bytes |
| `set_timeout` | `(&self, milliseconds: i64) -> Result<(), io::Error>` | read and write timeout in milliseconds; `0` disables it |
| `shutdown` | `(&self, how: i64) -> Result<(), io::Error>` | shuts down `0` read, `1` write or `2` both directions |
| `Drop` | - | shuts down both directions and closes the socket |

Failures are classified: a refused connection is
`ErrorKind::ConnectionRefused`, a reset connection is
`ErrorKind::ConnectionReset`, an address already in use is
`ErrorKind::AddressInUse` and a timeout is `ErrorKind::TimedOut`.

## Limits in 2.0

Plain TCP only: no TLS, no UDP, no Unix-domain sockets and no asynchronous I/O.
There is no DNS API; use a numeric address or `localhost`. There are no threads
in 2.0.0, so a server handles one connection at a time unless it multiplexes
manually.

## See also

* [io](io.md), [fs](fs.md) and [error](error.md).
* [Roadmap](../language/16-roadmap.md) - threads are not part of 2.0.
