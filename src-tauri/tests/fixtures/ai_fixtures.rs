use stratosort::ai::{FileAnalysis, OrganizationSuggestion};

/// Mock AI responses for testing without requiring actual AI service
pub struct MockAiResponses;

impl MockAiResponses {
    /// Returns a mock analysis based on file content and type
    pub fn mock_file_analysis(content: &str, file_type: &str, file_path: &str) -> FileAnalysis {
        let path_lower = file_path.to_lowercase();
        let content_lower = content.to_lowercase();
        
        // Determine category based on file type and extension
        let category = if file_type.starts_with("image/") {
            "Images"
        } else if file_type.starts_with("video/") {
            "Videos"
        } else if file_type.starts_with("audio/") {
            "Audio"
        } else if file_type.contains("pdf") || file_type.contains("document") || file_type.contains("text") {
            "Documents"
        } else {
            "Other"
        };

        // Generate tags based on content and filename
        let mut tags = Vec::new();
        
        // Content-based tags
        if content_lower.contains("contract") || content_lower.contains("agreement") {
            tags.extend_from_slice(&["contract", "legal", "agreement"]);
        }
        
        if content_lower.contains("invoice") || content_lower.contains("bill") {
            tags.extend_from_slice(&["invoice", "financial", "billing"]);
        }
        
        if content_lower.contains("receipt") {
            tags.extend_from_slice(&["receipt", "expense", "financial"]);
        }
        
        if content_lower.contains("report") {
            tags.extend_from_slice(&["report", "analysis", "data"]);
        }
        
        if content_lower.contains("presentation") || path_lower.contains("pptx") || path_lower.contains("ppt") {
            tags.extend_from_slice(&["presentation", "slides", "meeting"]);
        }
        
        if content_lower.contains("training") || content_lower.contains("onboarding") {
            tags.extend_from_slice(&["training", "educational", "learning"]);
        }
        
        if content_lower.contains("policy") || content_lower.contains("privacy") {
            tags.extend_from_slice(&["policy", "legal", "compliance"]);
        }
        
        if content_lower.contains("backup") || path_lower.contains("backup") {
            tags.extend_from_slice(&["backup", "archive", "storage"]);
        }
        
        // Filename-based tags
        if path_lower.contains("2024") {
            tags.push("2024");
        }
        
        if path_lower.contains("q1") || path_lower.contains("quarter") {
            tags.push("quarterly");
        }
        
        if path_lower.contains("march") || path_lower.contains("april") {
            tags.push("monthly");
        }
        
        if path_lower.contains("team") {
            tags.push("team");
        }
        
        if path_lower.contains("photo") {
            tags.push("photo");
        }
        
        if path_lower.contains("video") {
            tags.push("video");
        }
        
        // Remove duplicates and convert to strings
        let mut unique_tags: Vec<String> = tags.into_iter()
            .map(|s| s.to_string())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        unique_tags.sort();
        
        // Generate summary based on content and context
        let summary = Self::generate_summary(content, file_type, &path_lower, &unique_tags);
        
        // Calculate confidence based on content analysis quality
        let confidence = Self::calculate_confidence(content, &unique_tags);
        
        // Extract language if text content
        let detected_language = if !content.is_empty() && category == "Documents" {
            Some("en".to_string()) // Assume English for simplicity
        } else {
            None
        };
        
        // Extract text for searchable content
        let extracted_text = if !content.is_empty() && content.len() > 50 {
            Some(content[..content.len().min(500)].to_string()) // First 500 chars
        } else if !content.is_empty() {
            Some(content.to_string())
        } else {
            None
        };

        FileAnalysis {
            path: file_path.to_string(),
            category: category.to_string(),
            tags: unique_tags,
            summary,
            confidence,
            extracted_text,
            detected_language,
            metadata: serde_json::json!({
                "mock_analysis": true,
                "file_size": content.len(),
                "analysis_timestamp": chrono::Utc::now().to_rfc3339()
            }),
        }
    }

