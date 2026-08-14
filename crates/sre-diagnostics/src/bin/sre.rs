use sre_diagnostics::{default_data_directory, run_doctor};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
struct DoctorOptions {
    json: bool,
    data_directory: PathBuf,
    runtime_executable: Option<PathBuf>,
}

fn usage() -> &'static str {
    "Usage: sre doctor [--json] [--data-dir <path>] [--runtime-executable <path>]"
}

fn doctor_options(arguments: impl IntoIterator<Item = String>) -> Result<DoctorOptions, String> {
    let mut arguments = arguments.into_iter();
    if arguments.next().as_deref() != Some("doctor") {
        return Err(usage().to_owned());
    }

    let mut options = DoctorOptions {
        json: false,
        data_directory: default_data_directory(),
        runtime_executable: None,
    };
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--json" => options.json = true,
            "--data-dir" => {
                options.data_directory = PathBuf::from(
                    arguments
                        .next()
                        .ok_or_else(|| "--data-dir requires a path".to_owned())?,
                );
            }
            "--runtime-executable" => {
                options.runtime_executable =
                    Some(PathBuf::from(arguments.next().ok_or_else(|| {
                        "--runtime-executable requires a path".to_owned()
                    })?));
            }
            "--help" | "-h" => return Err(usage().to_owned()),
            _ => return Err(format!("Unknown argument {argument:?}.\n{}", usage())),
        }
    }
    Ok(options)
}

fn main() {
    let options = match doctor_options(std::env::args().skip(1)) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(2);
        }
    };
    let report = run_doctor(
        &options.data_directory,
        options.runtime_executable.as_deref(),
    );
    if options.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&report).expect("SRE Doctor report must be serializable")
        );
        return;
    }

    println!("{} Doctor ({})", report.product, report.platform);
    for check in report.checks {
        println!("{:?}: {} — {}", check.status, check.id, check.summary);
        if let Some(remediation) = check.remediation {
            println!("  {remediation}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_the_documented_json_command() {
        let options = doctor_options(["doctor".to_owned(), "--json".to_owned()]).unwrap();
        assert!(options.json);
    }

    #[test]
    fn rejects_missing_option_values() {
        assert!(doctor_options(["doctor".to_owned(), "--data-dir".to_owned()]).is_err());
    }
}
