use crate::ai::{AiService, FileAnalysis};
use crate::config::Config;
use std::path::PathBuf;
use tempfile::TempDir;
use tokio::io::AsyncWriteExt;

/// Build an AiService that's pinned to the Fallback provider. Lets us test the
/// dispatcher's branch selection without needing Ollama running.
async fn fallback_service() -> AiService {
    let config = Config {
        ai_provider: "fallback".to_string(),
        ollama_host: "".to_string(),
        ..Default::default()
    };
    AiService::new(&config).await.expect("fallback init")
}

/// Write a file with the given bytes to a tempdir and return the absolute path.
async fn write_temp_file(dir: &TempDir, name: &str, bytes: &[u8]) -> PathBuf {
    let path = dir.path().join(name);
    let mut f = tokio::fs::File::create(&path).await.expect("create");
    f.write_all(bytes).await.expect("write");
    f.flush().await.ok();
    path
}

fn assert_path_set(a: &FileAnalysis, expected: &str) {
    assert_eq!(
        a.path, expected,
        "dispatcher must echo path into the analysis result"
    );
}

#[tokio::test]
async fn dispatcher_image_branch_returns_image_analysis() {
    let svc = fallback_service().await;
    let tmp = tempfile::tempdir().unwrap();
    // No need for actual image bytes — the Fallback branch of analyze_image only
    // looks at the extension. We still create the file so future Ollama-enabled
    // runs of the same test don't 404 in analyze_image's existence check.
    let path = write_temp_file(&tmp, "photo.png", b"fake-png").await;
    let path_str = path.to_string_lossy().to_string();

    let result = svc.analyze_path_with_ai(&path_str).await.expect("dispatch");
    assert_path_set(&result, &path_str);
    assert_eq!(result.category, "Images");
    // The fallback image analyzer tags the result so we can assert on something
    // more specific than "didn't crash".
    assert!(
        result.metadata.get("fallback_analysis").is_some()
            || result.tags.iter().any(|t| t == "image"),
        "expected fallback image analysis marker; got tags={:?} meta={:?}",
        result.tags,
        result.metadata
    );
}

#[tokio::test]
async fn dispatcher_handles_every_supported_image_extension() {
    let svc = fallback_service().await;
    let tmp = tempfile::tempdir().unwrap();

    for ext in ["jpg", "jpeg", "png", "gif", "bmp", "webp"] {
        let name = format!("test.{}", ext);
        let path = write_temp_file(&tmp, &name, b"fake").await;
        let path_str = path.to_string_lossy().to_string();
        let result = svc.analyze_path_with_ai(&path_str).await.expect(ext);
        assert_eq!(
            result.category, "Images",
            "extension {} should route to Images",
            ext
        );
        assert_path_set(&result, &path_str);
    }
}

#[tokio::test]
async fn dispatcher_document_branch_classifies_pdf_even_when_extraction_fails() {
    let svc = fallback_service().await;
    let tmp = tempfile::tempdir().unwrap();
    // Not a real PDF — pdf-extract will error. The dispatcher's job is to
    // *not* propagate the error: it should fall back to the extension-based
    // classifier so the file is still recorded.
    let path = write_temp_file(&tmp, "doc.pdf", b"not-a-real-pdf").await;
    let path_str = path.to_string_lossy().to_string();

    let result = svc.analyze_path_with_ai(&path_str).await.expect("dispatch");
    assert_path_set(&result, &path_str);
    // mime_guess gives application/pdf -> "Documents" in fallback_analysis_with_path.
    assert_eq!(result.category, "Documents");
}

#[tokio::test]
async fn dispatcher_document_branch_handles_markdown() {
    let svc = fallback_service().await;
    let tmp = tempfile::tempdir().unwrap();
    // MarkdownProcessor works without features (uses raw text). The dispatcher
    // routes .md through the document branch, extracts text, then calls the
    // text analyzer.
    let body = b"# Heading\n\nSome paragraph with the word *invoice* in it.\n";
    let path = write_temp_file(&tmp, "notes.md", body).await;
    let path_str = path.to_string_lossy().to_string();

    let result = svc.analyze_path_with_ai(&path_str).await.expect("dispatch");
    assert_path_set(&result, &path_str);
    // Fallback file analysis sees `text/markdown` mime and returns "Text".
    assert!(
        matches!(result.category.as_str(), "Text" | "Documents"),
        "unexpected category for .md: {}",
        result.category
    );
}

