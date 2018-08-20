#!/usr/bin/env cargo-script
//!
//! ```cargo
//! [dependencies]
//!  tokio = "0.1.7"
//!  tokio-io = "0.1"
//!  tokio-stdin-stdout = "0.1"
//!  tokio-codec = "0.1.0"
//!  futures-await = "0.1.1"
//!  
//! ```
#![feature(proc_macro, proc_macro_non_items, generators)]
#![allow(stable_features)]
 
extern crate tokio;
extern crate tokio_io;
extern crate tokio_stdin_stdout;
extern crate tokio_codec;
extern crate futures_await as futures;
use futures::prelude::{*,await};


use tokio_codec::{FramedRead, FramedWrite, LinesCodec};

#[async]
fn async_op(input: String) -> Result<String,std::io::Error> {
  Ok(input.to_ascii_uppercase())
}

#[async]
fn prog() -> Result<(),std::io::Error> {
    let stdin = tokio_stdin_stdout::stdin(0).make_sendable();
    let stdout = tokio_stdin_stdout::stdout(0).make_sendable();
    
    let framed_stdin = FramedRead::new(stdin, LinesCodec::new());
    let mut framed_stdout = FramedWrite::new(stdout, LinesCodec::new());
    
    #[async] for line in framed_stdin {
        let line2 = await!(async_op(line))?;
        framed_stdout = await!(framed_stdout.send(line2))?;
    }
   
    await!(tokio_io::io::shutdown(framed_stdout.into_inner()));
    Ok(())
}

fn main() {
    tokio::run(prog().map_err(|e|panic!("Error:{}",e)));
}
