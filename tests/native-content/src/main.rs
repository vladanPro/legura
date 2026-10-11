use axonyx_runtime::native::AxNativeRegistry;
use legura::content_preparation::{prepare_content, ContentInput, ContentPreparation};
use serde_json::json;

include!(concat!(env!("OUT_DIR"), "/module.rs"));

fn compiled_action_proof() {
    use axonyx_runtime::backend::{AxDatabaseRuntime, AxEnv, MemoryAdapter};
    use axonyx_runtime::server::{AxHttpRequest, AxUnavailableFileStorage};
    use axonyx_runtime::{
        execute_preview_action_request_sources_with_runtime_and_storage, AxPreviewStore,
    };
    use legura::content_preparation::{prepare_content_form, ContentFormInput, PreparedColumns};
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };

    let preparations = Arc::new(AtomicUsize::new(0));
    let writes = Arc::new(AtomicUsize::new(0));
    let mut registry = AxNativeRegistry::default();
    let calls = Arc::clone(&preparations);
    registry
        .register("legura.content.prepare", move |input: ContentFormInput| {
            calls.fetch_add(1, Ordering::SeqCst);
            Ok::<_, ()>(prepare_content_form(input))
        })
        .unwrap();
    let calls = Arc::clone(&writes);
    registry
        .register("legura.proof.persist", move |columns: PreparedColumns| {
            calls.fetch_add(1, Ordering::SeqCst);
            Ok::<_, ()>(columns)
        })
        .unwrap();
    let runtime =
        AxDatabaseRuntime::new(AxEnv::default(), MemoryAdapter).with_native_functions(registry);

    let request = AxHttpRequest::new("POST", "/proof")
        .with_body(b"content_format=plain-text&body=Hello".to_vec());
    let output =
        backend::dispatch_action(&runtime, &AxUnavailableFileStorage, "PreparePost", &request)
            .unwrap()
            .unwrap();
    assert_eq!(output.payload["error"]["status"], 403);
    assert_eq!(preparations.load(Ordering::SeqCst), 0);
    assert_eq!(writes.load(Ordering::SeqCst), 0);
    let preview = execute_preview_action_request_sources_with_runtime_and_storage(
        &[include_str!("../actions.ax")],
        "PreparePost",
        &request,
        &runtime,
        &AxUnavailableFileStorage,
        &mut AxPreviewStore::default(),
    )
    .unwrap();
    assert_eq!(preview.error.unwrap().status, 403);
    assert_eq!(preparations.load(Ordering::SeqCst), 0);

    for (form, field) in [
        (
            "content_format=legura-richtext&body=private&content_document=private",
            "content_document",
        ),
        ("content_format=plain-text&body=+", "body"),
        ("content_format=forged&body=private", "content_format"),
    ] {
        let request = AxHttpRequest::new("POST", "/proof")
            .with_header("Authorization", "Bearer isolated-proof")
            .with_body(form.as_bytes().to_vec());
        let output =
            backend::dispatch_action(&runtime, &AxUnavailableFileStorage, "PreparePost", &request)
                .unwrap()
                .unwrap();
        assert_eq!(output.payload["ok"], false);
        assert_eq!(output.payload["error"]["status"], 422);
        assert!(output.payload["error"]["value"]["fields"][field].is_string());
        assert!(!output.payload.to_string().contains("private"));
        assert_eq!(writes.load(Ordering::SeqCst), 0);
        let preview = execute_preview_action_request_sources_with_runtime_and_storage(
            &[include_str!("../actions.ax")],
            "PreparePost",
            &request,
            &runtime,
            &AxUnavailableFileStorage,
            &mut AxPreviewStore::default(),
        )
        .unwrap();
        let error = preview.error.unwrap();
        assert_eq!(error.status, 422);
        assert!(format!("{:?}", error.value).contains(field));
        assert!(!format!("{:?}", error.value).contains("private"));
        assert_eq!(writes.load(Ordering::SeqCst), 0);
    }
    let request = AxHttpRequest::new("POST", "/proof")
        .with_header("Authorization", "Bearer isolated-proof")
        .with_body(b"content_format=plain-text&body=Hello".to_vec());
    let output =
        backend::dispatch_action(&runtime, &AxUnavailableFileStorage, "PreparePost", &request)
            .unwrap()
            .unwrap();
    assert_eq!(output.payload["ok"], true);
    assert_eq!(output.payload["value"]["body"], "Hello");
    assert_eq!(writes.load(Ordering::SeqCst), 1);
    let preview = execute_preview_action_request_sources_with_runtime_and_storage(
        &[include_str!("../actions.ax")],
        "PreparePost",
        &request,
        &runtime,
        &AxUnavailableFileStorage,
        &mut AxPreviewStore::default(),
    )
    .unwrap();
    assert!(preview.error.is_none());
    assert_eq!(writes.load(Ordering::SeqCst), 2);

    let document = json!({"format":"legura-richtext","version":1,"blocks":[
        {"type":"paragraph","children":[{"text":"Rich text","marks":["bold"]}]}
    ]})
    .to_string();
    let encoded = document
        .bytes()
        .map(|byte| format!("%{byte:02X}"))
        .collect::<String>();
    let request = AxHttpRequest::new("POST", "/proof")
        .with_header("Authorization", "Bearer isolated-proof")
        .with_body(
            format!("content_format=legura-richtext&body=forged&content_document={encoded}")
                .into_bytes(),
        );
    let output =
        backend::dispatch_action(&runtime, &AxUnavailableFileStorage, "PreparePost", &request)
            .unwrap()
            .unwrap();
    assert_eq!(output.payload["ok"], true);
    assert_eq!(output.payload["value"]["body"], "Rich text");
    assert_eq!(writes.load(Ordering::SeqCst), 3);
    let columns: PreparedColumns = serde_json::from_value(output.payload["value"].clone()).unwrap();
    assert!(legura::content::PostContent::from_stored(
        &columns.content_format,
        &columns.body,
        columns.content_document.as_deref()
    )
    .unwrap()
    .render_html()
    .contains("<strong>Rich text</strong>"));
}

