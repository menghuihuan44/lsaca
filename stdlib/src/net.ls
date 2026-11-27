// TCP and UDP networking.
//
// Addresses are read-only text (`&str`) in the form "host:port" (for example
// "127.0.0.1:8080"); a port of 0 asks the operating system for an ephemeral
// port, which local_port reports. Failures are reported as io::Error values,
// never as panics. Listener, stream and socket handles are closed when the
// value is dropped.
//
// v2.4 adds SocketAddr and UdpSocket. TCP behaviour is unchanged.
//
// Since 2.1.0 read and write return the number of bytes transferred as usize
// and shutdown takes a Shutdown value instead of the raw numeric code.

use crate::io::ErrorKind;

pub extern fn __net_listen(address: &str) -> i64;
pub extern fn __net_local_port(handle: i64) -> i64;
pub extern fn __net_accept(listener: i64) -> i64;
pub extern fn __net_connect(address: &str) -> i64;
pub extern fn __net_set_timeout(handle: i64, milliseconds: i64) -> i64;
pub extern fn __net_shutdown(handle: i64, how: i64) -> i64;
pub extern fn __io_close(handle: i64) -> i64;
pub extern fn __io_read(handle: i64, buffer: &mut [u8]) -> i64;
pub extern fn __io_write(handle: i64, buffer: &[u8]) -> i64;

/// The direction(s) of a connection to shut down.
pub enum Shutdown {
    /// Shut down the read half.
    Read,
    /// Shut down the write half.
    Write,
    /// Shut down both halves.
    Both,
}

impl Shutdown {
    /// The frozen runtime code for this direction: 0 read, 1 write, 2 both.
    fn code(self) -> i64 {
        match self {
            Shutdown::Read => {
                return 0;
            }
            Shutdown::Write => {
                return 1;
            }
            Shutdown::Both => {
                return 2;
            }
        }
    }
}

/// A listening TCP socket.
pub struct TcpListener {
    handle: i64,
}

impl TcpListener {
    /// Binds a listener to an address such as "127.0.0.1:0".
    pub fn bind(address: &str) -> Result<TcpListener, Error> {
        let handle: i64 = __net_listen(address);
        if handle < 0 {
            return Err(Error::last());
        }
        return Ok(TcpListener { handle: handle });
    }

    /// The local port the listener is bound to.
    pub fn local_port(&self) -> i64 {
        return __net_local_port(self.handle);
    }

    /// Accepts the next pending connection.
    ///
    /// On a non blocking listener with no pending connection the error kind
    /// is ErrorKind::WouldBlock and the caller may retry.
    pub fn accept(&self) -> Result<TcpStream, Error> {
        let handle: i64 = __net_accept(self.handle);
        if handle < 0 {
            return Err(Error::last());
        }
        return Ok(TcpStream { handle: handle });
    }

    /// The raw runtime handle of this listener.
    pub fn handle(&self) -> i64 {
        return self.handle;
    }
}

impl Drop for TcpListener {
    fn drop(&mut self) {
        __io_close(self.handle);
    }
}

/// A connected TCP stream.
pub struct TcpStream {
    handle: i64,
}

impl TcpStream {
    /// Connects to an address such as "127.0.0.1:8080".
    pub fn connect(address: &str) -> Result<TcpStream, Error> {
        let handle: i64 = __net_connect(address);
        if handle < 0 {
            return Err(Error::last());
        }
        return Ok(TcpStream { handle: handle });
    }

    /// The raw runtime handle of this stream.
    pub fn handle(&self) -> i64 {
        return self.handle;
    }

    /// Reads up to buffer.len() bytes, returning the number read.
    pub fn read(&mut self, buffer: &mut [u8]) -> Result<usize, Error> {
        let count: i64 = __io_read(self.handle, buffer);
        if count < 0 {
            return Err(Error::last());
        }
        return Ok(count as usize);
    }

    /// Writes the whole buffer, returning the number of bytes written.
    pub fn write(&mut self, buffer: &[u8]) -> Result<usize, Error> {
        let count: i64 = __io_write(self.handle, buffer);
        if count < 0 {
            return Err(Error::last());
        }
        return Ok(count as usize);
    }

    /// Sets a read and write timeout in milliseconds (0 disables it).
    pub fn set_timeout(&self, milliseconds: i64) -> Result<(), Error> {
        let status: i64 = __net_set_timeout(self.handle, milliseconds);
        if status < 0 {
            return Err(Error::last());
        }
        return Ok(());
    }

