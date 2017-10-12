extern crate tokio_core;
extern crate tokio_io;

extern crate tokio_stdin_stdout;

use std::io::Result;

fn run() -> Result<()> {
    let mut core = tokio_core::reactor::Core::new()?;
    //let handle = core.handle();
    
    let stdin = tokio_stdin_stdout::stdin();
    let stdout = tokio_stdin_stdout::stdout();
    
    core.run(tokio_io::io::copy(stdin, stdout))?;
    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("Something failed: {}", e);
        ::std::process::exit(1);
    }
}
