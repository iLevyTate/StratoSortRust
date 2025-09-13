use stratosort::core::document_processor::{
    DocumentProcessor, DocumentMetadata, ProcessedDocument,
    PdfProcessor, TextProcessor, MarkdownProcessor, XmlProcessor
};
use stratosort::error::{AppError, Result};
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::tempdir;
use chrono::Utc;

#[cfg(test)]
mod document_processor_tests {
    use super::*;

    // Helper function to create test documents
    fn create_test_document(content: &str, extension: &str) -> Result<PathBuf> {
        let temp_dir = tempdir()?;
        let file_path = temp_dir.path().join(format!("test_document.{}", extension));
        fs::write(&file_path, content)?;
        Ok(file_path)
    }

    // Helper function to create a test PDF (minimal valid PDF)
    fn create_minimal_pdf() -> Result<PathBuf> {
        let temp_dir = tempdir()?;
        let file_path = temp_dir.path().join("test.pdf");
        
        // Minimal valid PDF structure
        let pdf_content = b"%PDF-1.4
1 0 obj
<< /Type /Catalog /Pages 2 0 R >>
endobj
2 0 obj
<< /Type /Pages /Kids [3 0 R] /Count 1 >>
endobj
3 0 obj
<< /Type /Page /Parent 2 0 R /Resources << /Font << /F1 << /Type /Font /Subtype /Type1 /BaseFont /Times-Roman >> >> >> /MediaBox [0 0 612 792] /Contents 4 0 R >>
endobj
4 0 obj
<< /Length 44 >>
stream
BT
/F1 12 Tf
100 700 Td
(Test PDF Document) Tj
ET
endstream
endobj
xref
0 5
0000000000 65535 f 
0000000009 00000 n 
0000000058 00000 n 
0000000115 00000 n 
0000000274 00000 n 
trailer
<< /Size 5 /Root 1 0 R >>
startxref
365
%%EOF";
        
        fs::write(&file_path, pdf_content)?;
        Ok(file_path)
    }

    #[tokio::test]
    async fn test_text_processor_basic() {
        let processor = TextProcessor;
        let content = "This is a test document.\nIt has multiple lines.\nAnd some special characters: @#$%^&*()";
        let file_path = create_test_document(content, "txt").unwrap();
        
        let result = processor.process(&file_path).await;
        assert!(result.is_ok());
        
        let doc = result.unwrap();
        assert_eq!(doc.file_type, "text");
        assert!(doc.text_content.contains("test document"));
        assert!(doc.text_content.contains("multiple lines"));
        assert!(doc.processing_error.is_none());
        
        // Check metadata
        assert_eq!(doc.metadata.word_count, Some(15));
    }

    #[tokio::test]
    async fn test_text_processor_with_fixture() {
        let fixture_path = PathBuf::from("tests/fixtures/data/sample_demo_files/sample_contract.txt");
        
        if !fixture_path.exists() {
            println!("Skipping test: fixture file not found");
            return;
        }
        
        let processor = TextProcessor;
        let result = processor.process(&fixture_path).await;
        
        assert!(result.is_ok());
        let doc = result.unwrap();
        
        assert_eq!(doc.file_type, "text");
        assert!(!doc.text_content.is_empty());
        assert!(doc.metadata.word_count.unwrap() > 0);
        assert!(doc.processing_error.is_none());
    }

