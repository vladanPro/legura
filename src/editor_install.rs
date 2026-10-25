//! Side-by-side editor installation. Never change the application's active imports.
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};

#[derive(Debug, serde::Serialize)]
pub struct Installation {
    pub generation: String,
    pub component: String,
    pub stylesheet: String,
    pub module: String,
    pub installed: bool,
}

struct Plan {
    root: PathBuf,
    report: Installation,
    directories: Vec<String>,
    files: BTreeMap<String, &'static str>,
    receipt: String,
    inventory: Vec<u8>,
}

fn conflict(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn linked(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}

// Reject linked ancestors even when their destination happens to be inside root.
fn checked_path(root: &Path, relative: &str) -> io::Result<PathBuf> {
    let mut current = root.to_path_buf();
    for part in Path::new(relative).components() {
        let Component::Normal(part) = part else {
            return Err(conflict("Invalid editor installation path"));
        };
        current.push(part);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if linked(&metadata) => {
                return Err(conflict("Editor installation refuses linked paths"));
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    Ok(current)
}

fn plan(project: &Path) -> io::Result<Plan> {
    let root = project.canonicalize()?;
    if !root.is_dir() || !checked_path(&root, "Cargo.toml")?.is_file() {
        return Err(conflict("Expected an existing CMS project with Cargo.toml"));
    }
    let assets = legura_editor::assets::assets();
    let inventory = serde_json::to_vec(&crate::editor_assets::manifest()?)?;
    let generation = format!("{:x}", Sha256::digest(&inventory));
    let component_dir = format!("app/components/legura-editor/{generation}");
    let public_dir = format!("public/content-editor/{generation}");
    let mut files = BTreeMap::new();
    for asset in assets {
        let (directory, name) = if let Some(name) = asset.path.strip_prefix("components/") {
            (&component_dir, name)
        } else if let Some(name) = asset.path.strip_prefix("public/") {
            (&public_dir, name)
        } else {
            return Err(conflict("Unsupported editor asset namespace"));
        };
        // V0 deliberately accepts flat package directories only.
        if name.is_empty() || name.contains(['/', '\\', ':']) || name == "." || name == ".." {
            return Err(conflict("Unsupported editor asset path"));
        }
        files.insert(format!("{directory}/{name}"), asset.content);
    }
    for required in [
        format!("{component_dir}/RichTextEditor.asx"),
        format!("{public_dir}/editor.css"),
        format!("{public_dir}/rich-text-editor.js"),
    ] {
        if !files.contains_key(&required) {
            return Err(conflict("Editor bundle is missing a required entry point"));
        }
    }
    let receipt = format!(".legura/editor-assets/{generation}.json");
    for path in files.keys().chain(std::iter::once(&receipt)) {
        checked_path(&root, path)?;
    }
    Ok(Plan {
        root,
        report: Installation {
            component: format!("{component_dir}/RichTextEditor.asx"),
            stylesheet: format!("/content-editor/{generation}/editor.css"),
            module: format!("/content-editor/{generation}/rich-text-editor.js"),
            generation,
            installed: false,
        },
        directories: vec![component_dir, public_dir],
        files,
        receipt,
        inventory,
    })
}

fn verify(plan: &mut Plan) -> io::Result<()> {
    let receipt = checked_path(&plan.root, &plan.receipt)?;
    if !receipt.try_exists()? {
        for directory in &plan.directories {
            if checked_path(&plan.root, directory)?.try_exists()? {
                return Err(conflict(
                    "Incomplete or unowned editor generation; no files changed",
                ));
            }
        }
        return Ok(());
    }
    if !fs::metadata(&receipt)?.is_file() || fs::read(receipt)? != plan.inventory {
        return Err(conflict("Editor receipt differs; no files changed"));
    }
    for (relative, content) in &plan.files {
        let path = checked_path(&plan.root, relative)?;
        if !fs::metadata(&path)?.is_file() || fs::read(path)? != content.as_bytes() {
            return Err(conflict("Editor asset has local changes; no files changed"));
        }
    }
    for directory in &plan.directories {
        for entry in fs::read_dir(checked_path(&plan.root, directory)?)? {
            let entry = entry?;
            let relative = format!("{directory}/{}", entry.file_name().to_string_lossy());
            if !plan.files.contains_key(&relative) {
                return Err(conflict(
                    "Editor generation contains unknown files; no files changed",
                ));
            }
        }
    }
    plan.report.installed = true;
    Ok(())
}

/// Read-only preflight. Receipts are compared to the compiled package, not trusted.
pub fn check(project: &Path) -> io::Result<Installation> {
    let mut plan = plan(project)?;
    verify(&mut plan)?;
    Ok(plan.report)
}

/// Install a new immutable generation, or verify an unchanged existing one.
/// An interrupted write leaves no successful receipt and is refused on retry.
pub fn install(project: &Path) -> io::Result<Installation> {
    let mut plan = plan(project)?;
    verify(&mut plan)?;
    if plan.report.installed {
        return Ok(plan.report);
    }
    for directory in &plan.directories {
        let target = checked_path(&plan.root, directory)?;
        fs::create_dir_all(target.parent().expect("generation parent"))?;
        fs::create_dir(target)?;
    }
    for (relative, content) in &plan.files {
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(checked_path(&plan.root, relative)?)?
            .write_all(content.as_bytes())?;
    }
    let receipt = checked_path(&plan.root, &plan.receipt)?;
    fs::create_dir_all(receipt.parent().expect("receipt parent"))?;
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(receipt)?
        .write_all(&plan.inventory)?;
    plan.report.installed = true;
    Ok(plan.report)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Project(PathBuf);
    impl Project {
        fn new() -> Self {
            let mut random = [0u8; 16];
            getrandom::getrandom(&mut random).unwrap();
            let name: String = random.iter().map(|byte| format!("{byte:02x}")).collect();
            let root = std::env::temp_dir()
                .canonicalize()
                .unwrap()
                .join(format!("legura-install-{name}"));
            fs::create_dir(&root).unwrap();
            fs::write(root.join("Cargo.toml"), "[package]\nname = 'fixture'\n").unwrap();
            Self(root)
        }
    }
    impl Drop for Project {
        fn drop(&mut self) {
            let root = self.0.canonicalize().unwrap();
            assert!(root.starts_with(std::env::temp_dir().canonicalize().unwrap()));
            assert!(root
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("legura-install-"));
            fs::remove_dir_all(root).unwrap();
        }
    }

    #[test]
    fn check_is_read_only_and_install_is_idempotent_without_activation() {
        let project = Project::new();
        fs::write(project.0.join("operator-data"), "untouched").unwrap();
        let report = check(&project.0).unwrap();
        assert!(!report.installed);
        assert_eq!(fs::read_dir(&project.0).unwrap().count(), 2);
        let first = install(&project.0).unwrap();
        assert!(first.installed);
        assert_eq!(first.generation.len(), 64);
        assert_eq!(first.generation, install(&project.0).unwrap().generation);
        assert!(check(&project.0).unwrap().installed);
        assert_eq!(
            fs::read_to_string(project.0.join("operator-data")).unwrap(),
            "untouched"
        );
        assert!(!project.0.join("app/layout.asx").exists());
    }

    #[test]
    fn local_changes_and_unknown_files_are_never_repaired_or_overwritten() {
        let project = Project::new();
        let report = install(&project.0).unwrap();
        let component = project.0.join(&report.component);
        let original = fs::read(&component).unwrap();
        fs::write(&component, "my customization").unwrap();
        assert!(install(&project.0).is_err());
        assert_eq!(fs::read_to_string(&component).unwrap(), "my customization");
        fs::write(&component, original).unwrap();
        let unknown = component.parent().unwrap().join("my-file");
        fs::write(&unknown, "keep").unwrap();
        assert!(check(&project.0).is_err());
        assert!(install(&project.0).is_err());
        assert_eq!(fs::read_to_string(unknown).unwrap(), "keep");
    }

    #[test]
    fn incomplete_generation_or_changed_receipt_refuses_installation() {
        let project = Project::new();
        let plan = plan(&project.0).unwrap();
        fs::create_dir_all(project.0.join(&plan.directories[0])).unwrap();
        assert!(install(&project.0).is_err());
        assert!(!project.0.join(&plan.receipt).exists());
        let other = Project::new();
        install(&other.0).unwrap();
        let receipt = other.0.join(&plan.receipt);
        fs::write(&receipt, "tampered").unwrap();
        assert!(install(&other.0).is_err());
        assert_eq!(fs::read_to_string(receipt).unwrap(), "tampered");
    }

    #[test]
    fn old_generations_and_app_owned_glue_are_left_untouched() {
        let project = Project::new();
        let old = project.0.join("public/content-editor/old-generation");
        fs::create_dir_all(&old).unwrap();
        fs::write(old.join("editor.css"), "custom old css").unwrap();
        let host = project.0.join("public/content-editor/host.js");
        fs::write(&host, "application lifecycle").unwrap();
        install(&project.0).unwrap();
        assert!(check(&project.0).unwrap().installed);
        assert_eq!(
            fs::read_to_string(old.join("editor.css")).unwrap(),
            "custom old css"
        );
        assert_eq!(fs::read_to_string(host).unwrap(), "application lifecycle");
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_ancestors_and_assets_are_rejected_without_touching_target() {
        use std::os::unix::fs::symlink;
        let project = Project::new();
        let outside = Project::new();
        symlink(&outside.0, project.0.join("public")).unwrap();
        assert!(install(&project.0).is_err());
        assert_eq!(fs::read_dir(&outside.0).unwrap().count(), 1);
        fs::remove_file(project.0.join("public")).unwrap();
        let report = install(&project.0).unwrap();
        let component = project.0.join(report.component);
        fs::remove_file(&component).unwrap();
        symlink(outside.0.join("Cargo.toml"), &component).unwrap();
        assert!(check(&project.0).is_err());
        assert_eq!(
            fs::read_to_string(outside.0.join("Cargo.toml")).unwrap(),
            "[package]\nname = 'fixture'\n"
        );
    }
}
