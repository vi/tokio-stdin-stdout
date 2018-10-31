#![deny(missing_docs)]

//! A bridge between [std::io::std{in,out}][1] and Future's AsyncRead/AsyncWrite worlds.
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
//! It works by starting separate threads, which do actual synchronous I/O and communicating to
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

extern crate futures;
extern crate tokio_io;

const BUFSIZ: usize = 8192;

use futures::{Async, AsyncSink, Future, Poll, Sink, Stream};
use std::cell::RefCell;
use std::io::{Error, ErrorKind, Read, Result, Write};
use std::rc::Rc;
use std::sync::{Arc, LockResult, Mutex, MutexGuard, PoisonError, TryLockError, TryLockResult};
use std::thread::JoinHandle;
use tokio_io::{AsyncRead, AsyncWrite};

type BBR = futures::sync::mpsc::Receiver<Box<[u8]>>;
type BBS = futures::sync::mpsc::Sender<Box<[u8]>>;

/// Asynchronous stdin
pub struct ThreadedStdin {
    debt: Option<Box<[u8]>>,
    rcv: BBR,
}

impl ThreadedStdin {
    /// Wrap into `Arc<Mutex>` to make it clonable and sendable
    pub fn make_sendable(self) -> SendableStdin {
        SendableStdin::new(self)
    }
    /// Wrap into `Rc<RefCell>` to make it clonable
    pub fn make_clonable(self) -> ClonableStdin {
        ClonableStdin::new(self)
    }
}

/// Constructor for the `ThreadedStdin`
pub fn stdin(queue_size: usize) -> ThreadedStdin {
    let (snd_, rcv): (BBS, BBR) = futures::sync::mpsc::channel(queue_size);
    std::thread::spawn(move || {
        let mut snd = snd_;
        let sin = ::std::io::stdin();
        let mut sin_lock = sin.lock();
        let mut buf = vec![0; BUFSIZ];
        while let Ok(ret) = sin_lock.read(&mut buf[..]) {
            let content = buf[0..ret].to_vec().into_boxed_slice();
            snd = match snd.send(content).wait() {
                Ok(x) => x,
                Err(_) => break,
            }
        }
    });
    ThreadedStdin { debt: None, rcv }
}

impl AsyncRead for ThreadedStdin {}
impl Read for ThreadedStdin {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        let mut handle_the_buffer = |incoming_buf: Box<[u8]>| {
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

        let (new_debt, ret) = if let Some(debt) = self.debt.take() {
            handle_the_buffer(debt)
        } else {
            match self.rcv.poll() {
                Ok(Async::Ready(Some(newbuf))) => handle_the_buffer(newbuf),
                Ok(Async::Ready(None)) => (None, Err(ErrorKind::BrokenPipe.into())),
                Ok(Async::NotReady) => (None, Err(ErrorKind::WouldBlock.into())),
                Err(_) => (None, Err(ErrorKind::Other.into())),
            }
        };
        self.debt = new_debt;
        ret
    }
}

/// Asynchronous stdout
pub struct ThreadedStdout {
    snd: BBS,
    jh: Option<JoinHandle<()>>,
}

impl ThreadedStdout {
    /// Wrap into `Arc<Mutex>` to make it clonable and sendable
    pub fn make_sendable(self) -> SendableStdout {
        SendableStdout::new(self)
    }
    /// Wrap into `Rc<RefCell>` to make it clonable
    pub fn make_clonable(self) -> ClonableStdout {
        ClonableStdout::new(self)
    }
}
/// Constructor for the `ThreadedStdout`
pub fn stdout(queue_size: usize) -> ThreadedStdout {
    let (snd, rcv): (BBS, BBR) = futures::sync::mpsc::channel(queue_size);
    let jh = std::thread::spawn(move || {
        let sout = ::std::io::stdout();
        let mut sout_lock = sout.lock();
        for b in rcv.wait() {
            if let Ok(b) = b {
                if b.len() == 0 {
                    break;
                }
                if sout_lock.write_all(&b).is_err() {
                    break;
                }
                if sout_lock.flush().is_err() {
                    break;
                }
            } else {
                break;
            }
        }
        let _ = sout_lock.write(&[]);
    });
    ThreadedStdout { snd, jh: Some(jh) }
}

