extern crate tokio_stdin_stdout;
extern crate tokio;
extern crate tokio_io;
extern crate tokio_codec;

use tokio::prelude::future::ok;
use tokio::prelude::{Future, Stream};
use tokio_codec::{FramedRead, FramedWrite, LinesCodec};

fn async_op(input: String) -> Box<Future<Item = String, Error = ()> + Send> {
  Box::new(ok(input.to_ascii_uppercase()))
}

fn main() {
  let stdin = tokio_stdin_stdout::stdin(0);
  let stdout = tokio_stdin_stdout::stdout(0).make_sendable();

  let framed_stdin = FramedRead::new(stdin, LinesCodec::new());
  let framed_stdout = FramedWrite::new(stdout, LinesCodec::new());
  
  let future = framed_stdin
    .and_then(move |line| {
      // `and_then` above is not a Future's "and_then", it Stream's "and_then".
      async_op(line)
      .map_err(|err|panic!("Error: {:?}", err))
    })
    .forward(framed_stdout)
    .map(|(_framed_stdin,_framed_stdout)|{
      // _framed_stdin is exhaused now
      // _framed_stdout is closed by `forward` above and is also unusable
      
      // You may try with `send_all` approach if you need other behaviour,
      // but remember to `shutdown` the `stdout` to avoid existing before
      // the data is actually delivered to stdout.
      
      // this `map` is needed to bring the final type to (), 
      // as typically required for executing a future.
    })
    .map_err(|err| {
      panic!("Error: {:?}", err);
    });

  tokio::run(future);
}
