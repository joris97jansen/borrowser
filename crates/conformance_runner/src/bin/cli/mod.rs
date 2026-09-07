#![cfg_attr(
    not(feature = "aggregate"),
    allow(
        dead_code,
        reason = "requests are validated before checking feature availability"
    )
)]
#[cfg(feature = "aggregate")]
mod aggregate;
mod arguments;
mod legacy;
#[cfg(feature = "aggregate")]
mod publication;
#[cfg(all(test, feature = "aggregate"))]
mod tests;

pub(super) fn main() {
    let code = match arguments::parse(std::env::args_os().skip(1)) {
        Ok(arguments::Command::Legacy) => {
            legacy::main();
            return;
        }
        Err(error) => {
            eprintln!("{error}");
            2
        }
        Ok(arguments::Command::Aggregate(command)) => run(command),
    };
    std::process::exit(code);
}

#[cfg(not(feature = "aggregate"))]
fn run(_: arguments::AggregateCommand) -> i32 {
    eprintln!("aggregate operation failed: conformance runner aggregate feature is not enabled");
    3
}

#[cfg(feature = "aggregate")]
fn run(command: arguments::AggregateCommand) -> i32 {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("crate is a direct workspace child");
    // Preparation completes before stdout is acquired or written.
    let result = aggregate::prepare(root, command)
        .and_then(|artifact| publication::publish(artifact, &mut std::io::stdout().lock()));
    match result {
        Ok(code) => code,
        Err(error) => {
            eprintln!("{error}");
            error.exit_code()
        }
    }
}
