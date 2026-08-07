use hmm_games_mhw::validate_mhw_equipment_candidate_catalog;
use std::env;
use std::fs;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut require_bundled = false;
    let mut candidate_path = None;

    for argument in env::args_os().skip(1) {
        if argument == "--require-bundled" {
            require_bundled = true;
        } else if candidate_path.replace(argument).is_some() {
            eprintln!("usage: validate_equipment_candidates [--require-bundled] <candidate.json>");
            return ExitCode::from(64);
        }
    }

    let Some(candidate_path) = candidate_path else {
        eprintln!("usage: validate_equipment_candidates [--require-bundled] <candidate.json>");
        return ExitCode::from(64);
    };
    let Ok(source) = fs::read_to_string(candidate_path) else {
        eprintln!("candidate JSON could not be read");
        return ExitCode::from(66);
    };
    let report = match validate_mhw_equipment_candidate_catalog(&source) {
        Ok(report) => report,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(65);
        }
    };

    match serde_json::to_string_pretty(&report) {
        Ok(output) => println!("{output}"),
        Err(_) => {
            eprintln!("candidate validation report could not be serialized");
            return ExitCode::from(70);
        }
    }

    if !report.valid {
        ExitCode::from(1)
    } else if require_bundled && !report.bundled_eligible {
        ExitCode::from(2)
    } else {
        ExitCode::SUCCESS
    }
}