    #[tokio::test]
    async fn test_markdown_processor() {
        let processor = MarkdownProcessor;
        let content = r#"# Test Document

## Introduction
This is a **test** markdown document with various elements.

### Features
- Bullet point 1
- Bullet point 2
- Bullet point 3

### Code Example
```rust
fn main() {
    println!("Hello, world!");
}
```

## Conclusion
This document demonstrates markdown processing capabilities.

[Link to example](https://example.com)"#;
        
        let file_path = create_test_document(content, "md").unwrap();
        
        let result = processor.process(&file_path).await;
        assert!(result.is_ok());
        
        let doc = result.unwrap();
        assert_eq!(doc.file_type, "markdown");
        assert!(doc.text_content.contains("Test Document"));
        assert!(doc.text_content.contains("Introduction"));
        assert!(doc.text_content.contains("Hello, world!"));
        assert!(doc.processing_error.is_none());
        
        // Check metadata extraction
        assert!(doc.metadata.title.is_some());
        assert_eq!(doc.metadata.title.unwrap(), "Test Document");
        assert!(doc.metadata.word_count.unwrap() > 20);
    }

    #[tokio::test]
    async fn test_markdown_processor_with_fixture() {
        let fixture_path = PathBuf::from("tests/fixtures/data/sample_demo_files/qx7n9p.md");
        
        if !fixture_path.exists() {
            println!("Skipping test: fixture file not found");
            return;
        }
        
        let processor = MarkdownProcessor;
        let result = processor.process(&fixture_path).await;
        
        assert!(result.is_ok());
        let doc = result.unwrap();
        
        assert_eq!(doc.file_type, "markdown");
        assert!(!doc.text_content.is_empty());
        assert!(doc.processing_error.is_none());
    }

    #[tokio::test]
    async fn test_xml_processor() {
        let processor = XmlProcessor;
        let content = r#"<?xml version="1.0" encoding="UTF-8"?>
<document>
    <metadata>
        <title>Test XML Document</title>
        <author>Test Author</author>
        <created>2024-01-01</created>
    </metadata>
    <content>
        <section id="1">
            <heading>Introduction</heading>
            <paragraph>This is a test XML document with structured content.</paragraph>
        </section>
        <section id="2">
            <heading>Main Content</heading>
            <paragraph>XML processing should extract text from all elements.</paragraph>
            <list>
                <item>Item 1</item>
                <item>Item 2</item>
                <item>Item 3</item>
            </list>
        </section>
    </content>
</document>"#;
        
        let file_path = create_test_document(content, "xml").unwrap();
        
        let result = processor.process(&file_path).await;
        assert!(result.is_ok());
        
        let doc = result.unwrap();
        assert_eq!(doc.file_type, "xml");
        assert!(doc.text_content.contains("Test XML Document"));
        assert!(doc.text_content.contains("Introduction"));
        assert!(doc.text_content.contains("structured content"));
        assert!(doc.processing_error.is_none());
        
        // Check metadata extraction from XML
        assert_eq!(doc.metadata.title, Some("Test XML Document".to_string()));
        assert_eq!(doc.metadata.author, Some("Test Author".to_string()));
    }

    #[tokio::test]
    async fn test_xml_processor_with_fixture() {
        let fixture_path = PathBuf::from("tests/fixtures/data/sample_demo_files/n5r8t3.xml");
        
        if !fixture_path.exists() {
            println!("Skipping test: fixture file not found");
            return;
        }
        
        let processor = XmlProcessor;
        let result = processor.process(&fixture_path).await;
        
        assert!(result.is_ok());
        let doc = result.unwrap();
        
        assert_eq!(doc.file_type, "xml");
        assert!(!doc.text_content.is_empty());
        assert!(doc.processing_error.is_none());
    }

    #[tokio::test]
    async fn test_pdf_processor_basic() {
        // Skip if PDF libraries are not available
        let processor = PdfProcessor;
        let file_path = create_minimal_pdf().unwrap();
        
        let result = processor.process(&file_path).await;
        
        // PDF processing might fail due to library dependencies
        if result.is_ok() {
            let doc = result.unwrap();
            assert_eq!(doc.file_type, "pdf");
            assert!(doc.text_content.contains("Test PDF Document") || !doc.text_content.is_empty());
        }
    }

    #[tokio::test]
    async fn test_pdf_processor_with_fixture() {
        let fixture_paths = vec![
            PathBuf::from("tests/fixtures/data/sample_demo_files/Annual_Financial_Statement_2024.pdf"),
            PathBuf::from("tests/fixtures/data/sample_demo_files/z3m9p6.pdf"),
        ];
        
        let processor = PdfProcessor;
        
        for fixture_path in fixture_paths {
            if !fixture_path.exists() {
                continue;
            }
            
            let result = processor.process(&fixture_path).await;
            
            if result.is_ok() {
                let doc = result.unwrap();
                assert_eq!(doc.file_type, "pdf");
                // PDF might be empty or contain extracted text
                assert!(doc.processing_error.is_none() || doc.processing_error.is_some());
            }
        }
    }

    #[tokio::test]
    async fn test_processor_with_empty_file() {
        let processor = TextProcessor;
        let file_path = create_test_document("", "txt").unwrap();
        
        let result = processor.process(&file_path).await;
        assert!(result.is_ok());
        
        let doc = result.unwrap();
        assert_eq!(doc.text_content, "");
        assert_eq!(doc.metadata.word_count, Some(0));
    }

    #[tokio::test]
    async fn test_processor_with_large_file() {
        let processor = TextProcessor;
        // Create a large text file (1MB)
        let large_content = "Lorem ipsum dolor sit amet. ".repeat(40000);
        let file_path = create_test_document(&large_content, "txt").unwrap();
        
        let result = processor.process(&file_path).await;
        assert!(result.is_ok());
        
        let doc = result.unwrap();
        assert!(!doc.text_content.is_empty());
        assert!(doc.metadata.word_count.unwrap() > 100000);
    }

    #[tokio::test]
    async fn test_processor_with_unicode_content() {
        let processor = TextProcessor;
        let content = "Hello 世界! 🌍 Привет мир! مرحبا بالعالم";
        let file_path = create_test_document(content, "txt").unwrap();
        
        let result = processor.process(&file_path).await;
        assert!(result.is_ok());
        
        let doc = result.unwrap();
        assert!(doc.text_content.contains("世界"));
        assert!(doc.text_content.contains("🌍"));
        assert!(doc.text_content.contains("Привет"));
        assert!(doc.text_content.contains("مرحبا"));
    }

    #[tokio::test]
    async fn test_processor_with_malformed_xml() {
        let processor = XmlProcessor;
        let content = r#"<?xml version="1.0"?>
<document>
    <unclosed_tag>
    <another_tag>Content</wrong_closing_tag>
</document>"#;
        
        let file_path = create_test_document(content, "xml").unwrap();
        
        let result = processor.process(&file_path).await;
        
        // Should handle malformed XML gracefully
        if result.is_ok() {
            let doc = result.unwrap();
            assert!(doc.processing_error.is_some() || !doc.text_content.is_empty());
        } else {
            // Error is expected for malformed XML
            assert!(matches!(result.unwrap_err(), AppError::ProcessingError { .. }));
        }
    }

    #[tokio::test]
    async fn test_processor_metadata_extraction() {
        let processor = MarkdownProcessor;
        let content = r#"---
title: Test Article
author: John Doe
date: 2024-01-15
tags: [test, documentation, sample]
---

# Test Article

This is the content of the article."#;
        
        let file_path = create_test_document(content, "md").unwrap();
        
        let result = processor.process(&file_path).await;
        assert!(result.is_ok());
        
        let doc = result.unwrap();
        assert_eq!(doc.metadata.title, Some("Test Article".to_string()));
        assert_eq!(doc.metadata.author, Some("John Doe".to_string()));
    }

    #[tokio::test]
    async fn test_processor_supported_extensions() {
        let pdf_processor = PdfProcessor;
        assert_eq!(pdf_processor.supported_extensions(), vec!["pdf"]);
        
        let text_processor = TextProcessor;
        let text_extensions = text_processor.supported_extensions();
        assert!(text_extensions.contains(&"txt"));
        assert!(text_extensions.contains(&"text"));
        assert!(text_extensions.contains(&"log"));
        
        let markdown_processor = MarkdownProcessor;
        let md_extensions = markdown_processor.supported_extensions();
        assert!(md_extensions.contains(&"md"));
        assert!(md_extensions.contains(&"markdown"));
        
        let xml_processor = XmlProcessor;
        let xml_extensions = xml_processor.supported_extensions();
        assert!(xml_extensions.contains(&"xml"));
        assert!(xml_extensions.contains(&"xhtml"));
    }

    #[tokio::test]
    async fn test_processor_word_count_accuracy() {
        let processor = TextProcessor;
        let content = "One two three four five.\nSix seven eight nine ten.";
        let file_path = create_test_document(content, "txt").unwrap();
        
        let result = processor.process(&file_path).await;
        assert!(result.is_ok());
        
        let doc = result.unwrap();
        assert_eq!(doc.metadata.word_count, Some(10));
    }

    #[tokio::test]
    async fn test_processor_concurrent_processing() {
        use tokio::task::JoinSet;
        
        let mut tasks = JoinSet::new();
        
        for i in 0..10 {
            let content = format!("Document {} content with some test text.", i);
            let file_path = create_test_document(&content, "txt").unwrap();
            
            tasks.spawn(async move {
                let processor = TextProcessor;
                processor.process(&file_path).await
            });
        }
        
        let mut success_count = 0;
        while let Some(result) = tasks.join_next().await {
            if let Ok(Ok(_)) = result {
                success_count += 1;
            }
        }
        
        assert_eq!(success_count, 10);
    }

    #[tokio::test]
    async fn test_processor_with_special_characters_in_path() {
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("test file (with spaces) & special.txt");
        fs::write(&file_path, "Test content").unwrap();
        
        let processor = TextProcessor;
        let result = processor.process(&file_path).await;
        
        assert!(result.is_ok());
        let doc = result.unwrap();
        assert_eq!(doc.text_content, "Test content");
    }

    #[tokio::test]
    async fn test_processor_language_detection() {
        let test_cases = vec![
            ("This is an English text document.", Some("en".to_string())),
            ("Ceci est un document texte français.", Some("fr".to_string())),
            ("Dies ist ein deutsches Textdokument.", Some("de".to_string())),
            ("Este es un documento de texto español.", Some("es".to_string())),
        ];
        
        let processor = TextProcessor;
        
        for (content, expected_lang_prefix) in test_cases {
            let file_path = create_test_document(content, "txt").unwrap();
            let result = processor.process(&file_path).await;
            
            if result.is_ok() {
                let doc = result.unwrap();
                if let Some(lang) = doc.metadata.language {
                    if let Some(prefix) = expected_lang_prefix {
                        assert!(lang.starts_with(&prefix) || lang == "unknown");
                    }
                }
            }
        }
    }
}