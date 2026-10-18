//! Explicit native media configuration; disabled by default, never raw fallback.
use crate::media_library::MediaLibraryStorage;
use crate::media_upload::MEDIA_CAPABILITY;
use crate::storage::StoreId;
use axonyx_runtime::backend::{
    AxDataTransport, AxDatabaseDriver, AxEnv, AxRuntimeError, AxRuntimeResult,
};
use axonyx_runtime::server::{AxFileRef, AxFileStorage, AxIncomingFile};
use rusqlite::{Connection, OpenFlags};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

#[derive(Debug)]
pub struct MediaConfigurationError;
impl std::fmt::Display for MediaConfigurationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Media configuration is invalid or unavailable")
    }
}
impl std::error::Error for MediaConfigurationError {}

struct ConfiguredStorage {
    media: Option<MediaLibraryStorage>,
    fallback: Arc<dyn AxFileStorage>,
}
impl AxFileStorage for ConfiguredStorage {
    fn save_file(&self, capability: &str, file: &AxIncomingFile) -> AxRuntimeResult<AxFileRef> {
        if capability != MEDIA_CAPABILITY {
            return self.fallback.save_file(capability, file);
        }
        match &self.media {
            Some(media) => media.save_file(capability, file),
            None => Err(AxRuntimeError::Forbidden),
        }
    }
}

fn configured_path(
    base: &Path,
    value: &str,
    directory: bool,
) -> Result<PathBuf, MediaConfigurationError> {
    if value.is_empty() || value.trim() != value {
        return Err(MediaConfigurationError);
    }
    let input = Path::new(value);
    let path = if input.is_absolute() {
        input.to_owned()
    } else {
        base.join(input)
    };
    let path = path.components().collect::<PathBuf>();
    let kind = fs::symlink_metadata(&path)
        .map_err(|_| MediaConfigurationError)?
        .file_type();
    if (directory && !kind.is_dir()) || (!directory && !kind.is_file()) {
        return Err(MediaConfigurationError);
    }
    let path = path.canonicalize().map_err(|_| MediaConfigurationError)?;
    for name in ["public", "dist"] {
        let web = base.join(name);
        let web = web.canonicalize().unwrap_or(web);
        if path.starts_with(web) {
            return Err(MediaConfigurationError);
        }
    }
    if directory && base.starts_with(&path) {
        return Err(MediaConfigurationError);
    }
    Ok(path)
}

