//! Explicit native media configuration; disabled by default, never raw fallback.
use crate::media_library::MediaLibraryStorage;
use crate::media_upload::MEDIA_CAPABILITY;
use crate::storage::StoreId;
use axonyx_runtime::backend::{
    AxDataTransport, AxDatabaseDriver, AxEnv, AxRuntimeError, AxRuntimeResult,
};
use axonyx_runtime::native::{AxNativeError, AxNativeRegistry};
use axonyx_runtime::server::{AxFileRef, AxFileStorage, AxIncomingFile};
use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};
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
    let media = configured_media(env, base)?;
    Ok(Arc::new(ConfiguredStorage { media, fallback }))
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct MediaAvailability {
    pub available: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MediaReadInput {
    id: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PublishedImageInput {
    slug: String,
    id: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PublishedImagePresentationInput {
    slug: String,
}

/// Anonymous admission still requires the current published-post association.
pub fn register_published_reads(
    registry: &mut AxNativeRegistry,
    env: &AxEnv,
    base: &Path,
) -> Result<(), AxNativeError> {
    use axonyx_runtime::native_response::AxNativeResponseError;
    let media = Arc::new(configured_media(env, base).map_err(|_| AxNativeError::Rejected)?);
    let reader = media.clone();
    registry.register_public_response(
        "legura.post.image.read",
        crate::storage::MAX_OBJECT_BYTES as usize,
        move |input: PublishedImageInput| {
            reader
                .as_ref()
                .as_ref()
                .ok_or(AxNativeResponseError::NotFound)?
                .read_published_post_image(&input.slug, &input.id)
                .map_err(|error| match error {
                    crate::published_image::PublishedImageError::NotFound => {
                        AxNativeResponseError::NotFound
                    }
                    crate::published_image::PublishedImageError::Unavailable => {
                        AxNativeResponseError::Unavailable
                    }
                })?
                .into_native_response()
        },
    )?;
    registry.register(
        "legura.post.image.presentation",
        move |input: PublishedImagePresentationInput| {
            Ok::<_, ()>(match media.as_ref() {
                Some(media) => media.published_post_image_presentation(&input.slug),
                None => crate::published_image::ImagePresentation::default(),
            })
        },
    )
}

/// The runtime supplies a verified subject, never an input field or browser role.
pub fn register_private_reads(
    registry: &mut AxNativeRegistry,
    env: &AxEnv,
    base: &Path,
) -> Result<(), AxNativeError> {
    use axonyx_runtime::native_response::AxNativeResponseError;
    let media = configured_media(env, base).map_err(|_| AxNativeError::Rejected)?;
    registry.register_private_response(
        "legura.media.read",
        crate::storage::MAX_OBJECT_BYTES as usize,
        move |context, input: MediaReadInput| {
            let media = media.as_ref().ok_or(AxNativeResponseError::Forbidden)?;
            media
                .read_for_administrator(context.subject(), &input.id)
                .map_err(|error| match error {
                    crate::media_read::MediaReadError::Forbidden => {
                        AxNativeResponseError::Forbidden
                    }
                    crate::media_read::MediaReadError::NotFound => AxNativeResponseError::NotFound,
                    crate::media_read::MediaReadError::Unavailable => {
                        AxNativeResponseError::Unavailable
                    }
                })?
                .into_native_response()
        },
    )
}

/// Server-owned startup snapshot for presentation only, never an upload grant.
/// Invalid configuration aborts registration instead of masquerading as disabled.
pub fn register_availability(
    registry: &mut AxNativeRegistry,
    env: &AxEnv,
    base: &Path,
) -> Result<(), AxNativeError> {
    let available = configured_media(env, base)
        .map_err(|_| AxNativeError::Rejected)?
        .is_some();
    registry.register("legura.media.availability", move |_: ()| {
        Ok::<_, ()>(MediaAvailability { available })
    })
}

/// These internal callbacks are not HTTP handlers. Only guarded .ax actions may
/// supply administrator_id, after session verification and CSRF admission.
pub fn register_post_images(
    registry: &mut AxNativeRegistry,
    env: &AxEnv,
    base: &Path,
) -> Result<(), AxNativeError> {
    use crate::post_images::{
        mutation_result, ImageSelection, PostImageError, RemoveImageInput, SetImageInput,
    };
    let media = Arc::new(configured_media(env, base).map_err(|_| AxNativeError::Rejected)?);
    let setter = media.clone();
    registry.register("legura.post.image.set", move |input: SetImageInput| {
        let result = match setter.as_ref() {
            Some(media) => media.set_post_image_for_administrator(
                &input.administrator_id,
                &input.post_id,
                Some(ImageSelection {
                    media_id: &input.media_id,
                    alt: &input.alt,
                }),
            ),
            None => Err(PostImageError::Forbidden),
        };
        mutation_result(result)
    })?;
    registry.register(
        "legura.post.image.remove",
        move |input: RemoveImageInput| {
            mutation_result(match media.as_ref() {
                Some(media) => media.set_post_image_for_administrator(
                    &input.administrator_id,
                    &input.post_id,
                    None,
                ),
                None => Err(PostImageError::Forbidden),
            })
        },
    )
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DeleteMediaInput {
    administrator_id: String,
    media_id: String,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DeleteMediaResult {
    authorized: bool,
    found: bool,
    removed: bool,
    cleanup_ok: bool,
    errors: std::collections::BTreeMap<String, String>,
}

pub fn register_media_deletion(
    registry: &mut AxNativeRegistry,
    env: &AxEnv,
    base: &Path,
) -> Result<(), AxNativeError> {
    use crate::media_delete::{DeleteError, DeleteOutcome};
    let media = configured_media(env, base).map_err(|_| AxNativeError::Rejected)?;
    registry.register("legura.media.delete", move |input: DeleteMediaInput| {
        let result = match &media {
            Some(media) => media.delete_for_administrator(&input.administrator_id, &input.media_id),
            None => Err(DeleteError::Forbidden),
        };
        let mut output = DeleteMediaResult {
            authorized: true, found: true, removed: false, cleanup_ok: true,
            errors: Default::default(),
        };
        match result {
            Ok(DeleteOutcome::Removed) => output.removed = true,
            Ok(DeleteOutcome::CleanupRequired(_)) => {
                output.removed = true;
                output.cleanup_ok = false;
            }
            Err(DeleteError::Forbidden) => output.authorized = false,
            Err(DeleteError::NotFound) => output.found = false,
            Err(DeleteError::InUse) => {
                output.errors.insert("operation".into(), "This image is used by a post. Remove it from all posts, including drafts, before deleting it.".into());
            }
            Err(DeleteError::Unavailable) => return Err(AxNativeError::Rejected),
        }
        Ok(output)
    })
}

fn configured_media(
    env: &AxEnv,
    base: &Path,
) -> Result<Option<MediaLibraryStorage>, MediaConfigurationError> {
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
    Ok(media)
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
    fn disabled_deletion_refuses_authority_and_extra_input() {
        let mut registry = AxNativeRegistry::default();
        register_media_deletion(&mut registry, &AxEnv::new(), Path::new("unused")).unwrap();
        let input = serde_json::json!({"administrator_id":"owner","media_id":"a".repeat(64)});
        let result = registry.call("legura.media.delete", input.clone()).unwrap();
        assert_eq!(result["authorized"], false);
        assert_eq!(result["removed"], false);
        let mut forged = input;
        forged["path"] = "../private".into();
        assert!(registry.call("legura.media.delete", forged).is_err());
    }

    #[test]
    fn disabled_public_reads_have_no_public_or_json_byte_access() {
        use axonyx_runtime::backend::AxRuntimeEnvAccess;
        use axonyx_runtime::server::AxHttpRequest;
        let mut registry = AxNativeRegistry::default();
        register_published_reads(&mut registry, &AxEnv::new(), Path::new("unused")).unwrap();
        assert_eq!(
            registry
                .call(
                    "legura.post.image.presentation",
                    serde_json::json!({"slug":"draft"})
                )
                .unwrap(),
            serde_json::json!({"selected":false,"src":"","alt":""})
        );
        assert!(registry
            .call(
                "legura.post.image.read",
                serde_json::json!({"slug":"draft","id":"a".repeat(64)})
            )
            .is_err());
        let runtime = axonyx_runtime::backend::AxDatabaseRuntime::new(
            AxEnv::new(),
            axonyx_runtime::backend::MemoryAdapter,
        )
        .with_native_functions(registry);
        let request = AxHttpRequest::new("GET", "/image");
        assert_eq!(
            runtime
                .runtime_native_public_response(
                    &request,
                    "legura.post.image.read",
                    serde_json::json!({"slug":"draft","id":"a".repeat(64)})
                )
                .unwrap()
                .status,
            404
        );
    }

    #[test]
    fn post_image_callbacks_validate_inputs_authority_and_independent_mutations() {
        let fixture = Fixture::new();
        let database = Connection::open(fixture.0.join("cms.db")).unwrap();
        database.execute_batch("INSERT INTO legura_users VALUES('owner','owner@example.test','admin'); INSERT INTO legura_posts(id,title,slug,body,status) VALUES('post','Title','title','Body','draft');").unwrap();
        let upload = configure(&fixture.env(), &fixture.0, Arc::new(Fallback::default())).unwrap();
        let image = upload.save_file(MEDIA_CAPABILITY, &incoming()).unwrap();
        let mut registry = AxNativeRegistry::default();
        register_post_images(&mut registry, &fixture.env(), &fixture.0).unwrap();
        let input = serde_json::json!({"administrator_id":"owner","post_id":"post","media_id":image.id,"alt":"Useful alt"});
        assert_eq!(
            registry
                .call("legura.post.image.set", input.clone())
                .unwrap()["saved"],
            true
        );
        let mut invalid = input.clone();
        invalid["alt"] = "x".repeat(301).into();
        assert!(
            registry.call("legura.post.image.set", invalid).unwrap()["errors"]["alt"].is_string()
        );
        let mut extra = input.clone();
        extra["subject"] = "forged".into();
        assert_eq!(
            registry.call("legura.post.image.set", extra),
            Err(AxNativeError::InvalidInput)
        );
        let mut missing = input.clone();
        missing["post_id"] = "missing".into();
        assert_eq!(
            registry.call("legura.post.image.set", missing).unwrap()["found"],
            false
        );
        database
            .execute("DELETE FROM legura_users WHERE id='owner'", [])
            .unwrap();
        assert_eq!(
            registry
                .call("legura.post.image.set", input.clone())
                .unwrap()["authorized"],
            false
        );
        assert_eq!(
            registry
                .call(
                    "legura.post.image.remove",
                    serde_json::json!({"administrator_id":"owner","post_id":"post"})
                )
                .unwrap()["authorized"],
            false
        );
        database
            .execute(
                "INSERT INTO legura_users VALUES('owner','owner@example.test','admin')",
                [],
            )
            .unwrap();
        registry
            .call(
                "legura.post.image.remove",
                serde_json::json!({"administrator_id":"owner","post_id":"post"}),
            )
            .unwrap();
        assert_eq!(
            database
                .query_row("SELECT count(*) FROM legura_post_images", [], |row| row
                    .get::<_, u64>(0))
                .unwrap(),
            0
        );
        assert_eq!(
            database
                .query_row("SELECT count(*) FROM legura_media", [], |row| row
                    .get::<_, u64>(0))
                .unwrap(),
            1
        );
    }

    #[test]
    fn disabled_post_image_callbacks_never_become_a_write_capability() {
        let mut registry = AxNativeRegistry::default();
        register_post_images(&mut registry, &AxEnv::new(), Path::new("unused")).unwrap();
        for (name, input) in [
            (
                "legura.post.image.set",
                serde_json::json!({"administrator_id":"owner","post_id":"post","media_id":"a".repeat(64),"alt":""}),
            ),
            (
                "legura.post.image.remove",
                serde_json::json!({"administrator_id":"owner","post_id":"post"}),
            ),
        ] {
            let result = registry.call(name, input).unwrap();
            assert_eq!(result["authorized"], false);
            assert_eq!(result["saved"], false);
        }
    }

    #[test]
    fn private_native_reads_recheck_real_sessions_authority_and_disabled_storage() {
        use axonyx_runtime::backend::AxRuntimeEnvAccess;
        use axonyx_runtime::server::AxHttpRequest;
        let fixture = Fixture::new();
        let env = fixture
            .env()
            .with_secret("session_key", "isolated-media-read-session-secret")
            .with_secret("session_cookie_secure", "false");
        let connection = Connection::open(fixture.0.join("cms.db")).unwrap();
        connection
            .execute(
                "INSERT INTO legura_users VALUES ('owner', 'owner@example.test', 'admin')",
                [],
            )
            .unwrap();
        let upload = configure(&env, &fixture.0, Arc::new(Fallback::default())).unwrap();
        let image = upload.save_file(MEDIA_CAPABILITY, &incoming()).unwrap();
        let bytes = fs::read(fixture.0.join("private-media").join(&image.id)).unwrap();
        let mut registry = AxNativeRegistry::default();
        register_private_reads(&mut registry, &env, &fixture.0).unwrap();
        let runtime = runtime_from_env(env.clone())
            .unwrap()
            .with_native_functions(registry);
        let (_, cookie) = runtime
            .runtime_create_session("owner", Default::default())
            .unwrap();
        let request = AxHttpRequest::new("GET", "/api/media/image")
            .with_header("Cookie", format!("{}={}", cookie.name, cookie.value));
        let input = serde_json::json!({"id": image.id});
        let read = || {
            runtime
                .runtime_native_private_response(&request, "legura.media.read", input.clone())
                .unwrap()
        };
        let response = read();
        assert_eq!(response.status, 200);
        assert_eq!(response.body.into_bytes(), bytes);
        assert!(runtime
            .runtime_native_private_response(
                &request,
                "legura.media.read",
                serde_json::json!({"id": image.id, "subject": "owner"})
            )
            .is_err());
        connection
            .execute("DELETE FROM legura_users WHERE id='owner'", [])
            .unwrap();
        assert_eq!(read().status, 403);
        connection
            .execute(
                "INSERT INTO legura_users VALUES ('owner', 'owner@example.test', 'admin')",
                [],
            )
            .unwrap();
        assert_eq!(read().status, 200);
        runtime.runtime_destroy_session(&request).unwrap();
        assert_eq!(read().status, 403);

        let mut disabled = env;
        disabled
            .secret
            .insert("media_driver".into(), "disabled".into());
        disabled.secret.remove("media_root");
        disabled.secret.remove("media_store_id");
        let mut registry = AxNativeRegistry::default();
        register_private_reads(&mut registry, &disabled, Path::new("unused")).unwrap();
        let runtime = runtime_from_env(disabled)
            .unwrap()
            .with_native_functions(registry);
        let (_, cookie) = runtime
            .runtime_create_session("owner", Default::default())
            .unwrap();
        let request = AxHttpRequest::new("GET", "/api/media/image")
            .with_header("Cookie", format!("{}={}", cookie.name, cookie.value));
        assert_eq!(
            runtime
                .runtime_native_private_response(&request, "legura.media.read", input)
                .unwrap()
                .status,
            403
        );
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
    fn availability_is_disabled_without_touching_paths_or_accepting_client_flags() {
        for env in [
            AxEnv::new(),
            AxEnv::new().with_secret("media_driver", "disabled"),
        ] {
            let mut registry = AxNativeRegistry::default();
            register_availability(&mut registry, &env, Path::new("unused")).unwrap();
            assert_eq!(
                registry
                    .call("legura.media.availability", serde_json::Value::Null)
                    .unwrap(),
                serde_json::json!({"available": false})
            );
            for input in [
                serde_json::json!({"available": true}),
                serde_json::json!({"media_root": "secret"}),
                serde_json::json!({}),
            ] {
                assert_eq!(
                    registry.call("legura.media.availability", input),
                    Err(AxNativeError::InvalidInput)
                );
            }
        }
    }

    #[test]
    fn availability_uses_validated_configuration_and_exposes_only_a_boolean() {
        let fixture = Fixture::new();
        let mut registry = AxNativeRegistry::default();
        register_availability(&mut registry, &fixture.env(), &fixture.0).unwrap();
        assert_eq!(
            registry
                .call("legura.media.availability", serde_json::Value::Null)
                .unwrap(),
            serde_json::json!({"available": true})
        );
        assert_eq!(
            fs::read_dir(fixture.0.join("private-media"))
                .unwrap()
                .count(),
            0
        );
        let connection = Connection::open(fixture.0.join("cms.db")).unwrap();
        assert_eq!(
            connection
                .query_row("SELECT COUNT(*) FROM legura_media", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            0
        );
    }

    #[test]
    fn unavailable_or_unmigrated_storage_never_registers_a_positive_snapshot() {
        let fixture = Fixture::new();
        let mut partial = fixture.env();
        partial.secret.remove("media_store_id");
        let old_database = fixture.0.join("old.db");
        Connection::open(&old_database).unwrap();
        for env in [
            partial,
            fixture.env().with_secret("media_driver", "disabled"),
            fixture.env().with_public("media_root", "private"),
            fixture.env().with_secret("media_root", "missing"),
            fixture.env().with_secret("db_url", "sqlite:missing.db"),
            fixture.env().with_secret("db_url", "sqlite:old.db"),
        ] {
            let mut registry = AxNativeRegistry::default();
            assert_eq!(
                register_availability(&mut registry, &env, &fixture.0),
                Err(AxNativeError::Rejected)
            );
            assert_eq!(
                registry.call("legura.media.availability", serde_json::Value::Null),
                Err(AxNativeError::UnknownFunction)
            );
        }
        assert!(!fixture.0.join("missing").exists());
        assert!(!fixture.0.join("missing.db").exists());
        let old = Connection::open(old_database).unwrap();
        assert_eq!(
            old.query_row("SELECT COUNT(*) FROM sqlite_master", [], |row| row
                .get::<_, i64>(0))
                .unwrap(),
            0
        );
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
