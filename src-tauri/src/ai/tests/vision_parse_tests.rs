use crate::ai::ollama::parse_vision_response;

#[test]
fn parses_well_formed_json() {
    let response = r#"{
        "category": "Images",
        "tags": ["cat", "indoor"],
        "summary": "A cat on a sofa",
        "confidence": 0.92,
        "detected_objects": ["cat", "sofa"],
        "scene_type": "indoor",
        "colors": ["orange", "brown"],
        "text_detected": ""
    }"#;
    let (category, confidence, analysis) = parse_vision_response(response);
    assert_eq!(category, "Images");
    assert!((confidence - 0.92).abs() < 1e-6);
    assert_eq!(analysis.summary, "A cat on a sofa");
    assert_eq!(analysis.tags, vec!["cat", "indoor"]);
    assert_eq!(analysis.detected_objects, vec!["cat", "sofa"]);
}

#[test]
fn parses_json_wrapped_in_markdown_fence() {
    let response = "```json\n{\"category\": \"Documents\", \"summary\": \"a receipt\", \"confidence\": 0.7}\n```";
    let (category, confidence, analysis) = parse_vision_response(response);
    assert_eq!(category, "Documents");
    assert!((confidence - 0.7).abs() < 1e-6);
    assert_eq!(analysis.summary, "a receipt");
}

#[test]
fn parses_json_with_prose_prefix_and_suffix() {
    let response = "Here's the analysis: {\"category\": \"Images\", \"summary\": \"chart\", \"confidence\": 0.5} — let me know if you need more.";
    let (category, _, analysis) = parse_vision_response(response);
    assert_eq!(category, "Images");
    assert_eq!(analysis.summary, "chart");
}

#[test]
fn tolerates_missing_fields_via_serde_default() {
    // Only `summary` provided. Everything else should fall back to defaults
    // (empty string / empty vec / 0.0) and the helper should still normalize.
    let response = r#"{"summary": "minimal output"}"#;
    let (category, confidence, analysis) = parse_vision_response(response);
    // Empty category gets backfilled to "Images" by the helper.
    assert_eq!(category, "Images");
    // No confidence -> default 0.0 (after clamp).
    assert!((confidence - 0.0).abs() < 1e-6);
    assert_eq!(analysis.summary, "minimal output");
    assert!(analysis.tags.is_empty());
}

#[test]
fn synthesizes_fallback_when_response_is_pure_prose() {
    // Vision model got confused and returned no JSON. The helper must not
    // crash — it should fabricate a low-confidence Images result and stash
    // the raw text as the summary so the file is still classified.
    let response = "I see a picture but I cannot describe it precisely.";
    let (category, confidence, analysis) = parse_vision_response(response);
    assert_eq!(category, "Images");
    assert!((confidence - 0.4).abs() < 1e-6);
    assert!(
        analysis.summary.contains("picture"),
        "raw text should be preserved as summary; got {:?}",
        analysis.summary
    );
    assert_eq!(analysis.tags, vec!["image".to_string()]);
}

#[test]
fn synthesizes_fallback_when_response_is_empty() {
    let (category, confidence, _) = parse_vision_response("");
    assert_eq!(category, "Images");
    assert!((confidence - 0.4).abs() < 1e-6);
}

#[test]
fn clamps_confidence_above_one() {
    // Some models emit confidences like 95 (thinking 0–100). The dispatcher
    // contract says 0..=1.0; the helper must clamp.
    let response = r#"{"category": "Images", "summary": "x", "confidence": 95.0}"#;
    let (_, confidence, _) = parse_vision_response(response);
    assert!((confidence - 1.0).abs() < 1e-6);
}

#[test]
fn clamps_negative_confidence_to_zero() {
    let response = r#"{"category": "Images", "summary": "x", "confidence": -0.5}"#;
    let (_, confidence, _) = parse_vision_response(response);
    assert!((confidence - 0.0).abs() < 1e-6);
}

#[test]
fn empty_category_field_is_replaced_with_images() {
    let response = r#"{"category": "", "summary": "blank category", "confidence": 0.5}"#;
    let (category, _, _) = parse_vision_response(response);
    assert_eq!(category, "Images");
}

#[test]
fn truncates_very_long_pure_prose_summaries() {
    // A misbehaving model could emit 100KB of prose. The synthesized fallback
    // should cap the summary at 500 chars so we don't blow up the DB row.
    let response = "a ".repeat(10_000);
    let (_, _, analysis) = parse_vision_response(&response);
    assert!(
        analysis.summary.chars().count() <= 500,
        "summary should be capped at 500 chars, got {}",
        analysis.summary.chars().count()
    );
}

#[test]
fn accepts_field_aliases() {
    // Some models use "description" instead of "summary", or "text" instead
    // of "text_detected". The serde aliases cover the common variants.
    let response = r#"{
        "category": "Images",
        "description": "a stop sign",
        "text": "STOP",
        "confidence": 0.8
    }"#;
    let (category, _, analysis) = parse_vision_response(response);
    assert_eq!(category, "Images");
    assert_eq!(analysis.summary, "a stop sign");
    assert_eq!(analysis.text_detected, "STOP");
}

#[test]
fn handles_nested_object_in_response() {
    // The JSON extractor must walk into nested braces without prematurely
    // closing the outer object.
    let response = r#"{
        "category": "Images",
        "summary": "diagram",
        "confidence": 0.7,
        "metadata_unused": {"inner": {"deeper": 1}}
    }"#;
    let (category, confidence, analysis) = parse_vision_response(response);
    assert_eq!(category, "Images");
    assert!((confidence - 0.7).abs() < 1e-6);
    assert_eq!(analysis.summary, "diagram");
}
