use cellscript_artifact_checker::{check_bundle, CheckerBudgets};
use clap::Parser;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(name = "cellscript-artifact-checker")]
#[command(about = "Bounded independent CellScript lowering-record and CKB ELF checker")]
struct Args {
    #[arg(long)]
    artifact: PathBuf,
    #[arg(long)]
    metadata: PathBuf,
    #[arg(long = "lowering-record")]
    lowering_record: PathBuf,
    #[arg(long = "source-map")]
    source_map: PathBuf,
    #[arg(long)]
    policy: Option<PathBuf>,
}

fn main() {
    let args = Args::parse();
    match run(args) {
        Ok(report) => match serde_json::to_string(&report) {
            Ok(json) => println!("{json}"),
            Err(error) => {
                eprintln!("V2401: failed to serialize checker report: {error}");
                std::process::exit(2);
            }
        },
        Err(error) => {
            let json = serde_json::to_string(&error)
                .unwrap_or_else(|_| format!(r#"{{"code":"{}","message":"checker rejection"}}"#, error.code.as_str()));
            eprintln!("{json}");
            std::process::exit(1);
        }
    }
}

fn run(args: Args) -> Result<cellscript_artifact_checker::CheckerReport, cellscript_artifact_checker::CheckerError> {
    let budgets = match args.policy {
        Some(path) => {
            let bytes = std::fs::read(&path).map_err(|error| io_error("checker policy", &path, error))?;
            serde_json::from_slice::<CheckerBudgets>(&bytes).map_err(|error| cellscript_artifact_checker::CheckerError {
                code: cellscript_artifact_checker::CheckerRejectionCode::V2401MalformedJson,
                message: format!("failed to parse checker policy '{}': {error}", path.display()),
            })?
        }
        None => CheckerBudgets::default(),
    };
    let artifact = std::fs::read(&args.artifact).map_err(|error| io_error("artifact", &args.artifact, error))?;
    let metadata = std::fs::read(&args.metadata).map_err(|error| io_error("metadata", &args.metadata, error))?;
    let record = std::fs::read(&args.lowering_record).map_err(|error| io_error("lowering record", &args.lowering_record, error))?;
    let source_map = std::fs::read(&args.source_map).map_err(|error| io_error("source map", &args.source_map, error))?;
    check_bundle(&artifact, &metadata, &record, &source_map, &budgets)
}

fn io_error(label: &str, path: &std::path::Path, error: std::io::Error) -> cellscript_artifact_checker::CheckerError {
    cellscript_artifact_checker::CheckerError {
        code: cellscript_artifact_checker::CheckerRejectionCode::V2401MalformedJson,
        message: format!("failed to read {label} '{}': {error}", path.display()),
    }
}