/// Uses the same normalized AxEnv/database contract as the application. It never
/// creates paths, migrates schemas, serves images or grants request authority.
pub fn configure(
    env: &AxEnv,
    base: &Path,
    fallback: Arc<dyn AxFileStorage>,
) -> Result<Arc<dyn AxFileStorage>, MediaConfigurationError> {
    const KEYS: [&str; 3] = ["media_driver", "media_root", "media_store_id"];
    if env
        .secret
        .keys()
        .any(|key| key.starts_with("media_") && !KEYS.contains(&key.as_str()))
        || env.public.keys().any(|key| key.starts_with("media_"))
    {
        return Err(MediaConfigurationError);
    }
    let driver = env.secret.get("media_driver").map(String::as_str);
    let media = match driver {
        None | Some("disabled") => {
            if env.secret.contains_key("media_root") || env.secret.contains_key("media_store_id") {
                return Err(MediaConfigurationError);
            }
            None
        }
        Some("local") => {
            let base = base.canonicalize().map_err(|_| MediaConfigurationError)?;
            let root = env
                .secret
                .get("media_root")
                .ok_or(MediaConfigurationError)?;
            let store = StoreId::parse(
                env.secret
                    .get("media_store_id")
                    .ok_or(MediaConfigurationError)?,
            )
            .map_err(|_| MediaConfigurationError)?;
            let database = env.database_config().map_err(|_| MediaConfigurationError)?;
            if database.driver != AxDatabaseDriver::Sqlite
                || database.transport != AxDataTransport::Direct
            {
                return Err(MediaConfigurationError);
            }
            let url = database.url.as_deref().ok_or(MediaConfigurationError)?;
            let path = url
                .strip_prefix("sqlite://")
                .or_else(|| url.strip_prefix("sqlite:"))
                .ok_or(MediaConfigurationError)?;
            // Windows canonical paths may use the verbatim prefix `\\?\`;
            // its question mark is not a SQLite URI query option.
            let path_body = path
                .strip_prefix(r"\\?\")
                .or_else(|| path.strip_prefix("//?/"))
                .unwrap_or(path);
            if path == ":memory:"
                || path.starts_with("file:")
                || path_body.contains(['?', '#', '\0'])
            {
                return Err(MediaConfigurationError);
            }
            let root = configured_path(&base, root, true)?;
            let path = configured_path(&base, path, false)?;
            let connection = Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_ONLY)
                .map_err(|_| MediaConfigurationError)?;
            connection
                .execute_batch("PRAGMA trusted_schema=OFF; PRAGMA query_only=ON;")
                .map_err(|_| MediaConfigurationError)?;
            crate::database_contract::validate_current(&connection)
                .map_err(|_| MediaConfigurationError)?;
            let different_store: bool = connection
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM legura_media WHERE store_id != ?)",
                    [store.as_str()],
                    |row| row.get(0),
                )
                .map_err(|_| MediaConfigurationError)?;
            if different_store {
                return Err(MediaConfigurationError);
            }
            Some(
                MediaLibraryStorage::open(&root, store, &path)
                    .map_err(|_| MediaConfigurationError)?,
            )
        }
        _ => return Err(MediaConfigurationError),
    };
    Ok(Arc::new(ConfiguredStorage { media, fallback }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axonyx_runtime::backend::{runtime_from_env, AxMigrationExecutor};
    use image::ImageEncoder;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Default)]
    struct Fallback(AtomicUsize);
    impl AxFileStorage for Fallback {
        fn save_file(&self, _: &str, _: &AxIncomingFile) -> AxRuntimeResult<AxFileRef> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Err(AxRuntimeError::Forbidden)
        }
    }
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let mut random = [0; 16];
            getrandom::getrandom(&mut random).unwrap();
            let root = std::env::temp_dir().canonicalize().unwrap().join(format!(
                "legura-media-config-{:032x}",
                u128::from_le_bytes(random)
            ));
            fs::create_dir(&root).unwrap();
            fs::create_dir(root.join("private-media")).unwrap();
            let fixture = Self(root);
            let runtime = runtime_from_env(fixture.env()).unwrap();
            AxMigrationExecutor::apply_migrations(
                &runtime,
                &crate::database_contract::embedded_migrations(),
            )
            .unwrap();
            fixture
        }
        fn env(&self) -> AxEnv {
            AxEnv::new()
                .with_secret("db_dialect", "sqlite")
                .with_secret(
                    "db_url",
                    format!("sqlite:{}", self.0.join("cms.db").to_str().unwrap()),
                )
                .with_secret("media_driver", "local")
                .with_secret("media_root", "private-media")
                .with_secret("media_store_id", "local-media")
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let root = self.0.canonicalize().unwrap();
            assert_eq!(
                root.parent().unwrap(),
                std::env::temp_dir().canonicalize().unwrap()
            );
            assert!(root
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("legura-media-config-"));
            fs::remove_dir_all(root).unwrap();
        }
    }
    fn incoming() -> AxIncomingFile {
        let mut bytes = Vec::new();
        image::codecs::png::PngEncoder::new(&mut bytes)
            .write_image(&[1, 2, 3, 255], 1, 1, image::ExtendedColorType::Rgba8)
            .unwrap();
        AxIncomingFile {
            field_name: "image".into(),
            file_name: "ignored.svg".into(),
            content_type: None,
            bytes,
        }
    }

    #[test]
    fn disabled_media_never_delegates_to_raw_storage_but_other_capabilities_do() {
        for env in [
            AxEnv::new(),
            AxEnv::new().with_secret("media_driver", "disabled"),
        ] {
            let fallback = Arc::new(Fallback::default());
            let configured = configure(&env, Path::new("unused"), fallback.clone()).unwrap();
            assert!(configured.save_file(MEDIA_CAPABILITY, &incoming()).is_err());
            assert_eq!(fallback.0.load(Ordering::SeqCst), 0);
            assert!(configured.save_file("other", &incoming()).is_err());
            assert_eq!(fallback.0.load(Ordering::SeqCst), 1);
        }
    }

    #[test]
    fn configured_upload_uses_the_application_database_and_keeps_fallback_scoped() {
        let fixture = Fixture::new();
        let fallback = Arc::new(Fallback::default());
        let configured = configure(&fixture.env(), &fixture.0, fallback.clone()).unwrap();
        let saved = configured.save_file(MEDIA_CAPABILITY, &incoming()).unwrap();
        assert_eq!(saved.storage, "local-media");
        assert!(fixture.0.join("private-media").join(&saved.id).is_file());
        let connection = Connection::open(fixture.0.join("cms.db")).unwrap();
        let id: String = connection
            .query_row("SELECT id FROM legura_media", [], |row| row.get(0))
            .unwrap();
        assert_eq!(id, saved.id);
        assert_eq!(fallback.0.load(Ordering::SeqCst), 0);
        configured.save_file("other", &incoming()).unwrap_err();
        assert_eq!(fallback.0.load(Ordering::SeqCst), 1);
        assert!(configure(
            &fixture.env().with_secret("media_store_id", "another-store"),
            &fixture.0,
            fallback
        )
        .is_err());
    }

    #[test]
    fn invalid_partial_public_or_unsupported_configuration_never_falls_back() {
        let fixture = Fixture::new();
        let fallback = Arc::new(Fallback::default());
        let mut missing = fixture.env();
        missing.secret.remove("media_store_id");
        let cases = [
            missing,
            fixture.env().with_secret("media_driver", "disabled"),
            fixture.env().with_secret("media_driver", "s3"),
            fixture.env().with_secret("media_store_id", "../escape"),
            fixture.env().with_secret("media_db", "must-not-create.db"),
            fixture.env().with_public("media_root", "not-public"),
            fixture.env().with_secret("media_root", "missing"),
            fixture.env().with_secret("db_url", "sqlite:missing.db"),
            fixture.env().with_secret("db_url", "sqlite::memory:"),
            fixture
                .env()
                .with_secret("db_url", "sqlite:cms.db?mode=memory"),
            fixture.env().with_secret("db_dialect", "postgres"),
            fixture.env().with_secret("db_transport", "api"),
        ];
        for env in cases {
            let error = configure(&env, &fixture.0, fallback.clone())
                .err()
                .expect("configuration must fail");
            assert_eq!(
                error.to_string(),
                "Media configuration is invalid or unavailable"
            );
        }
        assert_eq!(fallback.0.load(Ordering::SeqCst), 0);
        assert!(!fixture.0.join("missing").exists());
        assert!(!fixture.0.join("missing.db").exists());
        assert_eq!(
            fs::read_dir(fixture.0.join("private-media"))
                .unwrap()
                .count(),
            0
        );
    }

    #[test]
    fn public_dist_and_project_roots_are_not_private_storage() {
        let fixture = Fixture::new();
        for name in ["public", "dist"] {
            fs::create_dir(fixture.0.join(name)).unwrap();
            fs::create_dir(fixture.0.join(name).join("media")).unwrap();
            assert!(configure(
                &fixture
                    .env()
                    .with_secret("media_root", format!("{name}/media")),
                &fixture.0,
                Arc::new(Fallback::default())
            )
            .is_err());
        }
        assert!(configure(
            &fixture.env().with_secret("media_root", "."),
            &fixture.0,
            Arc::new(Fallback::default())
        )
        .is_err());
        let database = Connection::open(fixture.0.join("cms.db")).unwrap();
        database
            .execute("UPDATE _axonyx_migrations SET checksum='wrong'", [])
            .unwrap();
        assert!(configure(&fixture.env(), &fixture.0, Arc::new(Fallback::default())).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn final_symlinks_and_symlinked_web_roots_are_refused() {
        use std::os::unix::fs::symlink;
        let fixture = Fixture::new();
        symlink(fixture.0.join("private-media"), fixture.0.join("linked")).unwrap();
        assert!(configure(
            &fixture.env().with_secret("media_root", "linked/"),
            &fixture.0,
            Arc::new(Fallback::default())
        )
        .is_err());
        symlink(fixture.0.join("cms.db"), fixture.0.join("linked.db")).unwrap();
        assert!(configure(
            &fixture.env().with_secret("db_url", "sqlite:linked.db"),
            &fixture.0,
            Arc::new(Fallback::default())
        )
        .is_err());
        symlink(fixture.0.join("private-media"), fixture.0.join("public")).unwrap();
        assert!(configure(&fixture.env(), &fixture.0, Arc::new(Fallback::default())).is_err());
    }
}
