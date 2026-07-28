use forgetest_report::evidence::{write_artifact_manifest, ArtifactManifest};

#[test]
fn artifact_manifest_hashes_bundle_files_deterministically() {
    let output = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(output.path().join("trials/task")).unwrap();
    std::fs::write(output.path().join("report.json"), b"{\"ok\":true}\n").unwrap();
    std::fs::write(
        output.path().join("trials/task/trace.jsonl"),
        b"{\"event\":\"done\"}\n",
    )
    .unwrap();

    let manifest_path = write_artifact_manifest(output.path()).unwrap();
    let first = std::fs::read(&manifest_path).unwrap();
    let manifest: ArtifactManifest = serde_json::from_slice(&first).unwrap();

    assert_eq!(manifest.schema_version, 1);
    assert_eq!(manifest.files.len(), 2);
    assert_eq!(manifest.files[0].path, "report.json");
    assert_eq!(manifest.files[0].media_type, "application/json");
    assert_eq!(manifest.files[0].sha256.len(), 64);
    assert_eq!(manifest.files[1].path, "trials/task/trace.jsonl");
    assert_eq!(manifest.files[1].media_type, "application/x-ndjson");
    assert!(manifest
        .files
        .iter()
        .all(|file| file.path != "artifact-manifest.json"));

    write_artifact_manifest(output.path()).unwrap();
    assert_eq!(std::fs::read(manifest_path).unwrap(), first);
}
