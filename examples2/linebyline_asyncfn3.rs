#!/usr/bin/env cargo-script
//!
//! ```cargo
//! [dependencies]
//!  tokio = "0.1.7"
//!  tokio-io = "0.1"
//!  tokio-stdin-stdout = "0.1"
//!  tokio-codec = "0.1.0"
//!  futures-util-preview = {version="0.3.0-alpha.3", features=["tokio-compat"]}
//!  futures-preview = "0.3.0-alpha.3"
//!  
//! ```

#![feature(await_macro,async_await,futures_api, stmt_expr_attributes, custom_attribute)]
 
extern crate tokio;
extern crate tokio_io;
extern crate tokio_stdin_stdout;
extern crate tokio_codec;

extern crate futures_util;

use futures_util::FutureExt;
use futures_util::TryFutureExt;
use futures_util::compat::Future01CompatExt;
use futures_util::compat::TokioDefaultSpawn;
use futures_util::compat::Stream01CompatExt;
use futures_util::StreamExt;


use tokio_codec::{FramedRead, FramedWrite, LinesCodec};

use tokio_io::io::write_all as tokio_write_all;

async fn prog() {
    let stdin = tokio_stdin_stdout::stdin(0).make_sendable();
    let stdout = tokio_stdin_stdout::stdout(0).make_sendable();
    
    let framed_stdin = FramedRead::new(stdin, LinesCodec::new());
    let framed_stdout = FramedWrite::new(stdout, LinesCodec::new());
    
    async for line in framed_stdin.compat() {
        // N/A
    }
    
    await!(tokio_io::io::shutdown(framed_stdout.into_inner()).compat());
    
}

fn main() {
    tokio::run(prog().boxed().unit_error().compat(TokioDefaultSpawn));
}
