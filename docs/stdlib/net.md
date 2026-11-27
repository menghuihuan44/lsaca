# net: TCP and UDP sockets

**Status in 2.4.0.** Implemented in `stdlib/src/net.ls`. The module provides
plain, blocking TCP (since 2.0.0) and UDP (since 2.4.0), with addresses as text:
`bind`, `connect`, `send_to` and `recv_from` take `"host:port"` strings. There is
no TLS, no Unix-domain socket API, no dedicated DNS lookup API beyond the
resolver used by the socket calls, and no asynchronous I/O.

Failures are [`io::Error`](io.md) values, never panics. A refused connection, a
reset peer, an address already in use and a timeout are reported with the
matching `ErrorKind`: `ConnectionRefused`, `ConnectionReset`, `AddrInUse`,
`TimedOut`, `WouldBlock`, and so on.

## SocketAddr

`SocketAddr` is a parsed IP address and port. It is a syntactic value: parsing
does not perform a DNS lookup, and a host name is rejected because an address is
not a name.

```rust
pub struct SocketAddr {
    ip: String,
    port: i64,
}
```

| Method | Signature | Notes |
| --- | --- | --- |
| `SocketAddr::parse` | `(text: &str) -> Result<SocketAddr, Error>` | parses `IP:port`; IPv4 is `1.2.3.4:80`, IPv6 is bracketed as `[::1]:80`; surrounding whitespace is ignored; a host name is rejected |
| `ip` | `(&self) -> String` | the IP address text, without brackets |
| `port` | `(&self) -> i64` | the port number, `0` through `65535` |
| `to_string` | `(&self) -> String` | formats the address; an IPv6 host is bracketed |
| `display` | `(&self) -> String` | alias for `to_string` |

An invalid address, an empty host or port, a port outside `0..=65535`, or a
bracketed IPv6 address whose syntax is invalid produces `ErrorKind::InvalidInput`.
The numeric address is not canonicalised: the parsed text is what `ip` returns.
The socket methods still accept address strings; a `SocketAddr` is a separate
value, not an overload of those methods.

## UdpSocket

```rust
pub struct UdpSocket {
    handle: i64,
}
```

| Method | Signature | Notes |
| --- | --- | --- |
| `UdpSocket::bind` | `(address: &str) -> Result<UdpSocket, Error>` | binds a local address; a port of `0` asks for an ephemeral port |
| `local_port` | `(&self) -> i64` | the local port the socket is bound to |
| `handle` | `(&self) -> i64` | the raw runtime table index |
| `send_to` | `(&self, bytes: &[u8], address: &str) -> Result<usize, Error>` | sends one datagram and returns the number of bytes sent |
| `recv_from` | `(&mut self, max: i64) -> Result<Option<(Vec<u8>, String)>, Error>` | receives one datagram of at most `max` bytes, returning its payload and peer address |
| `set_timeout` | `(&self, milliseconds: i64) -> Result<(), Error>` | sets the receive timeout; `0` disables it |
| `close` | `(&mut self) -> Result<(), Error>` | closes the socket; closing an already closed socket is `Ok(())` |
| `UdpSocket` `Drop` | - | closes the socket if it is still open |

`bind` is the only constructor. It returns an error instead of panicking when
the address cannot be bound, matching `TcpListener`. Each `send_to` produces one
datagram; UDP does not add message boundaries beyond that. A received datagram
larger than `max` is truncated to the buffer, and a negative `max` is
`ErrorKind::InvalidInput`.

When a timeout was set with `set_timeout` and no datagram arrives in time,
`recv_from` returns `Ok(None)`; other receive failures are `Err`. The peer
address is formatted as text (`1.2.3.4:80`, or `[::1]:80` for IPv6).
`close` is idempotent, and dropping a socket closes it as well.

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

Sending and receiving use byte buffers. A `Vec<u8>` gives a growable buffer, and
since 2.2.0 a fixed-size array `[u8; N]` works too; `&mut buffer` coerces to
`&mut [u8]` when passed to `read`:

```rust
let mut buffer: Vec<u8> = Vec::with_capacity(1024);
stream.read(&mut buffer)?;
stream.write(&buffer[0..count])?;
```

## Limits in 2.4.0

The module has TCP and UDP only: no Unix-domain sockets, no TLS, no dedicated
DNS lookup API, no listener iterator and no asynchronous I/O. There is no
`set_nonblocking` and no multicast or broadcast control; the only socket option
is the timeout. Addresses are strings resolved by the runtime, and UDP methods
take peer addresses as strings rather than `SocketAddr` values.

## See also

* [io](io.md) - errors and handles.
* [Standard library chapter](../language/15-standard-library.md#io-and-process).
