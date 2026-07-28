//! Integrity metadata for private and redacted evidence bundles.

use std::fs;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const MANIFEST_NAME: &str = "artifact-manifest.json";
const TEMP_MANIFEST_NAME: &str = ".artifact-manifest.json.tmp";

/// Deterministic inventory for one evidence bundle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactManifest {
    pub schema_version: u32,
    pub hash_algorithm: String,
    pub files: Vec<ArtifactFile>,
}

/// Integrity record for one artifact relative to the bundle root.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactFile {
    pub path: String,
    pub sha256: String,
    pub size_bytes: u64,
    pub media_type: String,
}

/// Inventory every regular file in `root`, excluding the manifest itself.
pub fn write_artifact_manifest(root: &Path) -> Result<PathBuf> {
    anyhow::ensure!(
        root.is_dir(),
        "evidence bundle does not exist: {}",
        root.display()
    );
    let mut paths = Vec::new();
    collect_files(root, root, &mut paths)?;
    paths.sort();

    let files = paths
        .into_iter()
        .map(|path| {
            let relative = relative_path(root, &path)?;
            let metadata = fs::metadata(&path)?;
            Ok(ArtifactFile {
                media_type: media_type(&relative).into(),
                path: relative,
                sha256: sha256_file(&path)?,
                size_bytes: metadata.len(),
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let manifest = ArtifactManifest {
        schema_version: 1,
        hash_algorithm: "sha256".into(),
        files,
    };
    let mut serialized = serde_json::to_vec_pretty(&manifest)?;
    serialized.push(b'\n');

    let temporary = root.join(TEMP_MANIFEST_NAME);
    let destination = root.join(MANIFEST_NAME);
    fs::write(&temporary, serialized)
        .with_context(|| format!("failed to write {}", temporary.display()))?;
    fs::rename(&temporary, &destination)
        .with_context(|| format!("failed to publish {}", destination.display()))?;
    Ok(destination)
}

fn collect_files(root: &Path, current: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    let mut entries = fs::read_dir(current)?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<std::io::Result<Vec<_>>>()?;
    entries.sort();
    for path in entries {
        let metadata = fs::symlink_metadata(&path)?;
        anyhow::ensure!(
            !metadata.file_type().is_symlink(),
            "evidence bundles cannot contain symlinks: {}",
            path.display()
        );
        if metadata.is_dir() {
            collect_files(root, &path, files)?;
        } else if metadata.is_file() {
            let relative = relative_path(root, &path)?;
            if relative != MANIFEST_NAME && relative != TEMP_MANIFEST_NAME {
                files.push(path);
            }
        } else {
            anyhow::bail!("unsupported evidence artifact: {}", path.display());
        }
    }
    Ok(())
}

fn relative_path(root: &Path, path: &Path) -> Result<String> {
    Ok(path
        .strip_prefix(root)
        .context("evidence artifact is outside bundle root")?
        .to_string_lossy()
        .replace('\\', "/"))
}

fn sha256_file(path: &Path) -> Result<String> {
    let mut reader = BufReader::new(fs::File::open(path)?);
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn media_type(path: &str) -> &'static str {
    if path.ends_with(".jsonl") {
        "application/x-ndjson"
    } else if path.ends_with(".sarif") {
        "application/sarif+json"
    } else if path.ends_with(".json") {
        "application/json"
    } else if path.ends_with(".html") {
        "text/html"
    } else if path.ends_with(".patch") || path.ends_with(".diff") {
        "text/x-diff"
    } else if path.ends_with(".log") || path.ends_with(".txt") {
        "text/plain"
    } else {
        "application/octet-stream"
    }
}
