extern crate tokio_stdin_stdout;
extern crate tokio;
extern crate tokio_io;
extern crate tokio_codec;

use tokio::prelude::future::ok;
use tokio::prelude::{Future, Stream};
use tokio_io::io::write_all as tokio_write_all;
use tokio_codec::{FramedRead, LinesCodec};

fn async_op(input: String) -> Box<Future<Item = String, Error = ()> + Send> {
  Box::new(ok(input.to_ascii_uppercase()))
}

fn main() {
  let stdin = tokio_stdin_stdout::stdin(0);
  let stdout = tokio_stdin_stdout::stdout(0).make_sendable();

  let framed_stdin = FramedRead::new(stdin, LinesCodec::new());
  let future = framed_stdin.for_each(move |line| {
    let stdout_ = stdout.clone();
    let future_compute = async_op(line).and_then(|mut result| {
      result.push_str("\n");
      let future_write = tokio_write_all(stdout_, result).map(drop).map_err(drop);
      return future_write;
    });

    future_compute.map_err(|err|panic!("Error: {:?}",err))
  }).map_err(|err| {
    panic!("Error: {:?}", err);
  });

  tokio::run(future);
}
