// SPDX-License-Identifier: MIT

//! Binary entry point. Delegates to [`rprof::cli::run`] and maps its
//! result to a process exit code; any error is printed and becomes exit 1.

use std::process::ExitCode;

fn main() -> ExitCode {
    match rprof::cli::run() {
        Ok(code) => ExitCode::from(code),
        Err(err) => {
            eprintln!("rprof: {err:#}");
            ExitCode::from(1)
        }
    }
}