#[tokio::test]
async fn dispatcher_text_branch_reads_utf8_files() {
    let svc = fallback_service().await;
    let tmp = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        &tmp,
        "readme.txt",
        b"This document is an invoice for services rendered.",
    )
    .await;
    let path_str = path.to_string_lossy().to_string();

    let result = svc.analyze_path_with_ai(&path_str).await.expect("dispatch");
    assert_path_set(&result, &path_str);
    // Fallback analyzer picks up "invoice" -> tags it. Loose assertion because
    // we don't want to brittle-couple to the exact category mapping.
    assert!(
        result.tags.iter().any(|t| t == "invoice")
            || matches!(result.category.as_str(), "Text" | "Documents"),
        "expected invoice tag or text-ish category; got {:?} / {}",
        result.tags,
        result.category
    );
}

#[tokio::test]
async fn dispatcher_unknown_extension_with_binary_content_does_not_drop_file() {
    let svc = fallback_service().await;
    let tmp = tempfile::tempdir().unwrap();
    // Non-UTF-8 bytes with an extension the dispatcher doesn't recognize.
    // Pre-fix behavior was silently dropping the file. New behavior: fall
    // back to extension-based classification and still return a FileAnalysis.
    let bytes: Vec<u8> = (0u8..=255).collect();
    let path = write_temp_file(&tmp, "blob.bin", &bytes).await;
    let path_str = path.to_string_lossy().to_string();

    let result = svc.analyze_path_with_ai(&path_str).await.expect("dispatch");
    assert_path_set(&result, &path_str);
    // Whatever the category ends up being, it must not be empty.
    assert!(!result.category.is_empty(), "category was empty");
}

#[tokio::test]
async fn dispatcher_unknown_extension_with_utf8_content_uses_text_branch() {
    let svc = fallback_service().await;
    let tmp = tempfile::tempdir().unwrap();
    // Unrecognized extension but valid UTF-8 — the dispatcher should still
    // try to read it as text and call the analyzer.
    let path = write_temp_file(&tmp, "config.weirdext", b"hello world").await;
    let path_str = path.to_string_lossy().to_string();

    let result = svc.analyze_path_with_ai(&path_str).await.expect("dispatch");
    assert_path_set(&result, &path_str);
    assert!(!result.category.is_empty());
}

#[tokio::test]
async fn dispatcher_missing_file_returns_fallback_not_panic() {
    let svc = fallback_service().await;
    let path = "/nonexistent/path/that/does/not/exist.pdf";

    // The dispatcher must not panic and must not propagate a hard I/O error
    // for the document branch — pdf-extract returns Err, and we fall through
    // to extension-based classification. For other branches the test would
    // also not crash.
    let result = svc.analyze_path_with_ai(path).await;
    match result {
        Ok(a) => {
            assert_path_set(&a, path);
            assert!(!a.category.is_empty());
        }
        Err(_) => {
            // An Err is acceptable too — the contract is "no panic" and the
            // caller already handles errors gracefully.
        }
    }
}

#[tokio::test]
async fn dispatcher_capitalized_extensions_are_routed_correctly() {
    let svc = fallback_service().await;
    let tmp = tempfile::tempdir().unwrap();
    // Users on case-sensitive filesystems often see `.PDF` or `.PNG` from
    // cameras / Windows exports. The dispatcher lowercases internally.
    let path = write_temp_file(&tmp, "PHOTO.PNG", b"fake").await;
    let path_str = path.to_string_lossy().to_string();

    let result = svc.analyze_path_with_ai(&path_str).await.expect("dispatch");
    assert_eq!(result.category, "Images");
}

#[tokio::test]
async fn dispatcher_file_with_no_extension_uses_fallback_or_text() {
    let svc = fallback_service().await;
    let tmp = tempfile::tempdir().unwrap();
    let path = write_temp_file(&tmp, "README", b"this is a readme").await;
    let path_str = path.to_string_lossy().to_string();

    let result = svc.analyze_path_with_ai(&path_str).await.expect("dispatch");
    assert_path_set(&result, &path_str);
    assert!(!result.category.is_empty());
}