    /// Generate organization suggestions based on file paths
    pub fn mock_organization_suggestions(file_paths: &[String]) -> Vec<OrganizationSuggestion> {
        file_paths.iter().map(|path| {
            let path_lower = path.to_lowercase();
            
            let (target_folder, confidence) = if path_lower.contains(".jpg") || 
                path_lower.contains(".png") || 
                path_lower.contains(".gif") || 
                path_lower.contains(".jpeg") {
                ("Images", 0.9)
            } else if path_lower.contains(".mp4") || 
                path_lower.contains(".avi") || 
                path_lower.contains(".mov") || 
                path_lower.contains(".mkv") {
                ("Videos", 0.9)
            } else if path_lower.contains(".mp3") || 
                path_lower.contains(".wav") || 
                path_lower.contains(".flac") || 
                path_lower.contains(".m4a") {
                ("Audio", 0.9)
            } else if path_lower.contains(".pdf") || 
                path_lower.contains(".doc") || 
                path_lower.contains(".docx") || 
                path_lower.contains(".txt") || 
                path_lower.contains(".xlsx") || 
                path_lower.contains(".pptx") {
                
                // Sub-categorize documents
                if path_lower.contains("contract") || path_lower.contains("legal") {
                    ("Documents/Legal", 0.85)
                } else if path_lower.contains("invoice") || path_lower.contains("receipt") || path_lower.contains("financial") {
                    ("Documents/Financial", 0.85)
                } else if path_lower.contains("report") || path_lower.contains("analysis") {
                    ("Documents/Reports", 0.8)
                } else if path_lower.contains("presentation") || path_lower.contains("pptx") {
                    ("Documents/Presentations", 0.8)
                } else {
                    ("Documents", 0.75)
                }
            } else if path_lower.contains(".zip") || 
                path_lower.contains(".rar") || 
                path_lower.contains(".7z") || 
                path_lower.contains(".tar") {
                ("Archives", 0.8)
            } else {
                ("Other", 0.5)
            };

            let reason = match target_folder {
                "Images" => format!("Image file detected based on extension"),
                "Videos" => format!("Video file detected based on extension"),
                "Audio" => format!("Audio file detected based on extension"),
                "Archives" => format!("Archive file detected based on extension"),
                folder if folder.starts_with("Documents") => {
                    if folder.contains("/") {
                        format!("Document file with specific category: {}", folder.split('/').last().unwrap())
                    } else {
                        format!("Document file detected based on extension")
                    }
                },
                _ => format!("File type classification based on extension and content analysis")
            };

            OrganizationSuggestion {
                source_path: path.clone(),
                target_folder: target_folder.to_string(),
                reason,
                confidence,
            }
        }).collect()
    }

    /// Generate mock embeddings for semantic search
    pub fn mock_embeddings(text: &str) -> Vec<f32> {
        // Create deterministic but varied embeddings based on text content
        let text_bytes = text.as_bytes();
        let mut embedding = Vec::with_capacity(384); // Common embedding dimension
        
        // Use text characteristics to generate embedding
        let text_lower = text.to_lowercase();
        let word_count = text.split_whitespace().count();
        let char_count = text.len();
        
        // Base values influenced by content
        let base_contract = if text_lower.contains("contract") || text_lower.contains("agreement") { 0.8 } else { 0.1 };
        let base_financial = if text_lower.contains("invoice") || text_lower.contains("payment") { 0.8 } else { 0.1 };
        let base_report = if text_lower.contains("report") || text_lower.contains("analysis") { 0.8 } else { 0.1 };
        let base_media = if text_lower.contains("image") || text_lower.contains("video") { 0.8 } else { 0.1 };
        
        for i in 0..384 {
            let mut value = 0.0;
            
            // Create patterns based on text content
            match i % 8 {
                0 => value = base_contract + (text_bytes.get(i % text_bytes.len()).unwrap_or(&0) as f32 / 255.0) * 0.2,
                1 => value = base_financial + (word_count as f32 / 1000.0).min(0.3),
                2 => value = base_report + (char_count as f32 / 10000.0).min(0.3),
                3 => value = base_media + (text_bytes.len() as f32 / 1000.0).min(0.3),
                4 => value = (text_bytes.get((i * 2) % text_bytes.len()).unwrap_or(&0) as f32 / 255.0) * 0.6,
                5 => value = (text_bytes.get((i * 3) % text_bytes.len()).unwrap_or(&0) as f32 / 255.0) * 0.4,
                6 => value = ((i as f32).sin() + 1.0) / 2.0 * 0.3,
                7 => value = ((i as f32).cos() + 1.0) / 2.0 * 0.3,
                _ => value = 0.1,
            }
            
            // Add some noise for realism
            let noise = ((text_bytes.get(i % text_bytes.len()).unwrap_or(&0) as f32 * i as f32) % 100.0) / 1000.0;
            value = (value + noise).max(0.0).min(1.0);
            
            embedding.push(value);
        }
        
        // Normalize the embedding
        let magnitude: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        if magnitude > 0.0 {
            embedding.iter_mut().for_each(|x| *x /= magnitude);
        }
        
        embedding
    }

