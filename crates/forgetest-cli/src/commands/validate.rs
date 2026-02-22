//! The `forgetest validate` command.

use std::path::PathBuf;

use anyhow::Result;

pub fn execute(eval_set_path: PathBuf) -> Result<()> {
    let sets = if eval_set_path.is_dir() {
        forgetest_core::parser::load_eval_directory(&eval_set_path)?
    } else {
        vec![forgetest_core::parser::parse_eval_set(&eval_set_path)?]
    };

    let mut total_warnings = 0;

    for set in &sets {
        println!("Eval set: {} ({} cases)", set.name, set.cases.len());

        let warnings = forgetest_core::parser::validate_eval_set(set);
        for w in &warnings {
            let prefix = w
                .case_id
                .as_ref()
                .map(|id| format!("  [{id}]"))
                .unwrap_or_else(|| "  ".to_string());
            println!("{prefix} WARNING: {}", w.message);
        }
        total_warnings += warnings.len();
    }

    if total_warnings == 0 {
        println!("All eval sets valid.");
    } else {
        println!("\n{total_warnings} warning(s) found.");
    }

    Ok(())
}
