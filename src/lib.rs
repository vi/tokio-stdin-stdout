extern crate tokio_io;
extern crate futures;

const BUFSIZ: usize = 8192;

use std::io::{Error, ErrorKind, Result, Read, Write};
use futures::{Stream,Poll,Async,Sink,Future,AsyncSink};
use tokio_io::{AsyncRead,AsyncWrite};
use futures::sync::mpsc::{Receiver, Sender};

type BBR = futures::sync::mpsc::Receiver <Box<[u8]>>;
type BBS = futures::sync::mpsc::Sender   <Box<[u8]>>;

use std::sync::Arc;
use std::sync::{Mutex, Condvar};

struct Traveller<BB> (
    Arc<(Mutex<Option<BB>>,Condvar)>,
);

impl<BB> Traveller<BB> {
    fn new(x:BB) -> Self {
        Traveller(
            Arc::new((Mutex::new(Some(x)),Condvar::new())),
        )
    }
    fn off_we_go(&self) -> Option<BB> {
        if let Ok(mut x) = (self.0).0.lock() {
            x.take()
        } else {
            None
        }
    }
    fn return_home(&self, b:BB, with_fanfares : bool) -> ::std::result::Result<(), BB> {
        if let Ok(mut x) = (self.0).0.lock() {
            if x.is_none() {
                *x = Some(b);
                if with_fanfares {
                    (self.0).1.notify_one();
                }
                Ok(())
            } else {
                Err(b)
            }
        } else {
            Err(b)
        }
    }
    fn wait_for_homecoming_party<F:Fn(&BB)->bool> (&self, requirement:F) -> ::std::result::Result<BB,()> {
        loop {
            if let Ok(mut x) = (self.0).0.lock() {
                if let Some(z) = x.take() {
                    if requirement(&z) {
                        return Ok(z)
                    } else {
                        *x = Some(z);
                    }   
                }
                if let Ok(mut y) = (self.0).1.wait(x) {
                    if let Some(z) = y.take() {
                        if requirement(&z) {
                            return Ok(z)
                        } else {
                            *y = Some(z);
                        }
                    }
                } else {
                    return Err(())
                }
            } else {
                return Err(())
            }
        }
    }
}
impl<BB> Clone for Traveller<BB> {
    fn clone(&self) -> Self {
        Traveller(self.0.clone())
    }
}

pub struct ThreadedStdin {
    debt : Option<Box<[u8]>>,
    rcv : BBR,
}

pub fn stdin() -> ThreadedStdin {
    let (snd_, rcv) : (BBS,BBR) =  futures::sync::mpsc::channel(0);
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

struct WriteOp {
    buf : [u8; BUFSIZ],
    len : isize,
    task : Option<futures::task::Task>,
}
type WriteOpBox = Box<WriteOp>;

pub struct ThreadedStdout (
    Traveller<WriteOpBox>,
);
pub fn stdout() -> ThreadedStdout {
    //let (snd, rcv) = futures::sync::mpsc::channel(0);
    let t1 = Traveller::new(Box::new(WriteOp { 
        buf: [0; BUFSIZ], 
        len:0, 
        task: None 
    }));
    let t2 = t1.clone();
    std::thread::spawn(move || {
        let sout = ::std::io::stdout();
        let mut sout_lock = sout.lock();
        while let Ok(mut x) = t1.wait_for_homecoming_party(|x|x.len > 0) {
            if let Err(_) = sout_lock.write_all(&x.buf[0..(x.len as usize)]) {
                x.len = -1;
            } else {
                x.len = 0;
            }
            
            let task = x.task.clone();
            t1.return_home(x, false);
            
            if let Some(t) = task {
                t.notify();
            } else {
                eprintln!("?");
            }
        }
        /*for b in rcv.wait() {
            if let Err(_) = b {
                break;
            }
            if let Err(_) = sout_lock.write_all(&b.unwrap()) {
                break;
            }
        }*/
    });
    ThreadedStdout(t2)
}
impl AsyncWrite for ThreadedStdout {
    fn shutdown(&mut self) -> Poll<(), Error> {
        // XXX
        Ok(Async::Ready(()))
    }
}
impl Write for ThreadedStdout {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        let would_block1 : Error = ErrorKind::WouldBlock.into();
        let would_block2 : Error = ErrorKind::WouldBlock.into();
        
        let mut x = self.0.off_we_go().ok_or(would_block1)?;
        let l = x.len;
        
        
        if l!=0 {
            self.0.return_home(x, true);
            if l < 0 { return Err(ErrorKind::Other.into()); }
            return Err(would_block2);
        }
        
        let l = std::cmp::min(x.buf.len(), buf.len());
        x.buf[0..l].copy_from_slice(&buf[0..l]);
        x.len = l as isize;
        x.task = Some(futures::task::current());
        
        if let Err(_) = self.0.return_home(x, true) {
            // Can't properly unwrap
            panic!("Assertion failed");
        }
        
        //self.buf.home_sweet_home
        /*match self.snd.start_send(buf.to_vec().into_boxed_slice()) {
            Ok(AsyncSink::Ready)       => (),
            Ok(AsyncSink::NotReady(_)) => return Err(ErrorKind::WouldBlock.into()),
            Err(_)                     => return Err(ErrorKind::Other.into()),
        }
        match self.snd.poll_complete() {
            // XXX
            Ok(Async::Ready(_))    => (), // OK
            Ok(Async::NotReady)    => (), // don't know what to do here
            Err(_) => return Err(ErrorKind::Other.into()),
        }*/
        Ok(l)
    }
    fn flush(&mut self) -> Result<()> {
        // XXX
        Ok(())
    }
}