fn main() {
    compiled_action_proof();
    let mut registry = AxNativeRegistry::default();
    registry
        .register("legura.content.prepare", |input: ContentInput| {
            Ok::<_, ()>(prepare_content(input))
        })
        .unwrap();
    let prepared = registry
        .call(
            "legura.content.prepare",
            json!({
                "content_format":"plain-text", "body":"<script>literal</script>"
            }),
        )
        .unwrap();
    let prepared: ContentPreparation = serde_json::from_value(prepared).unwrap();
    assert!(prepared.errors.is_empty());
    assert_eq!(prepared.columns.unwrap().body, "<script>literal</script>");

    let document = json!({"format":"legura-richtext","version":1,"blocks":[
        {"type":"paragraph","children":[{"text":"Actual text","marks":["bold"]}]}
    ]})
    .to_string();
    let rich = registry.call("legura.content.prepare", json!({
        "content_format":"legura-richtext", "body":"forged projection", "content_document":document
    })).unwrap();
    let rich: ContentPreparation = serde_json::from_value(rich).unwrap();
    assert!(rich.errors.is_empty());
    let columns = rich.columns.unwrap();
    assert_eq!(columns.body, "Actual text");
    let reloaded = legura::content::PostContent::from_stored(
        &columns.content_format,
        &columns.body,
        columns.content_document.as_deref(),
    )
    .unwrap();
    assert!(reloaded
        .render_html()
        .contains("<strong>Actual text</strong>"));

    let invalid = registry
        .call(
            "legura.content.prepare",
            json!({
                "content_format":"legura-richtext", "body":"private", "content_document":"private"
            }),
        )
        .unwrap();
    let invalid: ContentPreparation = serde_json::from_value(invalid).unwrap();
    assert!(invalid.columns.is_none());
    assert!(invalid.errors.contains_key("content_document"));
    assert!(!invalid.errors["content_document"].contains("private"));

    let long = registry
        .call(
            "legura.content.prepare",
            json!({
                "content_format":"plain-text", "body":"a".repeat(20_001)
            }),
        )
        .unwrap();
    let long: ContentPreparation = serde_json::from_value(long).unwrap();
    assert!(long.columns.is_none());
    assert!(long.errors.contains_key("body"));

    for input in [
        json!({"content_format":"plain-text", "body":42}),
        json!({"content_format":"plain-text", "body":"Text", "html":"untrusted"}),
    ] {
        assert!(registry.call("legura.content.prepare", input).is_err());
    }
    println!("Legura native content proof passed: typed preparation, compiled/preview action guards and recoverable field errors.");
}
