# tokio-stdin-stdout
AsyncRead/AsyncWrite stdin/stdout for Tokio

[Documentation](https://docs.rs/tokio-stdin-stdout) - more description there

# Example

```rust
let mut core = tokio_core::reactor::Core::new()?;

let stdin = tokio_stdin_stdout::stdin(0);
let stdout = tokio_stdin_stdout::stdout(0);

core.run(tokio_io::io::copy(stdin, stdout))?;

```

## Additional examples

1. [`loop.rs`](examples/loop.rs) - Write hello ten times
2. [`line-by-line.rs`](examples/line-by-line.rs) - Convert all input text to ASCII upper case, line by line. This example also demonstrates usage of [tokio-codec](https://docs.rs/tokio-codec) and various modes of starting Tokio programs (multithreaded, singlethreaded).


## async fn demo

Not much related to tokio-stdin-stdout, but there are some `async fn` examples runnable by [`cargo script`](https://crates.io/crates/cargo-script).

They require nightly Rust.

* [loop example as async fn v1](examples2/loop_asyncfn1.rs) - The same as loop.rs above, but may be more readable.due to async fn backed by futures_await crate.
* [line-by-line example as async fn v1](examples2/linebyline_asyncfn1.rs) - The same as line-by-line.rs, but more prodecural-looking.
* [loop example as async fn v3](examples2/loop_asyncfn3.rs) - Another try with loop.rs, but this time using [new async engine](https://github.com/rust-lang/rfcs/blob/master/text/2394-async_await.md) built-in in Rust itself. As it intrefaces early alpha code, it may stop working after a while.

# Alternatives

1. [tokio-stdin](https://crates.io/crates/tokio-stdin) no AsyncRead, only stdin, byte by byte
2. [tokio-file-unix](https://crates.io/crates/tokio-file-unix) - better, but only Unix