    /// Shuts down the connection.
    pub fn shutdown(&self, how: Shutdown) -> Result<(), Error> {
        let status: i64 = __net_shutdown(self.handle, how.code());
        if status < 0 {
            return Err(Error::last());
        }
        return Ok(());
    }
}

impl Drop for TcpStream {
    fn drop(&mut self) {
        __net_shutdown(self.handle, 2);
        __io_close(self.handle);
    }
}

// v2.4 UDP surface.

pub extern fn __net_udp_bind(address: &str) -> i64;
pub extern fn __net_udp_send_to(handle: i64, bytes: &[u8], address: &str) -> i64;
pub extern fn __net_udp_recv_from(handle: i64, buffer: &mut [u8]) -> (i64, String);

/// A parsed network address (IP address and port).
///
/// IPv4 addresses are written `1.2.3.4:80`; IPv6 addresses are bracketed as
/// `[::1]:80`. The address is a syntactic value only: it is resolved when it
/// is passed to a socket operation.
pub struct SocketAddr {
    ip: String,
    port: i64,
}

impl SocketAddr {
    /// Parses `IP:PORT`. A bracketed IPv6 host is accepted; a host name is
    /// rejected because a SocketAddr names an address, not a DNS lookup.
    pub fn parse(text: &str) -> Result<SocketAddr, Error> {
        let value: &str = text.trim();
        if value.is_empty() {
            return Err(__net_address_error("the socket address is empty"));
        }
        if value.starts_with("[") {
            let close: usize = match value.find("]") {
                Some(index) => index,
                None => {
                    return Err(__net_address_error("the IPv6 address is missing its closing bracket"));
                }
            };
            if close <= 1 {
                return Err(__net_address_error("the IPv6 address host is empty"));
            }
            let host: &str = value.slice(1, close);
            let rest: &str = value.slice(close + 1, value.len());
            if !rest.starts_with(":") {
                return Err(__net_address_error("the socket address is missing its port"));
            }
            if !__net_is_ipv6_text(host) {
                return Err(__net_address_error("the IPv6 address is invalid"));
            }
            let port: i64 = __net_parse_port(rest.slice(1, rest.len()))?;
            return Ok(SocketAddr {
                ip: String::from_str(host),
                port: port,
            });
        }
        if value.contains("[") || value.contains("]") {
            return Err(__net_address_error("the socket address brackets are invalid"));
        }
        let colon: usize = match value.find(":") {
            Some(index) => index,
            None => {
                return Err(__net_address_error("the socket address is missing its port"));
            }
        };
        let host: &str = value.slice(0, colon);
        let port_text: &str = value.slice(colon + 1, value.len());
        if host.is_empty() {
            return Err(__net_address_error("the socket address host is empty"));
        }
        if port_text.contains(":") {
            return Err(__net_address_error(
                "an IPv6 address must be enclosed in brackets",
            ));
        }
        if !__net_is_ipv4_text(host) {
            return Err(__net_address_error("the IPv4 address is invalid"));
        }
        let port: i64 = __net_parse_port(port_text)?;
        return Ok(SocketAddr {
            ip: String::from_str(host),
            port: port,
        });
    }

    /// The IP address text, without brackets.
    pub fn ip(&self) -> String {
        return self.ip.clone();
    }

    /// The port number.
    pub fn port(&self) -> i64 {
        return self.port;
    }

    /// Formats the address; IPv6 hosts are bracketed.
    pub fn to_string(&self) -> String {
        if self.ip.contains(":") {
            return format!("[{}]:{}", self.ip.as_str(), self.port);
        }
        return format!("{}:{}", self.ip.as_str(), self.port);
    }

    /// Alias for `to_string`, matching the rest of the standard library.
    pub fn display(&self) -> String {
        return self.to_string();
    }
}

fn __net_address_error(message: &str) -> Error {
    return Error::new(ErrorKind::InvalidInput, String::from_str(message));
}

fn __net_parse_port(text: &str) -> Result<i64, Error> {
    if text.is_empty() {
        return Err(__net_address_error("the socket address port is empty"));
    }
    match text.parse_i64() {
        Ok(port) => {
            if port < 0 || port > 65535 {
                return Err(__net_address_error("the socket address port is out of range"));
            }
            return Ok(port);
        }
        Err(_) => {
            return Err(__net_address_error("the socket address port is not a number"));
        }
    }
}

