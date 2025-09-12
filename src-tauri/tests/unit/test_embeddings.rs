use stratosort::ai::embeddings::{generate_simple_embeddings, normalize_text, hash_text};
use stratosort::error::Result;

#[cfg(test)]
mod embeddings_tests {
    use super::*;

    #[test]
    fn test_generate_simple_embeddings_basic() {
        let text = "This is a test sentence.";
        let embeddings = generate_simple_embeddings(text);
        
        assert!(embeddings.is_ok());
        let embedding_vec = embeddings.unwrap();
        
        // Check embedding vector properties
        assert!(!embedding_vec.is_empty());
        assert!(embedding_vec.len() > 0);
        
        // All values should be finite
        for value in &embedding_vec {
            assert!(value.is_finite());
        }
    }

    #[test]
    fn test_generate_simple_embeddings_empty_text() {
        let embeddings = generate_simple_embeddings("");
        
        assert!(embeddings.is_ok());
        let embedding_vec = embeddings.unwrap();
        
        // Even empty text should produce some embedding
        assert!(!embedding_vec.is_empty());
    }

    #[test]
    fn test_generate_simple_embeddings_consistency() {
        let text = "Consistent text for testing";
        
        let embeddings1 = generate_simple_embeddings(text).unwrap();
        let embeddings2 = generate_simple_embeddings(text).unwrap();
        
        // Same text should produce same embeddings
        assert_eq!(embeddings1.len(), embeddings2.len());
        
        for (e1, e2) in embeddings1.iter().zip(embeddings2.iter()) {
            assert!((e1 - e2).abs() < 0.0001);
        }
    }

    #[test]
    fn test_generate_simple_embeddings_different_texts() {
        let text1 = "First text";
        let text2 = "Completely different text";
        
        let embeddings1 = generate_simple_embeddings(text1).unwrap();
        let embeddings2 = generate_simple_embeddings(text2).unwrap();
        
        // Different texts should produce different embeddings
        assert_eq!(embeddings1.len(), embeddings2.len());
        
        let mut differences = 0;
        for (e1, e2) in embeddings1.iter().zip(embeddings2.iter()) {
            if (e1 - e2).abs() > 0.0001 {
                differences += 1;
            }
        }
        
        // At least some values should be different
        assert!(differences > 0);
    }

    #[test]
    fn test_generate_simple_embeddings_unicode() {
        let unicode_texts = vec![
            "Hello world",
            "你好世界",
            "مرحبا بالعالم",
            "שלום עולם",
            "Здравствуй, мир",
            "こんにちは世界"
        ];
        
        for text in unicode_texts {
            let embeddings = generate_simple_embeddings(text);
            assert!(embeddings.is_ok());
            
            let embedding_vec = embeddings.unwrap();
            assert!(!embedding_vec.is_empty());
            
            // Check all values are valid
            for value in &embedding_vec {
                assert!(value.is_finite());
                assert!(!value.is_nan());
            }
        }
    }

    #[test]
    fn test_generate_simple_embeddings_special_characters() {
        let special_texts = vec![
            "Text with\nnewlines\nand\ttabs",
            "Special chars: @#$%^&*()",
            "Quotes: 'single' and \"double\"",
            "Path: C:\\Windows\\System32",
            "URL: https://example.com/path?query=value",
            "Email: test@example.com"
        ];
        
        for text in special_texts {
            let embeddings = generate_simple_embeddings(text);
            assert!(embeddings.is_ok());
            
            let embedding_vec = embeddings.unwrap();
            assert!(!embedding_vec.is_empty());
        }
    }

    #[test]
    fn test_generate_simple_embeddings_long_text() {
        let long_text = "Lorem ipsum ".repeat(1000); // ~12KB of text
        
        let embeddings = generate_simple_embeddings(&long_text);
        assert!(embeddings.is_ok());
        
        let embedding_vec = embeddings.unwrap();
        assert!(!embedding_vec.is_empty());
        
        // All values should still be valid
        for value in &embedding_vec {
            assert!(value.is_finite());
            assert!(!value.is_nan());
        }
    }

    #[test]
    fn test_generate_simple_embeddings_very_long_text() {
        let very_long_text = "x".repeat(1_000_000); // 1MB of text
        
        let embeddings = generate_simple_embeddings(&very_long_text);
        assert!(embeddings.is_ok());
        
        let embedding_vec = embeddings.unwrap();
        assert!(!embedding_vec.is_empty());
    }

    #[test]
    fn test_normalize_text_basic() {
        let text = "This Is A Test";
        let normalized = normalize_text(text);
        
        assert_eq!(normalized, "this is a test");
    }

    #[test]
    fn test_normalize_text_with_punctuation() {
        let text = "Hello, World! How are you?";
        let normalized = normalize_text(text);
        
        // Should convert to lowercase and keep punctuation
        assert!(normalized.contains("hello"));
        assert!(normalized.contains("world"));
    }

    #[test]
    fn test_normalize_text_empty() {
        let normalized = normalize_text("");
        assert_eq!(normalized, "");
    }

    #[test]
    fn test_normalize_text_unicode() {
        let text = "Café München Zürich";
        let normalized = normalize_text(text);
        
        assert!(normalized.contains("café"));
        assert!(normalized.contains("münchen"));
        assert!(normalized.contains("zürich"));
    }

