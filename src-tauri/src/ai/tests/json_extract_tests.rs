use crate::ai::ollama::{extract_json_array, extract_json_object};

#[test]
fn extracts_plain_object() {
    let input = r#"{"category": "Images", "confidence": 0.8}"#;
    assert_eq!(extract_json_object(input), Some(input));
}

#[test]
fn extracts_object_from_markdown_fence() {
    let input = "```json\n{\"category\": \"Images\", \"tags\": []}\n```";
    let extracted = extract_json_object(input).expect("should find object");
    assert_eq!(extracted, "{\"category\": \"Images\", \"tags\": []}");
}

#[test]
fn extracts_object_from_prose() {
    let input = "Here is the analysis: {\"category\": \"Documents\"} — let me know.";
    assert_eq!(extract_json_object(input), Some("{\"category\": \"Documents\"}"));
}

#[test]
fn extracts_nested_object() {
    let input = r#"prefix {"outer": {"inner": "value"}, "x": 1} suffix"#;
    assert_eq!(
        extract_json_object(input),
        Some(r#"{"outer": {"inner": "value"}, "x": 1}"#)
    );
}

#[test]
fn handles_braces_inside_strings() {
    // The `}` inside the string must not close the outer object.
    let input = r#"{"summary": "this } is { inside", "ok": true}"#;
    assert_eq!(extract_json_object(input), Some(input));
}

#[test]
fn handles_escaped_quotes_inside_strings() {
    let input = r#"{"summary": "he said \"hi\"", "ok": true}"#;
    assert_eq!(extract_json_object(input), Some(input));
}

#[test]
fn returns_none_when_no_object_present() {
    assert_eq!(extract_json_object("just some prose"), None);
    assert_eq!(extract_json_object(""), None);
}

#[test]
fn returns_none_when_object_is_unterminated() {
    // Vision model truncated mid-response — don't claim partial JSON as valid.
    assert_eq!(extract_json_object(r#"{"category": "Images", "tags": ["#), None);
}

#[test]
fn extracts_array_from_prose() {
    let input = "Here you go: [{\"path\": \"a\"}, {\"path\": \"b\"}] done.";
    assert_eq!(
        extract_json_array(input),
        Some("[{\"path\": \"a\"}, {\"path\": \"b\"}]")
    );
}

#[test]
fn extracts_empty_array() {
    let input = "```json\n[]\n```";
    assert_eq!(extract_json_array(input), Some("[]"));
}