impl AsyncWrite for ThreadedStdout {
    fn shutdown(&mut self) -> Poll<(), Error> {
        // Signal the thread to exit.
        match self.snd.start_send(vec![].into_boxed_slice()) {
            Ok(AsyncSink::Ready) => (),
            Ok(AsyncSink::NotReady(_)) => return Ok(Async::NotReady),
            Err(_) => {
                return Ok(Async::Ready(()));
            }
        };
        match self.snd.poll_complete() {
            Ok(Async::Ready(_)) => (),
            Ok(Async::NotReady) => return Ok(Async::NotReady),
            Err(e) => {
                return Ok(Async::Ready(()));
            }
        };
        if self.snd.close().is_err() {
            return Ok(Async::Ready(()));
        };
        if let Some(jh) = self.jh.take() {
            if jh.join().is_err() {
                return Err(ErrorKind::Other.into());
            };
        }
        Ok(Async::Ready(()))
    }
}
impl Write for ThreadedStdout {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }

        match self.snd.start_send(buf.to_vec().into_boxed_slice()) {
            Ok(AsyncSink::Ready) => (),
            Ok(AsyncSink::NotReady(_)) => return Err(ErrorKind::WouldBlock.into()),
            Err(_) => return Err(ErrorKind::Other.into()),
        }

        Ok(buf.len())
    }
    fn flush(&mut self) -> Result<()> {
        match self.snd.poll_complete() {
            Ok(Async::Ready(_)) => Ok(()),
            Ok(Async::NotReady) => Err(ErrorKind::WouldBlock.into()),
            Err(_) => Err(ErrorKind::Other.into()),
        }
    }
}

// XXX code duplication:

/// Asynchronous stderr
pub type ThreadedStderr = ThreadedStdout;
/// Constructor for the `ThreadedStderr`
pub fn stderr(queue_size: usize) -> ThreadedStderr {
    let (snd, rcv): (BBS, BBR) = futures::sync::mpsc::channel(queue_size);
    let jh = std::thread::spawn(move || {
        let sout = ::std::io::stderr();
        let mut sout_lock = sout.lock();
        for b in rcv.wait() {
            if let Ok(b) = b {
                if b.len() == 0 {
                    break;
                }
                if sout_lock.write_all(&b).is_err() {
                    break;
                }
                if sout_lock.flush().is_err() {
                    break;
                }
            } else {
                break;
            }
        }
        let _ = sout_lock.write(&[]);
    });
    ThreadedStdout { snd, jh: Some(jh) }
}

/// A sendable and clonable ThreadedStdout wrapper based on `Arc<Mutex<ThreadedStdout>>`
///
/// Note that a mutex is being locked every time a write is performed,
/// so performance may be a problem, unless you use the `lock` method.
///
/// Also note that if data is outputted using multiple `Write::write` calls from multiple tasks, the order of chunks is not specified.
#[derive(Clone)]
pub struct SendableStdout(Arc<Mutex<ThreadedStdout>>);

/// Result of `SendableStdout::lock` or `SendableStdout::try_lock`
pub struct SendableStdoutGuard<'a>(MutexGuard<'a, ThreadedStdout>);

impl SendableStdout {
    /// wrap ThreadedStdout or ThreadedStderr in a sendable/clonable wrapper
    pub fn new(so: ThreadedStdout) -> SendableStdout {
        SendableStdout(Arc::new(Mutex::new(so)))
    }

    /// Acquire more permanent mutex guard on stdout, like with `std::io::Stdout::lock`
    /// The returned guard also implements AsyncWrite
    pub fn lock(&self) -> LockResult<SendableStdoutGuard> {
        match self.0.lock() {
            Ok(x) => Ok(SendableStdoutGuard(x)),
            Err(e) => Err(PoisonError::new(SendableStdoutGuard(e.into_inner()))),
        }
    }
    /// Acquire more permanent mutex guard on stdout
    /// The returned guard also implements AsyncWrite
    pub fn try_lock(&self) -> TryLockResult<SendableStdoutGuard> {
        match self.0.try_lock() {
            Ok(x) => Ok(SendableStdoutGuard(x)),
            Err(TryLockError::Poisoned(e)) => Err(TryLockError::Poisoned(PoisonError::new(
                SendableStdoutGuard(e.into_inner()),
            ))),
            Err(TryLockError::WouldBlock) => Err(TryLockError::WouldBlock),
        }
    }
}

impl Write for SendableStdout {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        match self.0.lock() {
            Ok(mut l) => l.write(buf),
            Err(e) => Err(Error::new(ErrorKind::Other, format!("{}", e))),
        }
    }
    fn flush(&mut self) -> Result<()> {
        match self.0.lock() {
            Ok(mut l) => l.flush(),
            Err(e) => Err(Error::new(ErrorKind::Other, format!("{}", e))),
        }
    }
}
impl AsyncWrite for SendableStdout {
    fn shutdown(&mut self) -> Poll<(), Error> {
        match self.0.lock() {
            Ok(mut l) => l.shutdown(),
            Err(e) => Err(Error::new(ErrorKind::Other, format!("{}", e))),
        }
    }
}
impl<'a> Write for SendableStdoutGuard<'a> {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        self.0.write(buf)
    }
    fn flush(&mut self) -> Result<()> {
        self.0.flush()
    }
}
impl<'a> AsyncWrite for SendableStdoutGuard<'a> {
    fn shutdown(&mut self) -> Poll<(), Error> {
        self.0.shutdown()
    }
}

