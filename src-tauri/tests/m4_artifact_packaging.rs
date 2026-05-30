use std::fs;
use std::path::PathBuf;
use std::process::Command;

#[test]
fn tauri_artifact_tarball_contains_manifest_and_editor_assets() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = manifest_dir.parent().expect("repo root");
    let out_dir = std::env::temp_dir().join("monaco-tauri-artifact-test");
    let _ = fs::remove_dir_all(&out_dir);
    fs::create_dir_all(&out_dir).expect("create temp output dir");

    let script = repo_root.join("scripts/package-tauri-artifact.sh");
    let status = Command::new("bash")
        .arg(script)
        .arg("--out-dir")
        .arg(&out_dir)
        .current_dir(repo_root)
        .status()
        .expect("run artifact packager");
    assert!(status.success(), "artifact packager should succeed");

    let cargo_manifest =
        fs::read_to_string(manifest_dir.join("Cargo.toml")).expect("read Cargo.toml");
    let version = cargo_manifest
        .lines()
        .find_map(|line| line.trim().strip_prefix("version = "))
        .map(|value| value.trim_matches('"').to_string())
        .expect("Cargo.toml version string");

    let tarball = out_dir.join(format!("monaco-tauri-artifact-v{version}.tar.gz"));
    assert!(tarball.is_file(), "expected artifact tarball");

    let tar_output = Command::new("tar")
        .arg("-tzf")
        .arg(&tarball)
        .output()
        .expect("list tarball contents");
    assert!(
        tar_output.status.success(),
        "tarball should be readable: {}",
        String::from_utf8_lossy(&tar_output.stderr)
    );

    let listing = String::from_utf8(tar_output.stdout).expect("utf8 tar listing");
    assert!(listing.contains("artifact-manifest.json"));
    assert!(listing.contains("tauri-dist/index.html"));
    assert!(listing.contains("out/monaco-editor/min/vs/loader.js"));
    assert!(listing.contains("proto/ipc_editor_host.proto"));
}