    /// Calculate similarity between two embeddings
    pub fn calculate_similarity(embedding1: &[f32], embedding2: &[f32]) -> f32 {
        if embedding1.len() != embedding2.len() {
            return 0.0;
        }
        
        // Cosine similarity
        let dot_product: f32 = embedding1.iter()
            .zip(embedding2.iter())
            .map(|(a, b)| a * b)
            .sum();
        
        let magnitude1: f32 = embedding1.iter().map(|x| x * x).sum::<f32>().sqrt();
        let magnitude2: f32 = embedding2.iter().map(|x| x * x).sum::<f32>().sqrt();
        
        if magnitude1 == 0.0 || magnitude2 == 0.0 {
            return 0.0;
        }
        
        dot_product / (magnitude1 * magnitude2)
    }

    // Helper methods
    
    fn generate_summary(content: &str, file_type: &str, path: &str, tags: &[String]) -> String {
        if content.is_empty() {
            return format!("File type: {}", file_type);
        }
        
        let content_preview = if content.len() > 100 {
            &content[..100]
        } else {
            content
        };
        
        if tags.contains(&"contract".to_string()) {
            "Legal contract document containing terms, conditions, and agreements between parties."
        } else if tags.contains(&"invoice".to_string()) {
            "Financial invoice document with billing information, amounts, and payment terms."
        } else if tags.contains(&"report".to_string()) {
            "Business report containing analysis, metrics, and performance data."
        } else if tags.contains(&"presentation".to_string()) {
            "Presentation document with slides, charts, and visual content for meetings."
        } else if tags.contains(&"photo".to_string()) {
            "Photograph or image file, possibly from events or documentation."
        } else if tags.contains(&"training".to_string()) {
            "Training or educational material for learning and development purposes."
        } else if tags.contains(&"policy".to_string()) {
            "Policy document outlining rules, procedures, and compliance requirements."
        } else if path.contains("receipt") {
            "Receipt or expense documentation for financial record keeping."
        } else {
            // Generate summary from content
            let words: Vec<&str> = content_preview.split_whitespace().take(10).collect();
            if !words.is_empty() {
                format!("Document content: {}", words.join(" "))
            } else {
                format!("File type: {}", file_type)
            }
        }.to_string()
    }
    
    fn calculate_confidence(content: &str, tags: &[String]) -> f32 {
        let mut confidence = 0.5; // Base confidence
        
        // Increase confidence based on content quality
        if !content.is_empty() {
            confidence += 0.2;
            
            if content.len() > 100 {
                confidence += 0.1;
            }
        }
        
        // Increase confidence based on tag matches
        confidence += (tags.len() as f32 * 0.05).min(0.3);
        
        // Specific high-confidence patterns
        let content_lower = content.to_lowercase();
        if content_lower.contains("invoice") || content_lower.contains("contract") {
            confidence += 0.2;
        }
        
        confidence.min(1.0)
    }
}

/// Predefined test scenarios with expected AI responses
pub struct AiTestScenarios;

impl AiTestScenarios {
    /// Financial documents scenario
    pub fn financial_documents() -> Vec<(String, String, FileAnalysis)> {
        vec![
            (
                "INVOICE #2024-001\nBill To: Acme Corp\nAmount: $5,000.00".to_string(),
                "application/pdf".to_string(),
                FileAnalysis {
                    path: "/invoices/invoice_2024_001.pdf".to_string(),
                    category: "Documents".to_string(),
                    tags: vec!["invoice".to_string(), "financial".to_string(), "2024".to_string()],
                    summary: "Financial invoice document with billing information, amounts, and payment terms.".to_string(),
                    confidence: 0.95,
                    extracted_text: Some("INVOICE #2024-001\nBill To: Acme Corp\nAmount: $5,000.00".to_string()),
                    detected_language: Some("en".to_string()),
                    metadata: serde_json::json!({
                        "amount": 5000.00,
                        "currency": "USD",
                        "invoice_number": "2024-001"
                    }),
                }
            ),
            (
                "RECEIPT\nStore: OfficeMax\nDate: 03/15/2024\nTotal: $127.45".to_string(),
                "image/jpeg".to_string(),
                FileAnalysis {
                    path: "/receipts/office_supplies.jpg".to_string(),
                    category: "Images".to_string(),
                    tags: vec!["receipt".to_string(), "expense".to_string(), "financial".to_string()],
                    summary: "Receipt or expense documentation for financial record keeping.".to_string(),
                    confidence: 0.92,
                    extracted_text: Some("RECEIPT\nStore: OfficeMax\nDate: 03/15/2024\nTotal: $127.45".to_string()),
                    detected_language: Some("en".to_string()),
                    metadata: serde_json::json!({
                        "amount": 127.45,
                        "vendor": "OfficeMax",
                        "date": "2024-03-15"
                    }),
                }
            )
        ]
    }

