//! Command-line argument types for configuring the language server.
//!
//! This module documents the command-line concepts that `achitek-ls` accepts,
//! including the communication channel selected by the client or editor integration.

use lexopt::{
    Arg::{Long, Short},
    Parser,
};
use std::{fmt::Display, path::PathBuf};

/// Communication channel used by the language server.
#[derive(Debug, PartialEq, Eq, Default)]
pub enum CommunicationsChannel {
    /// Use standard input and standard output for JSON-RPC messages.
    #[default]
    Stdio,
    /// Use a named pipe on Windows or a Unix socket file on Linux and macOS.
    Pipe { path: PathBuf },
    /// Use a TCP socket listening on the given port.
    Socket { port: u16 },
    /// Use Node.js IPC when the server is launched from a Node process.
    NodeIpc,
}

impl Display for CommunicationsChannel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Stdio => f.write_str("stdio"),
            Self::Pipe { path } => write!(f, "pipe:{}", path.display()),
            Self::Socket { port } => write!(f, "socket:{port}"),
            Self::NodeIpc => f.write_str("node-ipc"),
        }
    }
}

/// Command line arguments
pub struct Args {
    /// The version of this binary as defined in Cargo.toml
    pub version: String,
    /// Communication channel selected for the language server.
    pub channel: Option<CommunicationsChannel>,
}

#[doc(hidden)]
const HELP_TEXT: &str = r#"
Usage: achitek-ls [ARGS]

ARGS:
  -v, --version  Print version
      --stdio    Uses stdio as the communication channel
  -h, --help     Print help
"#;

/// Parses command-line arguments into language server configuration.
///
/// Prints help or version information and exits the process when `--help`,
/// `-h`, `--version`, or `-v` is supplied.
#[doc(hidden)]
pub fn parse() -> Result<Args, lexopt::Error> {
    parse_parser(Parser::from_env())
}

fn parse_parser(mut parser: Parser) -> Result<Args, lexopt::Error> {
    let mut version = "".to_string();
    let mut channel = Some(CommunicationsChannel::default());

    while let Some(arg) = parser.next()? {
        match arg {
            Short('v') | Long("version") => {
                version = env!("CARGO_PKG_VERSION").to_string();
                println!("achitek-ls {version}");
                std::process::exit(0);
            }
            Short('h') | Long("help") => {
                println!("{HELP_TEXT}");
                std::process::exit(0);
            }
            Long("stdio") => channel = Some(CommunicationsChannel::Stdio),
            _ => return Err(arg.unexpected()),
        }
    }

    Ok(Args { version, channel })
}

#[cfg(test)]
mod tests {
    use super::{CommunicationsChannel, parse_parser};
    use lexopt::Parser;

    #[test]
    fn defaults_to_stdio_when_no_channel_is_provided() {
        let args = parse_parser(Parser::from_args([] as [&str; 0])).unwrap();

        assert_eq!(args.channel, Some(CommunicationsChannel::Stdio));
    }

    #[test]
    fn keeps_explicit_stdio_channel() {
        let args = parse_parser(Parser::from_args(["--stdio"])).unwrap();

        assert_eq!(args.channel, Some(CommunicationsChannel::Stdio));
    }
}
