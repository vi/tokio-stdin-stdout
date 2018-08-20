#!/usr/bin/env cargo-script
//!
//! ```cargo
//! [dependencies]
//!  tokio = "0.1.7"
//!  tokio-io = "0.1"
//!  tokio-stdin-stdout = "0.1"
//!  futures-util-preview = {version="0.3.0-alpha.3", features=["tokio-compat"]}
//!  futures-preview = "0.3.0-alpha.3"
//!  
//! ```

#![feature(await_macro,async_await,futures_api)]
 
extern crate tokio;
extern crate tokio_io;
extern crate tokio_stdin_stdout;

extern crate futures_util;

use futures_util::FutureExt;
use futures_util::TryFutureExt;
use futures_util::compat::Future01CompatExt;
use futures_util::compat::TokioDefaultSpawn;

use tokio_io::io::write_all as tokio_write_all;

async fn prog() {
    
    let stdout = tokio_stdin_stdout::stdout(0).make_sendable();
    
    for _ in 0..10usize {
        await!(tokio_write_all(stdout.clone(), "hello\n").compat());
    }
    await!(tokio_io::io::shutdown(stdout).compat());
    
}

fn main() {
    tokio::run(prog().boxed().unit_error().compat(TokioDefaultSpawn));
}
