mod ocr;
mod proxy;
mod redact;
mod secrets;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "eyesoff", version, about = "Keep secrets out of what your coding agent sends to the model")]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    #[command(about = "Run the proxy that hides secrets from the model")]
    Start {
        #[arg(long, default_value_t = 8787)]
        port: u16,
    },
    #[command(about = "Save the clipboard into an env file, or pipe it into a command after --")]
    Paste {
        name: String,
        #[arg(long = "env", value_name = "FILE")]
        env_file: Option<PathBuf>,
        #[arg(last = true)]
        command: Vec<String>,
    },
}

fn main() -> ExitCode {
    let result = match Cli::parse().command {
        Cmd::Start { port } => proxy::start(port),
        Cmd::Paste { name, env_file, command } => secrets::paste_from_clipboard(&name, env_file.as_deref(), &command),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("eyesoff: {e:#}");
            ExitCode::FAILURE
        }
    }
}
