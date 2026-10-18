//! mch — the McHarness binary. A minimal command surface that proves the
//! crate composition: scope authority, completion gates, technique ledger.

mod scope_cli;

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(code) => code,
    }
}

fn run(args: &[String]) -> Result<(), ExitCode> {
    match args.get(1).map(String::as_str) {
        None | Some("help") => {
            println!("{}", USAGE);
            Ok(())
        }
        Some("check-scope") => {
            let host = args
                .get(2)
                .ok_or_else(|| {
                    eprintln!("check-scope requires a host argument");
                    ExitCode::from(2)
                })?;
            match scope_cli::check(host) {
                Ok(v) => {
                    println!("{v}");
                    Ok(())
                }
                Err(e) => {
                    eprintln!("DENIED: {e}");
                    Err(ExitCode::from(3))
                }
            }
        }
        Some(other) => {
            eprintln!("unknown command: {other}");
            println!("{}", USAGE);
            Err(ExitCode::from(2))
        }
    }
}

pub const USAGE: &str = "mch — McHarness\n\nUSAGE:\n  mch help                 Show this help\n  mch check-scope <host>   Evaluate a host against the scope authority\n\nEXIT CODES:\n  0  allowed / ok\n  2  usage error\n  3  denied (fail-closed)";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usage_nonempty() {
        assert!(USAGE.contains("mch"));
        assert!(USAGE.contains("check-scope"));
    }

    #[test]
    fn authority_helper_parses() {
        let toml = "[in_scope]\nhosts = [\"example.com\"]\n";
        let auth = mch_scope::ScopeAuthority::from_toml(toml).unwrap();
        assert!(auth.authorize("example.com").is_ok());
        assert!(auth.authorize("evil.com").is_err());
    }
}
