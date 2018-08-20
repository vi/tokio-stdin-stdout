#!/usr/bin/env cargo-script
//!
//! ```cargo
//! [dependencies]
//!  tokio = "0.1.7"
//!  tokio-io = "0.1"
//!  tokio-stdin-stdout = "0.1"
//!  futures-await = "0.1.1"
//!  
//! ```

#![feature(proc_macro, proc_macro_non_items, generators)]
#![allow(stable_features)]
 
extern crate tokio;
extern crate tokio_io;
extern crate tokio_stdin_stdout;
extern crate futures_await as futures;
use futures::prelude::{*,await};

use tokio_io::io::write_all as tokio_write_all;

#[async]
fn prog() -> Result<(),()> {
    let stdout = tokio_stdin_stdout::stdout(0).make_sendable();
    
    for _ in 0..10 {
        await!(tokio_write_all(stdout.clone(), "hello\n"));
    }
    await!(tokio_io::io::shutdown(stdout));
    
    Ok(())
}

fn main() {
    tokio::run(prog());
}
