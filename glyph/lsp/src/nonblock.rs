//! This code is adapted from https://github.com/anowell/nonblock-rs
//! Read available data from file descriptors without blocking
//!
//! Useful for nonblocking reads from sockets, named pipes, and child stdout/stderr
//!
//! # Example
//!
//! ```no_run
//! use std::io::Read;
//! use std::process::{Command, Stdio};
//! use std::time::Duration;
//! use lsp::nonblock::NonBlockingReader;
//!
//! let mut child = Command::new("some-executable")
//!                         .stdout(Stdio::piped())
//!                         .spawn().unwrap();
//! let stdout = child.stdout.take().unwrap();
//! let mut noblock_stdout = NonBlockingReader::from_fd(stdout).unwrap();
//! while !noblock_stdout.is_eof() {
//!     let mut buf = String::new();
//!     noblock_stdout.read_available_to_string(&mut buf).unwrap();
//!     std::thread::sleep(Duration::from_secs(5));
//! }
//! ```
extern crate libc;
use bytes::{BufMut, BytesMut};
use libc::{fcntl, F_GETFL, F_SETFL, O_NONBLOCK};
use std::io::{self, ErrorKind, Read};
use std::os::unix::io::{AsRawFd, RawFd};

/// Simple non-blocking wrapper for reader types that implement AsRawFd
pub struct NonBlockingReader<R: AsRawFd + Read> {
    eof: bool,
    reader: R,
}

impl<R: AsRawFd + Read> NonBlockingReader<R> {
    /// Initialize a NonBlockingReader from the reader's file descriptor.
    ///
    /// The reader will be managed internally,
    ///   and O_NONBLOCK will be set the file descriptor.
    pub fn from_fd(reader: R) -> io::Result<NonBlockingReader<R>> {
        let fd = reader.as_raw_fd();
        set_blocking(fd, false)?;
        Ok(NonBlockingReader { reader, eof: false })
    }

    /// Consume this NonBlockingReader and return the blocking version
    ///   of the internally managed reader.
    ///
    /// This will disable O_NONBLOCK on the file descriptor,
    ///   and any data read from the NonBlockingReader before calling `into_blocking`
    ///   will already have been consumed from the reader.
    pub fn into_blocking(self) -> io::Result<R> {
        let fd = self.reader.as_raw_fd();
        set_blocking(fd, true)?;
        Ok(self.reader)
    }

    /// Indicates if EOF has been reached for the reader.
    ///
    /// Currently this defaults to false until one of the `read_available` methods is called,
    ///   but this may change in the future if I stumble on a compelling way
    ///   to check for EOF without consuming any of the internal reader.
    pub fn is_eof(&self) -> bool {
        self.eof
    }

    /// Reads any available data from the reader without blocking, placing them into `buf`.
    ///
    /// If successful, this function will return the total number of bytes read.
    ///  0 bytes read may indicate the EOF has been reached or that reading
    ///  would block because there is not any data immediately available.
    ///  Call `is_eof()` after this method to determine if EOF was reached.
    ///
    /// ## Errors
    ///
    /// If this function encounters an error of the kind `ErrorKind::Interrupted`
    ///   then the error is ignored and the operation will continue.
    ///   If it encounters `ErrorKind::WouldBlock`, then this function immediately returns
    ///   the total number of bytes read so far.
    ///
    /// If any other read error is encountered then this function immediately returns.
    ///   Any bytes which have already been read will be appended to buf.
    ///
    /// ## Examples
    /// ```no_run
    /// # use std::io::Read;
    /// # use std::net::TcpStream;
    /// # use std::time::Duration;
    /// # use lsp::nonblock::NonBlockingReader;
    /// #
    /// let client = TcpStream::connect("127.0.0.1:34567").unwrap();
    /// let mut noblock_stdout = NonBlockingReader::from_fd(client).unwrap();
    /// // let mut buf = Vec::new();
    /// // noblock_stdout.read_available(&mut buf).unwrap();
    /// ```

