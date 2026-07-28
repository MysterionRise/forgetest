//! Constrained Harbor bridge commands.

use std::path::PathBuf;

use anyhow::Result;
use forgetest_core::harbor::{export_suite_to_harbor, import_harbor_task, HarborImportMetadata};
use forgetest_core::suite::load_suite;

pub fn export(suite: PathBuf, output: PathBuf, base_image: String) -> Result<()> {
    let suite = load_suite(&suite)?;
    export_suite_to_harbor(&suite, &output, &base_image)?;
    println!(
        "Exported {} forgetest-marked Harbor task(s) to {}",
        suite.tasks.len(),
        output.display()
    );
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn import(
    task: PathBuf,
    output: PathBuf,
    suite_id: String,
    suite_name: String,
    source_url: String,
    source_revision: String,
    license: String,
) -> Result<()> {
    import_harbor_task(
        &task,
        &output,
        &HarborImportMetadata {
            suite_id,
            suite_name,
            source_url,
            source_revision,
            license,
        },
    )?;
    println!(
        "Imported supported Harbor task into {}",
        output.join("suite.toml").display()
    );
    Ok(())
}