/// A clonable `ThreadedStdout` wrapper based on `Rc<RefCell<ThreadedStdout>>`
/// If you need `Send`, use SendableStdout
#[derive(Clone)]
pub struct ClonableStdout(Rc<RefCell<ThreadedStdout>>);
impl ClonableStdout {
    /// wrap ThreadedStdout or ThreadedStderr in a sendable/clonable wrapper
    pub fn new(so: ThreadedStdout) -> ClonableStdout {
        ClonableStdout(Rc::new(RefCell::new(so)))
    }
}

impl Write for ClonableStdout {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        self.0.borrow_mut().write(buf)
    }
    fn flush(&mut self) -> Result<()> {
        self.0.borrow_mut().flush()
    }
}
impl AsyncWrite for ClonableStdout {
    fn shutdown(&mut self) -> Poll<(), Error> {
        self.0.borrow_mut().shutdown()
    }
}

/// Alias for SendableStdout to avoid confusion of SendableStdout being used for stderr.
///
/// Note that a mutex is being locked every time a read is performed,
/// so performance may be a problem.
///
/// Also note that if data is outputted using multiple `Write::write` calls from multiple tasks, the order of chunks is not specified.
pub type SendableStderr = SendableStdout;
/// Result of `SendableStderr::lock` or `SendableStderr::try_lock`
pub type SendableStderrGuard<'a> = SendableStdoutGuard<'a>;
/// Alias for ClonableStdout to avoid confusion of ClonableStdout being used for stderr.
pub type ClonableStderr = ClonableStdout;

/// A clonable `ThreadedStdout` wrapper based on `Rc<RefCell<ThreadedStdout>>`
/// If you need `Send`, use SendableStdout
///
/// Note that data being read is not duplicated across cloned readers used from multiple tasks.
/// Be careful about corruption.
#[derive(Clone)]
pub struct ClonableStdin(Rc<RefCell<ThreadedStdin>>);
impl ClonableStdin {
    /// wrap ThreadedStdout or ThreadedStderr in a sendable/clonable wrapper
    pub fn new(so: ThreadedStdin) -> ClonableStdin {
        ClonableStdin(Rc::new(RefCell::new(so)))
    }
}

impl Read for ClonableStdin {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        self.0.borrow_mut().read(buf)
    }
}
impl AsyncRead for ClonableStdin {}
/// A sendable and clonable ThreadedStdin wrapper based on `Arc<Mutex<ThreadedStdin>>`
///
/// Note that data being read is not duplicated across cloned readers used from multiple tasks.
/// Be careful about corruption.
#[derive(Clone)]
pub struct SendableStdin(Arc<Mutex<ThreadedStdin>>);

/// Result of `SendableStdin::lock` or `SendableStdin::try_lock`
pub struct SendableStdinGuard<'a>(MutexGuard<'a, ThreadedStdin>);

impl SendableStdin {
    /// wrap ThreadedStdin in a sendable/clonable wrapper
    pub fn new(si: ThreadedStdin) -> SendableStdin {
        SendableStdin(Arc::new(Mutex::new(si)))
    }

    /// Acquire more permanent mutex guard on stdout, like with `std::io::Stdout::lock`
    /// The returned guard also implements AsyncWrite
    pub fn lock(&self) -> LockResult<SendableStdinGuard> {
        match self.0.lock() {
            Ok(x) => Ok(SendableStdinGuard(x)),
            Err(e) => Err(PoisonError::new(SendableStdinGuard(e.into_inner()))),
        }
    }
    /// Acquire more permanent mutex guard on stdout
    /// The returned guard also implements AsyncWrite
    pub fn try_lock(&self) -> TryLockResult<SendableStdinGuard> {
        match self.0.try_lock() {
            Ok(x) => Ok(SendableStdinGuard(x)),
            Err(TryLockError::Poisoned(e)) => Err(TryLockError::Poisoned(PoisonError::new(
                SendableStdinGuard(e.into_inner()),
            ))),
            Err(TryLockError::WouldBlock) => Err(TryLockError::WouldBlock),
        }
    }
}

impl Read for SendableStdin {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        match self.0.lock() {
            Ok(mut l) => l.read(buf),
            Err(e) => Err(Error::new(ErrorKind::Other, format!("{}", e))),
        }
    }
}
impl AsyncRead for SendableStdin {}

impl<'a> Read for SendableStdinGuard<'a> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        self.0.read(buf)
    }
}
impl<'a> AsyncRead for SendableStdinGuard<'a> {}