    pub fn read_available(&mut self, buf: &mut BytesMut) -> io::Result<usize> {
        let mut buf_len = 0;
        loop {
            let mut bytes = [0u8; 1024];
            match self.reader.read(&mut bytes[..]) {
                // EOF
                Ok(0) => {
                    self.eof = true;
                    break;
                }
                // Not EOF, but no more data currently available
                Err(ref err) if err.kind() == ErrorKind::WouldBlock => {
                    self.eof = false;
                    break;
                }
                // Ignore interruptions, continue reading
                Err(ref err) if err.kind() == ErrorKind::Interrupted => {}
                // bytes available
                Ok(len) => {
                    buf_len += len;
                    buf.put(&bytes[0..(len)])
                }
                // IO Error encountered
                Err(err) => {
                    return Err(err);
                }
            }
        }
        Ok(buf_len)
    }

    /// Reads any available data from the reader without blocking, placing them into `buf`.
    ///
    /// If successful, this function returns the number of bytes which were read and appended to buf.
    ///
    /// ## Errors
    ///
    /// This function inherits all the possible errors of `read_available()`.
    ///   In the case of errors that occur after successfully reading some data,
    ///   the successfully read data will still be parsed and appended to `buf`.
    ///
    /// Additionally, if the read data cannot be parsed as UTF-8,
    ///   then `buf` will remain unmodified, and this method will return `ErrorKind::InvalidData`
    ///   with the `FromUtf8Error` containing any data that was read.
    ///
    /// ## Examples
    /// ```no_run
    /// # use std::io::Read;
    /// # use std::process::{Command, Stdio};
    /// # use std::time::Duration;
    /// # use lsp::nonblock::NonBlockingReader;
    /// #
    /// let mut child = Command::new("foo").stdout(Stdio::piped()).spawn().unwrap();
    /// let stdout = child.stdout.take().unwrap();
    /// let mut noblock_stdout = NonBlockingReader::from_fd(stdout).unwrap();
    /// let mut buf = String::new();
    /// noblock_stdout.read_available_to_string(&mut buf).unwrap();
    /// ```
    ///
    /// In theory, since this function only reads immediately available data,
    ///   There may not be any guarantee that the data immediately available ends
    ///   on a UTF-8 alignment, so it might be worth a bufferred wrapper
    ///   that manages the captures a final non-UTF-8 character and prepends it to the next call,
    ///   but in practice, this has worked as expected.
    pub fn read_available_to_string(&mut self, buf: &mut String) -> io::Result<usize> {
        let mut byte_buf = BytesMut::new();
        let res = self.read_available(&mut byte_buf);
        match String::from_utf8(byte_buf.to_vec()) {
            Ok(utf8_buf) => {
                // append any read data before returning the `read_available` result
                buf.push_str(&utf8_buf);
                res
            }
            Err(err) => {
                // check for read error before returning the UTF8 Error
                let _ = res?;
                Err(io::Error::new(ErrorKind::InvalidData, err))
            }
        }
    }
}

fn set_blocking(fd: RawFd, blocking: bool) -> io::Result<()> {
    let flags = unsafe { fcntl(fd, F_GETFL, 0) };
    if flags < 0 {
        return Err(io::Error::last_os_error());
    }

    let flags = if blocking {
        flags & !O_NONBLOCK
    } else {
        flags | O_NONBLOCK
    };
    let res = unsafe { fcntl(fd, F_SETFL, flags) };
    if res != 0 {
        return Err(io::Error::last_os_error());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use bytes::BytesMut;

    use super::NonBlockingReader;
    use std::io::Write;
    use std::net::{TcpListener, TcpStream};
    use std::sync::mpsc::channel;
    use std::thread;

    #[test]
    fn it_works() {
        let server = TcpListener::bind("127.0.0.1:34567").unwrap();
        let (tx, rx) = channel();

        thread::spawn(move || {
            let (stream, _) = server.accept().unwrap();
            tx.send(stream).unwrap();
        });

        let client = TcpStream::connect("127.0.0.1:34567").unwrap();
        let mut stream = rx.recv().unwrap();

        let mut nonblocking = NonBlockingReader::from_fd(client).unwrap();
        let mut buf = BytesMut::new();

        assert_eq!(nonblocking.read_available(&mut buf).unwrap(), 0);
        assert_eq!(buf.to_vec().as_slice(), b"");

        assert_eq!(stream.write(b"foo").unwrap(), 3);

        let mut read = nonblocking.read_available(&mut buf).unwrap();
        while read == 0 && !nonblocking.is_eof() {
            read = nonblocking.read_available(&mut buf).unwrap();
        }

        assert_eq!(read, 3);
        assert_eq!(buf.to_vec().as_slice(), b"foo");
    }
}
