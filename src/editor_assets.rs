//! Export a package-owned bundle to a new directory; never update operator files.
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Component, Path};

#[derive(Debug, Serialize)]
pub struct BundleManifest {
    pub format_version: u32,
    pub package: &'static str,
    pub package_version: &'static str,
    pub assets: Vec<AssetEntry>,
}

#[derive(Debug, Serialize)]
pub struct AssetEntry {
    pub path: &'static str,
    pub bytes: usize,
    pub sha256: String,
}

fn portable_path(path: &str) -> bool {
    !path.is_empty()
        && !path.contains(['\\', ':'])
        && path
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
        && Path::new(path)
            .components()
            .all(|part| matches!(part, Component::Normal(_)))
}

pub fn export_bundle(destination: &Path) -> io::Result<BundleManifest> {
    let assets = legura_editor::assets::assets();
    let mut paths = BTreeSet::new();
    if legura_editor::assets::BUNDLE_VERSION != 1
        || assets.iter().any(|asset| {
            !portable_path(asset.path)
                || !(asset.path.starts_with("components/") || asset.path.starts_with("public/"))
                || !paths.insert(asset.path.to_ascii_lowercase())
        })
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Unsupported editor bundle layout",
        ));
    }
    #[cfg(unix)]
    let mut directory = fs::DirBuilder::new();
    #[cfg(not(unix))]
    let directory = fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        directory.mode(0o700);
    }
    directory.create(destination)?;
    let mut entries = Vec::new();
    for asset in assets {
        let target = destination.join(asset.path);
        fs::create_dir_all(target.parent().expect("bundle asset parent"))?;
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(target)?
            .write_all(asset.content.as_bytes())?;
        entries.push(AssetEntry {
            path: asset.path,
            bytes: asset.content.len(),
            sha256: format!("{:x}", Sha256::digest(asset.content.as_bytes())),
        });
    }
    let manifest = BundleManifest {
        format_version: legura_editor::assets::BUNDLE_VERSION,
        package: "legura-editor",
        package_version: legura_editor::assets::PACKAGE_VERSION,
        assets: entries,
    };
    // Written last: failed exports remain incomplete and are never called installed.
    let bytes = serde_json::to_vec_pretty(&manifest).map_err(io::Error::other)?;
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination.join("bundle.json"))?
        .write_all(&bytes)?;
    Ok(manifest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_is_complete_inventory_matches_and_replay_never_clobbers() {
        let temp = std::env::temp_dir().canonicalize().unwrap();
        let mut random = [0u8; 16];
        getrandom::getrandom(&mut random).unwrap();
        let name = random
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let root = temp.join(format!("legura-assets-{name}"));
        let manifest = export_bundle(&root).unwrap();
        assert_eq!(manifest.assets.len(), 5);
        for (entry, asset) in manifest.assets.iter().zip(legura_editor::assets::assets()) {
            let bytes = fs::read(root.join(entry.path)).unwrap();
            assert_eq!(bytes, asset.content.as_bytes());
            assert_eq!(bytes.len(), entry.bytes);
            assert_eq!(format!("{:x}", Sha256::digest(&bytes)), entry.sha256);
        }
        let inventory = fs::read(root.join("bundle.json")).unwrap();
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&inventory).unwrap()["format_version"],
            1
        );
        fs::write(root.join("operator-file"), "keep").unwrap();
        assert_eq!(
            export_bundle(&root).unwrap_err().kind(),
            io::ErrorKind::AlreadyExists
        );
        assert_eq!(fs::read(root.join("bundle.json")).unwrap(), inventory);
        assert_eq!(
            fs::read_to_string(root.join("operator-file")).unwrap(),
            "keep"
        );
        assert!(root.canonicalize().unwrap().starts_with(&temp));
        assert!(root
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("legura-assets-"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn traversal_and_platform_paths_are_not_bundle_entries() {
        for path in [
            "",
            "/etc/passwd",
            "../secret",
            "public/../secret",
            "C:/secret",
            "public\\editor.css",
            "public//editor.css",
            "./editor.css",
        ] {
            assert!(!portable_path(path), "{path}");
        }
        assert!(portable_path("public/editor.css"));
    }
}
