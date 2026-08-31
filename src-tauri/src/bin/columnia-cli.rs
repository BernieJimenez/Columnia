use std::{env, io, process::ExitCode};

use columnia_lib::{
    automation::{self, CliCommand},
    privacy,
};

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
            privacy::write_sanitized_json(
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
            privacy::write_sanitized_json(
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
            privacy::write_sanitized_json(io::stdout().lock(), &output)?;
            println!();
            if passed {
                return Ok(ExitCode::SUCCESS);
            }
            return Ok(ExitCode::from(2));
        }
        CliCommand::QualityMigrationReport { rules } => {
            let output = automation::quality_migration_report(&rules)?;
            let requires_manual_review = output.requires_manual_review();
            privacy::write_sanitized_json(io::stdout().lock(), &output)?;
            println!();
            if requires_manual_review {
                return Ok(ExitCode::from(2));
            }
        }
        CliCommand::Batch { manifest, force } => {
            let output = automation::batch_with_options(&manifest, force)?;
            let failed = output.failed();
            privacy::write_sanitized_json(io::stdout().lock(), &output)?;
            println!();
            if failed {
                return Ok(ExitCode::from(2));
            }
        }
        CliCommand::ProjectList { store } => {
            privacy::write_sanitized_json(io::stdout().lock(), &automation::project_list(&store)?)?;
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
            privacy::write_sanitized_json(
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
            privacy::write_sanitized_json(
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
            privacy::write_sanitized_json(io::stdout().lock(), &result)?;
            println!();
            if blocked {
                return Ok(ExitCode::from(2));
            }
        }
        CliCommand::ProjectDelete { store, id, confirm } => {
            privacy::write_sanitized_json(
                io::stdout().lock(),
                &automation::project_delete(&store, id, &confirm)?,
            )?;
            println!();
        }
    }
    Ok(ExitCode::SUCCESS)
}
