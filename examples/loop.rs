extern crate tokio_io;
extern crate tokio_stdin_stdout;
extern crate tokio;
use tokio::prelude::Future;

use tokio_io::io::write_all as tokio_write_all;

use tokio::prelude::Stream;

fn main() {
  
  let stdout = tokio_stdin_stdout::stdout(0).make_sendable();
  let stdout_ = stdout.clone();

  let h = std::iter::repeat("hello\n").take(10);
  let s = tokio::prelude::stream::iter_ok::<_,()>(h);
  let f = s.for_each(move |x| {
    tokio_write_all(stdout_.clone(), x).map(drop).map_err(drop)
  });
  let prog = f.and_then(move |()| {
    tokio_io::io::shutdown(stdout.clone()).map(drop).map_err(drop)
  });

  tokio::run(prog);
}