    #[test]
    fn test_normalize_text_whitespace() {
        let text = "  Multiple   spaces   and\ttabs\n\nnewlines  ";
        let normalized = normalize_text(text);
        
        // Should handle various whitespace consistently
        assert!(!normalized.starts_with(' '));
        assert!(!normalized.ends_with(' '));
    }

    #[test]
    fn test_hash_text_basic() {
        let text = "Test text for hashing";
        let hash = hash_text(text);
        
        assert!(hash != 0);
    }

    #[test]
    fn test_hash_text_consistency() {
        let text = "Consistent text";
        
        let hash1 = hash_text(text);
        let hash2 = hash_text(text);
        
        // Same text should produce same hash
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_hash_text_different_inputs() {
        let text1 = "First text";
        let text2 = "Second text";
        
        let hash1 = hash_text(text1);
        let hash2 = hash_text(text2);
        
        // Different texts should produce different hashes
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_hash_text_empty() {
        let hash = hash_text("");
        
        // Even empty text should produce a hash
        assert!(hash != 0);
    }

    #[test]
    fn test_hash_text_unicode() {
        let unicode_texts = vec![
            ("English", "Hello"),
            ("Chinese", "你好"),
            ("Arabic", "مرحبا"),
            ("Hebrew", "שלום"),
            ("Russian", "Привет"),
            ("Japanese", "こんにちは")
        ];
        
        let mut hashes = Vec::new();
        
        for (_, text) in &unicode_texts {
            let hash = hash_text(text);
            assert!(hash != 0);
            hashes.push(hash);
        }
        
        // All different texts should produce different hashes
        for i in 0..hashes.len() {
            for j in i+1..hashes.len() {
                assert_ne!(hashes[i], hashes[j], 
                    "Hash collision between {} and {}", 
                    unicode_texts[i].0, unicode_texts[j].0);
            }
        }
    }

    #[test]
    fn test_embedding_vector_properties() {
        let texts = vec![
            "Short",
            "Medium length text with more words",
            "Very long text that contains many words and should still produce valid embeddings regardless of length"
        ];
        
        for text in texts {
            let embeddings = generate_simple_embeddings(text).unwrap();
            
            // Check vector length is consistent
            assert!(embeddings.len() > 0);
            
            // Check values are in reasonable range
            for value in &embeddings {
                assert!(value.is_finite());
                assert!(*value >= -1.0 && *value <= 1.0, 
                    "Embedding value {} out of expected range", value);
            }
            
            // Check vector has some variance (not all same value)
            let first = embeddings[0];
            let has_variance = embeddings.iter().any(|&v| (v - first).abs() > 0.0001);
            assert!(has_variance, "Embedding vector has no variance");
        }
    }

    #[test]
    fn test_embedding_similarity() {
        // Similar texts should produce similar embeddings
        let text1 = "The cat sits on the mat";
        let text2 = "The cat sits on the rug";
        let text3 = "The dog runs in the park";
        
        let emb1 = generate_simple_embeddings(text1).unwrap();
        let emb2 = generate_simple_embeddings(text2).unwrap();
        let emb3 = generate_simple_embeddings(text3).unwrap();
        
        // Calculate simple cosine similarity
        let similarity_1_2 = cosine_similarity(&emb1, &emb2);
        let similarity_1_3 = cosine_similarity(&emb1, &emb3);
        
        // Text 1 and 2 should be more similar than 1 and 3
        assert!(similarity_1_2 > similarity_1_3,
            "Expected similar texts to have higher similarity score");
    }

    // Helper function for cosine similarity
    fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        assert_eq!(a.len(), b.len());
        
        let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let magnitude_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let magnitude_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
        
        if magnitude_a == 0.0 || magnitude_b == 0.0 {
            return 0.0;
        }
        
        dot_product / (magnitude_a * magnitude_b)
    }

    #[test]
    fn test_embedding_determinism() {
        let text = "Deterministic embedding test";
        
        // Generate embeddings multiple times
        let mut embeddings_list = Vec::new();
        for _ in 0..5 {
            let emb = generate_simple_embeddings(text).unwrap();
            embeddings_list.push(emb);
        }
        
        // All should be identical
        for i in 1..embeddings_list.len() {
            assert_eq!(embeddings_list[0].len(), embeddings_list[i].len());
            
            for j in 0..embeddings_list[0].len() {
                assert!((embeddings_list[0][j] - embeddings_list[i][j]).abs() < 0.0001,
                    "Embeddings not deterministic at index {}", j);
            }
        }
    }

    #[test]
    fn test_embedding_edge_cases() {
        let edge_cases = vec![
            " ",           // Single space
            "\n",          // Single newline
            "\t",          // Single tab
            ".",           // Single punctuation
            "1",           // Single digit
            "😀",          // Single emoji
            "　",          // Full-width space
            "\u{200B}",    // Zero-width space
        ];
        
        for text in edge_cases {
            let embeddings = generate_simple_embeddings(text);
            assert!(embeddings.is_ok(), "Failed to generate embeddings for: {:?}", text);
            
            let embedding_vec = embeddings.unwrap();
            assert!(!embedding_vec.is_empty());
            
            // Check all values are valid
            for value in &embedding_vec {
                assert!(value.is_finite());
                assert!(!value.is_nan());
            }
        }
    }
}