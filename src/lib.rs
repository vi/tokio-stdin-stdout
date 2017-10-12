#![deny(missing_docs)]

//! A bridge between [std::io::std{in,out}][1] and Future's AsyncRead/AsyncWrite world.
//! [1]:https://doc.rust-lang.org/std/io/fn.stdin.html
//!
//! Example:
//!
//! ```rust,no_run
//! extern crate tokio_core;
//! extern crate tokio_io;
//! extern crate tokio_stdin_stdout;
//!
//! let mut core = tokio_core::reactor::Core::new().unwrap();
//!
//! let stdin = tokio_stdin_stdout::stdin(0);
//! let stdout = tokio_stdin_stdout::stdout(0);
//!
//! core.run(tokio_io::io::copy(stdin, stdout)).unwrap();
//! ```
//!
//! It works by starting separate threads, which do actual synchronous I/O and communicates to
//! the asynchronous world using [future::sync::mpsc](http://alexcrichton.com/futures-rs/futures/sync/mpsc/index.html).
//!
//! For Unix (Linux, OS X) better use [tokio-file-unix](https://crates.io/crates/tokio-file-unix).
//!
//! Concerns:
//!
//! * stdin/stdout are not expected to be ever normally used after using functions from this crate
//! * Allocation-heavy.
//! * All errors collapsed to ErrorKind::Other (for stdout) or ErrorKind::BrokenPipe (for stdin)
//! * Failure to write to stdout is only seen after attempting to send there about 3 more buffers.

extern crate tokio_io;
extern crate futures;

const BUFSIZ: usize = 8192;

use std::io::{Error, ErrorKind, Result, Read, Write};
use futures::{Stream,Poll,Async,Sink,Future,AsyncSink};
use tokio_io::{AsyncRead,AsyncWrite};

type BBR = futures::sync::mpsc::Receiver <Box<[u8]>>;
type BBS = futures::sync::mpsc::Sender   <Box<[u8]>>;

/// Asynchronous stdin
pub struct ThreadedStdin {
    debt : Option<Box<[u8]>>,
    rcv : BBR,

}
/// Constructor for the `ThreadedStdin`
pub fn stdin(queue_size:usize) -> ThreadedStdin {
    let (snd_, rcv) : (BBS,BBR) =  futures::sync::mpsc::channel(queue_size);
    std::thread::spawn(move || {
        let mut snd = snd_;
        let sin = ::std::io::stdin();
        let mut sin_lock = sin.lock();
        let mut buf = vec![0; BUFSIZ];
        loop {
            let ret = match sin_lock.read(&mut buf[..]) {
                Ok(x) => x,
                Err(_) => {
                    // BrokenPipe
                    break;
                }
            };
            let content = buf[0..ret].to_vec().into_boxed_slice();
            snd = match snd.send(content).wait() {
                Ok(x) => x,
                Err(_) => break,
            }
        }
    });
    ThreadedStdin {
        debt: None,
        rcv,
    }
}

impl AsyncRead for ThreadedStdin {}
impl Read for ThreadedStdin {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
    
        let mut handle_the_buffer = |incoming_buf:Box<[u8]>| {
            let l = buf.len();
            let dl = incoming_buf.len();
            if l >= dl {
                buf[0..dl].copy_from_slice(&incoming_buf);
                (None, Ok(dl))
            } else {
                buf[0..l].copy_from_slice(&incoming_buf[0..l]);
                let newdebt = Some(incoming_buf[l..].to_vec().into_boxed_slice());
                (newdebt, Ok(l))
            }
        };
        
        let (new_debt, ret) =
            if let Some(debt) = self.debt.take() {
                handle_the_buffer(debt)
            } else {
                match self.rcv.poll() {
                    Ok(Async::Ready(Some(newbuf))) => handle_the_buffer(newbuf),
                    Ok(Async::Ready(None)) => (None, Err(ErrorKind::BrokenPipe.into())),
                    Ok(Async::NotReady)    => (None, Err(ErrorKind::WouldBlock.into())),
                    Err(_)                 => (None, Err(ErrorKind::Other.into())),
                }
            };
        self.debt = new_debt;
        return ret
    }
}


/// Asynchronous stdout
pub struct ThreadedStdout {
    snd : BBS,
}
/// Constructor for the `ThreadedStdout`
pub fn stdout(queue_size:usize) -> ThreadedStdout {
    let (snd, rcv) : (BBS,BBR) =  futures::sync::mpsc::channel(queue_size);
    std::thread::spawn(move || {
        let sout = ::std::io::stdout();
        let mut sout_lock = sout.lock();
        for b in rcv.wait() {
            if let Err(_) = b {
                break;
            }
            if let Err(_) = sout_lock.write_all(&b.unwrap()) {
                break;
            }
        }
        let _ = sout_lock.write(&[]);
    });
    ThreadedStdout {
        snd,
    }
}
impl AsyncWrite for ThreadedStdout {
    fn shutdown(&mut self) -> Poll<(), Error> {
        match self.snd.close() {
            Ok(x) => Ok(x),
            Err(_) => Err(ErrorKind::Other.into()),
        }
    }
}
impl Write for ThreadedStdout {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        match self.snd.start_send(buf.to_vec().into_boxed_slice()) {
            Ok(AsyncSink::Ready)       => (),
            Ok(AsyncSink::NotReady(_)) => return Err(ErrorKind::WouldBlock.into()),
            Err(_)                     => return Err(ErrorKind::Other.into()),
        }
        
        Ok(buf.len())
    }
    fn flush(&mut self) -> Result<()> {
        match self.snd.poll_complete() {
            Ok(Async::Ready(_))    => Ok(()),
            Ok(Async::NotReady)    => Err(ErrorKind::WouldBlock.into()),
            Err(_) => return Err(ErrorKind::Other.into()),
        }
    }
}

