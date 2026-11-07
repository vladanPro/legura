//! Read-only integrity check for a freshly extracted native package.

use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    io::Read,
    path::{Path, PathBuf},
};

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PackageManifest {
    format: u32,
    product: String,
    version: String,
    platform: String,
    files: Vec<PackageFile>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PackageFile {
    path: String,
    sha256: String,
}

pub fn verify(root: &Path) -> Result<()> {
    ensure!(
        real_file_type(root)?.is_dir(),
        "package root must be a real directory"
    );
    let manifest_path = root.join("package.json");
    ensure!(
        real_file_type(&manifest_path)?.is_file(),
        "package manifest must be a regular file"
    );
    ensure!(
        fs::metadata(&manifest_path)?.len() <= 2 * 1024 * 1024,
        "package manifest is too large"
    );
    let manifest: PackageManifest = serde_json::from_reader(fs::File::open(&manifest_path)?)
        .context("invalid package manifest")?;
    ensure!(
        manifest.format == 1
            && manifest.product == "legura"
            && manifest.version == env!("CARGO_PKG_VERSION")
            && !manifest.platform.is_empty(),
        "unsupported package format, product, version or platform"
    );
    ensure!(
        !manifest.files.is_empty() && manifest.files.len() <= 10_000,
        "invalid package file count"
    );

    let mut listed = BTreeMap::new();
    for file in &manifest.files {
        ensure!(
            valid_package_path(&file.path),
            "invalid package path: {}",
            file.path
        );
        ensure!(
            file.sha256.len() == 64
                && file
                    .sha256
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase()),
            "invalid package checksum for {}",
            file.path
        );
        ensure!(
            listed
                .insert(file.path.to_ascii_lowercase(), file)
                .is_none(),
            "duplicate package path: {}",
            file.path
        );
    }
    let executable_suffix = if cfg!(windows) { ".exe" } else { "" };
    for required in [
        format!("legura-server{executable_suffix}"),
        format!("legura-maintenance{executable_suffix}"),
        "README.md".into(),
        "native-service-v0.md".into(),
        "start.ps1".into(),
    ] {
        ensure!(
            listed.contains_key(&required.to_ascii_lowercase()),
            "package missing required file: {required}"
        );
    }
    ensure!(
        listed.keys().any(|path| path.starts_with("dist/")),
        "package has no runtime assets"
    );

    let mut actual = BTreeMap::new();
    collect_files(root, root, &mut actual)?;
    for (key, file) in &listed {
        ensure!(
            actual.contains_key(key),
            "package file missing: {}",
            file.path
        );
    }
    for (key, path) in &actual {
        ensure!(
            listed.contains_key(key),
            "unexpected package file: {}",
            path.display()
        );
    }
    for file in &manifest.files {
        let path = root.join(&file.path);
        ensure!(
            sha256(&path)? == file.sha256,
            "package checksum mismatch: {}",
            file.path
        );
    }
    Ok(())
}

fn valid_package_path(path: &str) -> bool {
    if path.contains('\\') || path.contains(':') || path.chars().any(char::is_control) {
        return false;
    }
    let parts: Vec<_> = path.split('/').collect();
    if parts
        .iter()
        .any(|part| part.is_empty() || *part == "." || *part == "..")
    {
        return false;
    }
    matches!(
        path,
        "README.md"
            | "native-service-v0.md"
            | "start.ps1"
            | "legura-server"
            | "legura-server.exe"
            | "legura-maintenance"
            | "legura-maintenance.exe"
    ) || (parts.len() >= 2 && parts[0] == "dist")
}

fn real_file_type(path: &Path) -> Result<fs::FileType> {
    let metadata = fs::symlink_metadata(path)?;
    ensure!(
        !metadata.file_type().is_symlink(),
        "package contains a link or reparse point: {}",
        path.display()
    );
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        ensure!(
            metadata.file_attributes() & 0x400 == 0,
            "package contains a link or reparse point: {}",
            path.display()
        );
    }
    Ok(metadata.file_type())
}

