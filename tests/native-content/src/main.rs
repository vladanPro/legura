use axonyx_runtime::native::AxNativeRegistry;
use legura::content_preparation::{prepare_content, ContentInput, ContentPreparation};
use serde_json::json;

fn main() {
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
    println!("Legura native content proof passed: typed preparation and recoverable field errors.");
}