    /// Legal documents scenario
    pub fn legal_documents() -> Vec<(String, String, FileAnalysis)> {
        vec![
            (
                "SOFTWARE DEVELOPMENT AGREEMENT\nThis agreement is between...".to_string(),
                "application/pdf".to_string(),
                FileAnalysis {
                    path: "/contracts/dev_agreement.pdf".to_string(),
                    category: "Documents".to_string(),
                    tags: vec!["contract".to_string(), "legal".to_string(), "agreement".to_string()],
                    summary: "Legal contract document containing terms, conditions, and agreements between parties.".to_string(),
                    confidence: 0.96,
                    extracted_text: Some("SOFTWARE DEVELOPMENT AGREEMENT\nThis agreement is between...".to_string()),
                    detected_language: Some("en".to_string()),
                    metadata: serde_json::json!({
                        "document_type": "agreement",
                        "parties": 2,
                        "pages": 12
                    }),
                }
            )
        ]
    }

    /// Media files scenario  
    pub fn media_files() -> Vec<(String, String, FileAnalysis)> {
        vec![
            (
                "".to_string(), // Binary content
                "image/jpeg".to_string(),
                FileAnalysis {
                    path: "/photos/team_event.jpg".to_string(),
                    category: "Images".to_string(),
                    tags: vec!["photo".to_string(), "team".to_string()],
                    summary: "Photograph or image file, possibly from events or documentation.".to_string(),
                    confidence: 0.85,
                    extracted_text: None,
                    detected_language: None,
                    metadata: serde_json::json!({
                        "width": 1920,
                        "height": 1080,
                        "camera": "iPhone 12"
                    }),
                }
            ),
            (
                "".to_string(), // Binary content
                "video/mp4".to_string(),
                FileAnalysis {
                    path: "/videos/training_session.mp4".to_string(),
                    category: "Videos".to_string(),
                    tags: vec!["video".to_string(), "training".to_string()],
                    summary: "Training or educational material for learning and development purposes.".to_string(),
                    confidence: 0.88,
                    extracted_text: None,
                    detected_language: Some("en".to_string()),
                    metadata: serde_json::json!({
                        "duration": 1800,
                        "resolution": "1920x1080",
                        "codec": "H.264"
                    }),
                }
            )
        ]
    }