fn collect_files(
    root: &Path,
    directory: &Path,
    found: &mut BTreeMap<String, PathBuf>,
) -> Result<()> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        let kind = real_file_type(&path)?;
        if kind.is_dir() {
            collect_files(root, &path, found)?;
        } else {
            ensure!(
                kind.is_file(),
                "package contains a non-file entry: {}",
                path.display()
            );
            let relative = path
                .strip_prefix(root)?
                .to_str()
                .context("package path is not UTF-8")?
                .replace('\\', "/");
            if relative == "package.json" {
                continue;
            }
            ensure!(
                found.insert(relative.to_ascii_lowercase(), path).is_none(),
                "package contains duplicate file names"
            );
        }
    }
    Ok(())
}

fn sha256(path: &Path) -> Result<String> {
    ensure!(
        real_file_type(path)?.is_file(),
        "package file must be regular"
    );
    let mut input = fs::File::open(path)?;
    let mut digest = Sha256::new();
    let mut buffer = [0; 65536];
    loop {
        let count = input.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT: AtomicU64 = AtomicU64::new(0);

    struct Fixture(PathBuf);

    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "legura-package-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&root).unwrap();
            let suffix = if cfg!(windows) { ".exe" } else { "" };
            let paths = [
                format!("legura-server{suffix}"),
                format!("legura-maintenance{suffix}"),
                "README.md".into(),
                "native-service-v0.md".into(),
                "start.ps1".into(),
                "dist/site.css".into(),
            ];
            let files = paths
                .iter()
                .map(|path| {
                    let target = root.join(path);
                    fs::create_dir_all(target.parent().unwrap()).unwrap();
                    fs::write(&target, path.as_bytes()).unwrap();
                    PackageFile {
                        path: path.clone(),
                        sha256: sha256(&target).unwrap(),
                    }
                })
                .collect();
            let manifest = PackageManifest {
                format: 1,
                product: "legura".into(),
                version: env!("CARGO_PKG_VERSION").into(),
                platform: "test-native".into(),
                files,
            };
            fs::write(
                root.join("package.json"),
                serde_json::to_vec(&manifest).unwrap(),
            )
            .unwrap();
            Self(root)
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn complete_package_passes_and_changes_fail_closed() {
        let fixture = Fixture::new();
        verify(&fixture.0).unwrap();
        fs::write(fixture.0.join("dist/site.css"), "modified").unwrap();
        assert!(verify(&fixture.0)
            .unwrap_err()
            .to_string()
            .contains("checksum mismatch"));
        fs::write(fixture.0.join("dist/site.css"), "dist/site.css").unwrap();
        fs::write(fixture.0.join("extra.txt"), "extra").unwrap();
        assert!(verify(&fixture.0)
            .unwrap_err()
            .to_string()
            .contains("unexpected package file"));
        fs::remove_file(fixture.0.join("extra.txt")).unwrap();
        fs::remove_file(fixture.0.join("native-service-v0.md")).unwrap();
        assert!(verify(&fixture.0)
            .unwrap_err()
            .to_string()
            .contains("package file missing"));
        fs::write(
            fixture.0.join("native-service-v0.md"),
            "native-service-v0.md",
        )
        .unwrap();
        fs::remove_file(fixture.0.join("README.md")).unwrap();
        assert!(verify(&fixture.0)
            .unwrap_err()
            .to_string()
            .contains("package file missing"));
    }

    #[test]
    fn unsafe_manifest_paths_are_rejected_before_file_access() {
        let fixture = Fixture::new();
        let path = fixture.0.join("package.json");
        let mut manifest: PackageManifest =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        manifest.files[0].path = "../outside".into();
        fs::write(path, serde_json::to_vec(&manifest).unwrap()).unwrap();
        assert!(verify(&fixture.0)
            .unwrap_err()
            .to_string()
            .contains("invalid package path"));
    }

    #[cfg(unix)]
    #[test]
    fn symbolic_link_in_package_is_rejected() {
        let fixture = Fixture::new();
        std::os::unix::fs::symlink("site.css", fixture.0.join("dist/alias.css")).unwrap();
        assert!(verify(&fixture.0)
            .unwrap_err()
            .to_string()
            .contains("link or reparse point"));
    }
}
