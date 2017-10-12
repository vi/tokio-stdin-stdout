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

### Alternatives

1. [tokio-stdin](https://crates.io/crates/tokio-stdin) no AsyncRead, only stdin, byte by byte
2. [tokio-file-unix](https://crates.io/crates/tokio-file-unix) - better, but only Unix