    /// Get all test scenarios combined
    pub fn all_scenarios() -> Vec<(String, String, FileAnalysis)> {
        let mut all = Vec::new();
        all.extend(Self::financial_documents());
        all.extend(Self::legal_documents());
        all.extend(Self::media_files());
        all
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_file_analysis_contract() {
        let content = "SOFTWARE DEVELOPMENT AGREEMENT\nThis agreement outlines the terms...";
        let analysis = MockAiResponses::mock_file_analysis(content, "application/pdf", "/contracts/dev_agreement.pdf");
        
        assert_eq!(analysis.category, "Documents");
        assert!(analysis.tags.contains(&"contract".to_string()));
        assert!(analysis.tags.contains(&"legal".to_string()));
        assert!(analysis.confidence > 0.8);
        assert!(analysis.summary.contains("contract") || analysis.summary.contains("legal"));
    }

    #[test]
    fn test_mock_file_analysis_invoice() {
        let content = "INVOICE #001\nBill To: Company\nAmount: $1,500.00";
        let analysis = MockAiResponses::mock_file_analysis(content, "application/pdf", "/invoices/inv001.pdf");
        
        assert_eq!(analysis.category, "Documents");
        assert!(analysis.tags.contains(&"invoice".to_string()));
        assert!(analysis.tags.contains(&"financial".to_string()));
        assert!(analysis.confidence > 0.8);
    }

    #[test]
    fn test_mock_file_analysis_image() {
        let analysis = MockAiResponses::mock_file_analysis("", "image/jpeg", "/photos/team.jpg");
        
        assert_eq!(analysis.category, "Images");
        assert!(analysis.tags.contains(&"photo".to_string()));
        assert!(analysis.extracted_text.is_none());
    }

    #[test]
    fn test_mock_organization_suggestions() {
        let files = vec![
            "/documents/contract.pdf".to_string(),
            "/photos/image.jpg".to_string(),
            "/videos/training.mp4".to_string(),
            "/audio/podcast.mp3".to_string(),
        ];
        
        let suggestions = MockAiResponses::mock_organization_suggestions(&files);
        
        assert_eq!(suggestions.len(), 4);
        
        let doc_suggestion = suggestions.iter().find(|s| s.source_path.contains("contract")).unwrap();
        assert!(doc_suggestion.target_folder.contains("Documents"));
        
        let image_suggestion = suggestions.iter().find(|s| s.source_path.contains("image")).unwrap();
        assert_eq!(image_suggestion.target_folder, "Images");
        
        let video_suggestion = suggestions.iter().find(|s| s.source_path.contains("training")).unwrap();
        assert_eq!(video_suggestion.target_folder, "Videos");
        
        let audio_suggestion = suggestions.iter().find(|s| s.source_path.contains("podcast")).unwrap();
        assert_eq!(audio_suggestion.target_folder, "Audio");
    }

    #[test]
    fn test_mock_embeddings() {
        let text1 = "This is a contract document with legal terms";
        let text2 = "This is a contract document with legal terms"; // Same text
        let text3 = "This is an invoice with financial information";
        
        let embed1 = MockAiResponses::mock_embeddings(text1);
        let embed2 = MockAiResponses::mock_embeddings(text2);
        let embed3 = MockAiResponses::mock_embeddings(text3);
        
        // Check embedding dimension
        assert_eq!(embed1.len(), 384);
        assert_eq!(embed2.len(), 384);
        assert_eq!(embed3.len(), 384);
        
        // Same text should produce identical embeddings
        assert_eq!(embed1, embed2);
        
        // Different texts should produce different embeddings
        assert_ne!(embed1, embed3);
        
        // Check that embeddings are normalized (values between -1 and 1 approximately)
        for &value in &embed1 {
            assert!(value >= -1.0 && value <= 1.0);
        }
    }

    #[test]
    fn test_embedding_similarity() {
        let text1 = "contract agreement legal document";
        let text2 = "contract agreement legal terms";
        let text3 = "invoice payment financial billing";
        
        let embed1 = MockAiResponses::mock_embeddings(text1);
        let embed2 = MockAiResponses::mock_embeddings(text2);
        let embed3 = MockAiResponses::mock_embeddings(text3);
        
        let similarity_12 = MockAiResponses::calculate_similarity(&embed1, &embed2);
        let similarity_13 = MockAiResponses::calculate_similarity(&embed1, &embed3);
        
        // Similar legal documents should have higher similarity than legal vs financial
        assert!(similarity_12 > similarity_13);
        
        // Self-similarity should be 1.0 (approximately)
        let self_similarity = MockAiResponses::calculate_similarity(&embed1, &embed1);
        assert!((self_similarity - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_ai_test_scenarios() {
        let financial_scenarios = AiTestScenarios::financial_documents();
        assert!(!financial_scenarios.is_empty());
        
        let legal_scenarios = AiTestScenarios::legal_documents();
        assert!(!legal_scenarios.is_empty());
        
        let media_scenarios = AiTestScenarios::media_files();
        assert!(!media_scenarios.is_empty());
        
        let all_scenarios = AiTestScenarios::all_scenarios();
        assert_eq!(all_scenarios.len(), financial_scenarios.len() + legal_scenarios.len() + media_scenarios.len());
    }

    #[test]
    fn test_confidence_calculation() {
        // Test with different content qualities
        let high_quality_content = "This is a detailed software development agreement with specific terms, conditions, payment schedules, and deliverables outlined clearly for both parties involved in the contract.";
        let low_quality_content = "contract";
        let empty_content = "";
        
        let analysis_high = MockAiResponses::mock_file_analysis(high_quality_content, "application/pdf", "/contracts/detailed.pdf");
        let analysis_low = MockAiResponses::mock_file_analysis(low_quality_content, "text/plain", "/file.txt");
        let analysis_empty = MockAiResponses::mock_file_analysis(empty_content, "application/octet-stream", "/file.bin");
        
        assert!(analysis_high.confidence > analysis_low.confidence);
        assert!(analysis_low.confidence > analysis_empty.confidence);
        assert!(analysis_high.confidence > 0.8);
        assert!(analysis_empty.confidence < 0.7);
    }
}