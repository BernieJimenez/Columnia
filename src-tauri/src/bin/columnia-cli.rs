use std::{env, io, process::ExitCode};

use columnia_lib::automation::{self, CliCommand};

fn main() -> ExitCode {
    match run() {
        Ok(exit_code) => exit_code,
        Err(error) => {
            eprintln!("Error: {error}");
            eprintln!("Usa columnia-cli --help para ver la interfaz admitida.");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<ExitCode, Box<dyn std::error::Error>> {
    let command = automation::parse_cli_args(env::args_os().skip(1))?;
    match command {
        CliCommand::Help(text) => println!("{text}"),
        CliCommand::Inspect {
            input,
            sheet,
            header,
        } => {
            serde_json::to_writer(
                io::stdout().lock(),
                &automation::inspect(&input, sheet.as_deref(), header)?,
            )?;
            println!();
        }
        CliCommand::Transform {
            input,
            sheet,
            header,
            recipe,
            output,
            format,
        } => {
            serde_json::to_writer(
                io::stdout().lock(),
                &automation::transform(&input, sheet.as_deref(), header, &recipe, &output, format)?,
            )?;
            println!();
        }
        CliCommand::Validate {
            input,
            sheet,
            header,
            rules,
        } => {
            let output = automation::validate(&input, sheet.as_deref(), header, &rules)?;
            let passed = output.passed();
            serde_json::to_writer(io::stdout().lock(), &output)?;
            println!();
            if passed {
                return Ok(ExitCode::SUCCESS);
            }
            return Ok(ExitCode::from(2));
        }
        CliCommand::Batch { manifest } => {
            let output = automation::batch(&manifest)?;
            let failed = output.failed();
            serde_json::to_writer(io::stdout().lock(), &output)?;
            println!();
            if failed {
                return Ok(ExitCode::from(2));
            }
        }
    }
    Ok(ExitCode::SUCCESS)
}
