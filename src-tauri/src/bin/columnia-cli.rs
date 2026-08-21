use std::{env, io, process::ExitCode};

use columnia_lib::automation::{self, CliCommand};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("Error: {error}");
            eprintln!("Usa columnia-cli --help para ver la interfaz admitida.");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let command = automation::parse_cli_args(env::args_os().skip(1))?;
    match command {
        CliCommand::Help(text) => println!("{text}"),
        CliCommand::Inspect { input } => {
            serde_json::to_writer(io::stdout().lock(), &automation::inspect(&input)?)?;
            println!();
        }
        CliCommand::Transform {
            input,
            recipe,
            output,
            format,
        } => {
            serde_json::to_writer(
                io::stdout().lock(),
                &automation::transform(&input, &recipe, &output, format)?,
            )?;
            println!();
        }
    }
    Ok(())
}
