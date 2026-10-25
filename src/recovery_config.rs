//! New-only private recovery settings. No ambient env loading or service switch.
use anyhow::{ensure, Context, Result};
use axonyx_runtime::backend::AxEnv;
use axonyx_runtime::server::AxUnavailableFileStorage;
use std::{fs, fs::OpenOptions, io::Write, path::Path, sync::Arc};

#[derive(Clone, Copy)]
pub enum CookieMode {
    Https,
    LocalHttp,
}

fn random_secret() -> Result<String> {
    let mut bytes = [0; 32];
    getrandom::getrandom(&mut bytes).map_err(|_| anyhow::anyhow!("OS randomness unavailable"))?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn dotenv_path(path: &Path) -> Result<&str> {
    let value = path.to_str().context("Configuration paths must be UTF-8")?;
    ensure!(
        value.trim() == value
            && !value
                .chars()
                .any(|c| c.is_control() || c == '\'' || c == '"'),
        "Configuration path cannot be represented safely in the runtime dotenv format"
    );
    Ok(value)
}

pub fn generate(archive: &Path, recovery: &Path, package: &Path, mode: CookieMode) -> Result<()> {
    let package = package.components().collect::<std::path::PathBuf>();
    ensure!(
        fs::symlink_metadata(&package)?.file_type().is_dir(),
        "Package root must be an existing directory, not a symlink"
    );
    let package = package.canonicalize()?;
    ensure!(
        !package
            .components()
            .any(|part| part.as_os_str().to_str().is_some_and(|name| name
                .eq_ignore_ascii_case("public")
                || name.eq_ignore_ascii_case("dist"))),
        "Private configuration cannot be generated inside public/dist directories"
    );
    let destination = package.join(".env");
    match fs::symlink_metadata(&destination) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        _ => anyhow::bail!(
            "Existing configuration is never replaced; choose a new package directory"
        ),
    }
    let (_, store) = crate::backup_archive::check_recovery_with_store(archive, recovery)?;
    let recovery = recovery.canonicalize()?;
    let database = recovery.join("database.sqlite");
    let media = recovery.join("media").join(store.as_str());
    let url = format!("sqlite:{}", dotenv_path(&database)?);
    let media = dotenv_path(&media)?;
    let session = random_secret()?;
    let setup = random_secret()?;
    ensure!(
        session != setup,
        "Independent secrets could not be generated"
    );
    let env = AxEnv::new()
        .with_secret("db_url", &url)
        .with_secret("db_dialect", "sqlite")
        .with_secret("media_driver", "local")
        .with_secret("media_root", media)
        .with_secret("media_store_id", store.as_str());
    let _storage =
        crate::media_config::configure(&env, &package, Arc::new(AxUnavailableFileStorage))?;
    let secure = match mode {
        CookieMode::Https => "true",
        CookieMode::LocalHttp => "false",
    };
    let content = format!("AX_SECRET_DB_URL={url}\nAX_SECRET_DB_DIALECT=sqlite\nAX_SECRET_SESSION_KEY={session}\nAX_SECRET_SETUP_TOKEN={setup}\nAX_SECRET_SESSION_COOKIE_SECURE={secure}\nAX_SECRET_MEDIA_DRIVER=local\nAX_SECRET_MEDIA_ROOT={media}\nAX_SECRET_MEDIA_STORE_ID={}\n", store.as_str());
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(&destination)
        .context("Cannot create NEW private configuration")?;
    let result = file
        .write_all(content.as_bytes())
        .and_then(|()| file.sync_all());
    drop(file);
    if result.is_err() {
        let _ = fs::remove_file(&destination);
    }
    result.context("Could not persist private configuration")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dotenv_paths_cannot_inject_settings_or_depend_on_quote_escaping() {
        for path in [
            "/private/new\nAX_SECRET_MEDIA_DRIVER=disabled",
            "/private/quote\"/file",
            "/private/quote'/file",
            "/private/control\0/file",
        ] {
            assert!(dotenv_path(Path::new(path)).is_err());
        }
        assert_eq!(
            dotenv_path(Path::new("/private/space in name/database.sqlite")).unwrap(),
            "/private/space in name/database.sqlite"
        );
    }
}
