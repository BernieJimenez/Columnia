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
        CliCommand::ProjectList { store } => {
            serde_json::to_writer(io::stdout().lock(), &automation::project_list(&store)?)?;
            println!();
        }
        CliCommand::ProjectSave {
            store,
            name,
            input,
            id,
            sheet,
            header,
            recipe,
            rules,
            profile,
        } => {
            serde_json::to_writer(
                io::stdout().lock(),
                &automation::project_save(
                    &store,
                    name,
                    &input,
                    id,
                    sheet.as_deref(),
                    header,
                    recipe.as_deref(),
                    rules.as_deref(),
                    profile,
                )?,
            )?;
            println!();
        }
        CliCommand::ProjectInspect { store, id } => {
            serde_json::to_writer(
                io::stdout().lock(),
                &automation::project_inspect(&store, &id)?,
            )?;
            println!();
        }
        CliCommand::ProjectExport {
            store,
            id,
            output,
            format,
            allow_unvalidated,
        } => {
            let result =
                automation::project_export(&store, &id, &output, format, allow_unvalidated)?;
            let blocked = result.blocked();
            serde_json::to_writer(io::stdout().lock(), &result)?;
            println!();
            if blocked {
                return Ok(ExitCode::from(2));
            }
        }
        CliCommand::ProjectDelete { store, id, confirm } => {
            serde_json::to_writer(
                io::stdout().lock(),
                &automation::project_delete(&store, id, &confirm)?,
            )?;
            println!();
        }
    }
    Ok(ExitCode::SUCCESS)
}
