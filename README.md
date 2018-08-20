# tokio-stdin-stdout
AsyncRead/AsyncWrite stdin/stdout for Tokio

[Documentation](https://docs.rs/tokio-stdin-stdout) - more description there

Example:

```rust
let mut core = tokio_core::reactor::Core::new()?;

let stdin = tokio_stdin_stdout::stdin(0);
let stdout = tokio_stdin_stdout::stdout(0);

core.run(tokio_io::io::copy(stdin, stdout))?;

```

There are additional examples:

1. [`loop.rs`](examples/loop.rs) - Write hello ten times
2. [`line-by-line.rs`](examples/line-by-line.rs) - Convert all input text to ASCII upper case, line by line. This example also demonstrates usage of [tokio-codec](https://docs.rs/tokio-codec) and various modes of starting Tokio programs (multithreaded, singlethreaded).

### Alternatives

1. [tokio-stdin](https://crates.io/crates/tokio-stdin) no AsyncRead, only stdin, byte by byte
2. [tokio-file-unix](https://crates.io/crates/tokio-file-unix) - better, but only Unix
