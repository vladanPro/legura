use rusqlite::Connection;
use std::{fs, path::PathBuf, process::Command};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let mut random = [0u8; 16];
        getrandom::getrandom(&mut random).unwrap();
        let suffix: String = random.iter().map(|byte| format!("{byte:02x}")).collect();
        let root = std::env::temp_dir()
            .canonicalize()
            .unwrap()
            .join(format!("legura-content-check-{suffix}"));
        fs::create_dir(&root).unwrap();
        Self(root)
    }
    fn run(&self, command: &str, name: &str) -> std::process::Output {
        Command::new(env!("CARGO_BIN_EXE_legura-maintenance"))
            .arg(command)
            .arg(self.0.join(name))
            .output()
            .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let root = self.0.canonicalize().unwrap();
        assert!(root.starts_with(std::env::temp_dir().canonicalize().unwrap()));
        assert!(root
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("legura-content-check-"));
        fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn cli_reports_counts_sets_failure_status_and_never_repairs_storage() {
    let fixture = Fixture::new();
    assert!(fixture.run("init", "test.db").status.success());
    let empty = fixture.run("content-check", "test.db");
    assert!(empty.status.success());
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&empty.stdout).unwrap()["total"],
        0
    );
    {
        let connection = Connection::open(fixture.0.join("test.db")).unwrap();
        connection.execute_batch("INSERT INTO legura_posts(id,title,slug,body,status) VALUES ('private-id', 'Private title', 'private-slug', 'Private body', 'draft'); INSERT INTO legura_posts(id,title,slug,body,status,content_format,content_document) VALUES ('broken-id', 'Secret title', 'secret-slug', 'Secret body', 'draft', 'legura-richtext', '{secret-bad-json');").unwrap();
    }
    let before = fs::read(fixture.0.join("test.db")).unwrap();
    let rejected = fixture.run("content-check", "test.db");
    assert_eq!(rejected.status.code(), Some(1));
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&rejected.stdout).unwrap(),
        serde_json::json!({"total":2,"plain":1,"rich":0,"invalid":1})
    );
    let output = format!(
        "{}{}",
        String::from_utf8_lossy(&rejected.stdout),
        String::from_utf8_lossy(&rejected.stderr)
    );
    for secret in [
        "private-id",
        "Private title",
        "Private body",
        "Secret body",
        "{secret-bad-json",
    ] {
        assert!(!output.contains(secret));
    }
    assert_eq!(fs::read(fixture.0.join("test.db")).unwrap(), before);
    assert!(!fixture.run("content-check", "absent.db").status.success());
    assert!(!fixture.0.join("absent.db").exists());
}
