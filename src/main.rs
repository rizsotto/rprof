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
