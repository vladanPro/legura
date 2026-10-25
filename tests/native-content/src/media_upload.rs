use axonyx_runtime::backend::{AxDatabaseRuntime, AxEnv, AxRuntimeError, MemoryAdapter};
use axonyx_runtime::server::{AxHttpRequest, AxIncomingFile, AxMultipartForm};
use axonyx_runtime::{
    execute_preview_action_request_sources_with_runtime_and_storage, AxPreviewStore,
};
use image::ImageEncoder;
use legura::media_upload::MediaUploadStorage;
use legura::storage::{
    LocalStorage, ObjectKey, ObjectRef, ObjectStorage, StoreId, MAX_OBJECT_BYTES,
};
use std::collections::BTreeMap;

struct Fixture(std::path::PathBuf);
impl Fixture {
    fn new() -> Self {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "legura-upload-proof-{}-{nonce}",
            std::process::id()
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn count(&self) -> usize {
        std::fs::read_dir(&self.0).unwrap().count()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn request(bytes: Vec<u8>, authorization: Option<&str>) -> AxHttpRequest {
    let mut request = AxHttpRequest::new("POST", "/isolated-upload-proof");
    request.multipart = Some(AxMultipartForm {
        fields: BTreeMap::new(),
        files: BTreeMap::from([(
            "image".into(),
            vec![AxIncomingFile {
                field_name: "image".into(),
                file_name: "../../do-not-trust.svg".into(),
                content_type: Some("text/html".into()),
                bytes,
            }],
        )]),
    });
    if let Some(value) = authorization {
        request = request.with_header("Authorization", value);
    }
    request
}

pub fn compiled_upload_proof() {
    let fixture = Fixture::new();
    let open = || {
        LocalStorage::open(
            &fixture.0,
            StoreId::parse("local-images").unwrap(),
            MAX_OBJECT_BYTES,
        )
        .unwrap()
    };
    let storage = MediaUploadStorage::new(open());
    let runtime = AxDatabaseRuntime::new(AxEnv::default(), MemoryAdapter);
    let mut bytes = Vec::new();
    image::codecs::png::PngEncoder::new(&mut bytes)
        .write_image(&[70, 120, 200, 255], 1, 1, image::ExtendedColorType::Rgba8)
        .unwrap();
    bytes.extend_from_slice(b"do-not-persist-this-trailer");

    for authorization in [None, Some("Bearer forged")] {
        let request = request(bytes.clone(), authorization);
        let compiled = super::backend::dispatch_action(&runtime, &storage, "UploadMedia", &request)
            .unwrap()
            .unwrap();
        assert_eq!(compiled.payload["error"]["status"], 403);
        let preview = execute_preview_action_request_sources_with_runtime_and_storage(
            &[include_str!("../actions.ax")],
            "UploadMedia",
            &request,
            &runtime,
            &storage,
            &mut AxPreviewStore::default(),
        )
        .unwrap();
        assert_eq!(preview.error.unwrap().status, 403);
        assert_eq!(fixture.count(), 0);
    }

    let invalid = request(
        b"<svg onload='alert(1)'/>".to_vec(),
        Some("Bearer isolated-proof"),
    );
    assert!(matches!(
        super::backend::dispatch_action(&runtime, &storage, "UploadMedia", &invalid),
        Err(AxRuntimeError::InvalidInput { .. })
    ));
    assert!(
        execute_preview_action_request_sources_with_runtime_and_storage(
            &[include_str!("../actions.ax")],
            "UploadMedia",
            &invalid,
            &runtime,
            &storage,
            &mut AxPreviewStore::default(),
        )
        .is_err()
    );
    assert_eq!(fixture.count(), 0);

    let valid = request(bytes.clone(), Some("Bearer isolated-proof"));
    let output = super::backend::dispatch_action(&runtime, &storage, "UploadMedia", &valid)
        .unwrap()
        .unwrap();
    assert_eq!(output.payload["ok"], true);
    let value = &output.payload["value"];
    let reference = ObjectRef {
        store: StoreId::parse(value["storage"].as_str().unwrap()).unwrap(),
        key: ObjectKey::parse(value["id"].as_str().unwrap()).unwrap(),
    };
    let saved = open().get(&reference).unwrap();
    assert_eq!(value["storage"], "local-images");
    assert_eq!(
        value["file_name"],
        format!("{}.png", reference.key.as_str())
    );
    assert_eq!(value["content_type"], "image/png");
    assert_eq!(value["size"], saved.len() as u64);
    assert_ne!(saved, bytes);
    assert!(!saved
        .windows(27)
        .any(|part| part == b"do-not-persist-this-trailer"));
    assert_eq!(
        image::load_from_memory(&saved)
            .unwrap()
            .to_rgba8()
            .get_pixel(0, 0)
            .0,
        [70, 120, 200, 255]
    );
    assert_eq!(fixture.count(), 1);

    let preview = execute_preview_action_request_sources_with_runtime_and_storage(
        &[include_str!("../actions.ax")],
        "UploadMedia",
        &valid,
        &runtime,
        &storage,
        &mut AxPreviewStore::default(),
    )
    .unwrap();
    assert!(preview.error.is_none());
    assert_eq!(fixture.count(), 2);
    println!("Compiled/preview media adapter proof passed: guarded writes, invalid-image refusal, canonical bytes and reopen persistence (no HTTP/CSRF or CMS activation claimed).");
}