fn __net_is_ipv4_text(text: &str) -> bool {
    let parts: Vec<String> = text.split(".");
    if parts.len() != 4 {
        return false;
    }
    let mut index: usize = 0;
    while index < parts.len() {
        let part: String = parts.get(index).unwrap();
        if part.is_empty() {
            return false;
        }
        match part.parse_i64() {
            Ok(value) => {
                if value < 0 || value > 255 {
                    return false;
                }
            }
            Err(_) => { return false; }
        }
        index += 1;
    }
    return true;
}

fn __net_is_ipv6_text(text: &str) -> bool {
    if text.is_empty() || !text.contains(":") {
        return false;
    }
    let bytes: &[u8] = text.as_bytes();
    let mut index: usize = 0;
    while index < bytes.len() {
        let byte: u8 = bytes.get(index).unwrap();
        let digit: bool = byte >= 48 && byte <= 57;
        let lower: bool = byte >= 97 && byte <= 102;
        let upper: bool = byte >= 65 && byte <= 70;
        let allowed: bool = digit || lower || upper || byte == 58 || byte == 46;
        if !allowed {
            return false;
        }
        index += 1;
    }
    return true;
}

/// A UDP socket bound to a local address.
///
/// `bind` returns an error instead of panicking when the address cannot be
/// bound, matching TcpListener.
pub struct UdpSocket {
    handle: i64,
}

impl UdpSocket {
    /// Binds a UDP socket. Port 0 asks for an ephemeral local port.
    pub fn bind(address: &str) -> Result<UdpSocket, Error> {
        let handle: i64 = __net_udp_bind(address);
        if handle < 0 {
            return Err(Error::last());
        }
        return Ok(UdpSocket { handle: handle });
    }

    /// The local port the socket is bound to.
    pub fn local_port(&self) -> i64 {
        return __net_local_port(self.handle);
    }

    /// The underlying runtime handle.
    pub fn handle(&self) -> i64 {
        return self.handle;
    }

    /// Sends one datagram to the address and returns the bytes sent.
    pub fn send_to(&self, bytes: &[u8], address: &str) -> Result<usize, Error> {
        let count: i64 = __net_udp_send_to(self.handle, bytes, address);
        if count < 0 {
            return Err(Error::last());
        }
        return Ok(count as usize);
    }

    /// Receives one datagram of at most `max` bytes.
    ///
    /// Returns the payload and the peer address. A read timeout configured
    /// with `set_timeout` is reported as Ok(None); other failures are errors.
    pub fn recv_from(&mut self, max: i64) -> Result<Option<(Vec<u8>, String)>, Error> {
        if max < 0 {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                String::from_str("the receive size must not be negative"),
            ));
        }
        let mut buffer: Vec<u8> = Vec::new();
        let mut filled: usize = 0;
        while filled < max as usize {
            buffer.push(0);
            filled += 1;
        }
        let (count, peer) = __net_udp_recv_from(self.handle, buffer.as_mut_slice());
        if count < 0 {
            let error: Error = Error::last();
            match error.kind() {
                ErrorKind::WouldBlock => { return Ok(None); }
                ErrorKind::TimedOut => { return Ok(None); }
                _ => { return Err(error); }
            }
        }
        let mut data: Vec<u8> = Vec::new();
        let mut index: usize = 0;
        while index < count as usize {
            data.push(buffer.get(index).unwrap());
            index += 1;
        }
        return Ok(Some((data, peer)));
    }

    /// Sets a read timeout in milliseconds (0 disables it).
    pub fn set_timeout(&self, milliseconds: i64) -> Result<(), Error> {
        let status: i64 = __net_set_timeout(self.handle, milliseconds);
        if status < 0 {
            return Err(Error::last());
        }
        return Ok(());
    }

    /// Closes the socket. The value may be dropped afterwards.
    pub fn close(&mut self) -> Result<(), Error> {
        if self.handle < 0 {
            return Ok(());
        }
        let status: i64 = __io_close(self.handle);
        if status < 0 {
            return Err(Error::last());
        }
        self.handle = -1;
        return Ok(());
    }
}

impl Drop for UdpSocket {
    fn drop(&mut self) {
        if self.handle >= 0 {
            __io_close(self.handle);
        }
    }
}
