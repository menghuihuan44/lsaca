// TCP networking.
//
// Plain TCP only. Addresses are owned String values in the form
// "host:port" (for example "127.0.0.1:8080"); a port of 0 asks the
// operating system for an ephemeral port, which local_port reports.
// Failures are reported as io::Error values, never as panics. Both
// listener and stream handles are closed when the value is dropped.

pub extern fn __net_listen(address: String) -> i64;
pub extern fn __net_local_port(handle: i64) -> i64;
pub extern fn __net_accept(listener: i64) -> i64;
pub extern fn __net_connect(address: String) -> i64;
pub extern fn __net_set_timeout(handle: i64, milliseconds: i64) -> i64;
pub extern fn __net_shutdown(handle: i64, how: i64) -> i64;
pub extern fn __io_close(handle: i64) -> i64;
pub extern fn __io_read(handle: i64, buffer: &mut [u8]) -> i64;
pub extern fn __io_write(handle: i64, buffer: &[u8]) -> i64;

/// A listening TCP socket.
pub struct TcpListener {
    handle: i64,
}

impl TcpListener {
    /// Binds a listener to an address such as "127.0.0.1:0".
    pub fn bind(address: String) -> Result<TcpListener, Error> {
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
    pub fn connect(address: String) -> Result<TcpStream, Error> {
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
    pub fn read(&mut self, buffer: &mut [u8]) -> Result<i64, Error> {
        let count: i64 = __io_read(self.handle, buffer);
        if count < 0 {
            return Err(Error::last());
        }
        return Ok(count);
    }

    /// Writes the whole buffer, returning the number of bytes written.
    pub fn write(&mut self, buffer: &[u8]) -> Result<i64, Error> {
        let count: i64 = __io_write(self.handle, buffer);
        if count < 0 {
            return Err(Error::last());
        }
        return Ok(count);
    }

    /// Sets a read and write timeout in milliseconds (0 disables it).
    pub fn set_timeout(&self, milliseconds: i64) -> Result<(), Error> {
        let status: i64 = __net_set_timeout(self.handle, milliseconds);
        if status < 0 {
            return Err(Error::last());
        }
        return Ok(());
    }

    /// Shuts down the connection: how 0 read, 1 write, 2 both directions.
    pub fn shutdown(&self, how: i64) -> Result<(), Error> {
        let status: i64 = __net_shutdown(self.handle, how);
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
