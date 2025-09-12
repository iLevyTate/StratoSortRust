#[cfg(test)]
mod validate_test_structure {
    use std::collections::HashMap;
    use std::fs;
    use std::path::Path;

    const TEST_FIXTURES_PATH: &str =
        r"C:\Users\benja\Documents\GitHub\StratoRust\src-tauri\tests\fixtures\data";
    const SAMPLE_FILES_DIR: &str = "sample_demo_files";
    const SMART_FOLDERS_DIR: &str = "sample_demo_smart_folders";

    #[derive(Debug)]
    struct TestStructureValidation {
        issues: Vec<String>,
        warnings: Vec<String>,
        info: Vec<String>,
    }

    impl TestStructureValidation {
        fn new() -> Self {
            Self {
                issues: Vec::new(),
                warnings: Vec::new(),
                info: Vec::new(),
            }
        }

        fn add_issue(&mut self, issue: String) {
            self.issues.push(issue);
        }

        fn add_warning(&mut self, warning: String) {
            self.warnings.push(warning);
        }

        fn add_info(&mut self, info: String) {
            self.info.push(info);
        }

        fn print_report(&self) {
            println!("\n=== TEST STRUCTURE VALIDATION REPORT ===\n");

            if !self.issues.is_empty() {
                println!("❌ ISSUES FOUND ({}):", self.issues.len());
                for issue in &self.issues {
                    println!("  - {}", issue);
                }
                println!();
            }

            if !self.warnings.is_empty() {
                println!("⚠️ WARNINGS ({}):", self.warnings.len());
                for warning in &self.warnings {
                    println!("  - {}", warning);
                }
                println!();
            }

            if !self.info.is_empty() {
                println!("ℹ️ INFO ({}):", self.info.len());
                for info in &self.info {
                    println!("  - {}", info);
                }
                println!();
            }

            if self.issues.is_empty() && self.warnings.is_empty() {
                println!("✅ Test structure validation passed!");
            }
        }
    }

    #[test]
    fn test_validate_fixture_structure() {
        let mut validation = TestStructureValidation::new();

        // Check if main directories exist
        let fixtures_path = Path::new(TEST_FIXTURES_PATH);
        if !fixtures_path.exists() {
            validation.add_issue(format!(
                "Test fixtures directory does not exist: {}",
                TEST_FIXTURES_PATH
            ));
            validation.print_report();
            panic!("Cannot continue without fixtures directory");
        }

        let sample_files_path = fixtures_path.join(SAMPLE_FILES_DIR);
        let smart_folders_path = fixtures_path.join(SMART_FOLDERS_DIR);

        // Validate sample files directory
        if !sample_files_path.exists() {
            validation.add_issue(format!(
                "Sample files directory missing: {}",
                sample_files_path.display()
            ));
        } else {
            validate_sample_files(&sample_files_path, &mut validation);
        }

        // Validate smart folders directory
        if !smart_folders_path.exists() {
            validation.add_issue(format!(
                "Smart folders directory missing: {}",
                smart_folders_path.display()
            ));
        } else {
            validate_smart_folders(&smart_folders_path, &mut validation);
        }

        // Check for naming convention consistency
        validate_naming_conventions(&smart_folders_path, &mut validation);

        // Test file categorization
        test_file_categorization(&sample_files_path, &mut validation);

        validation.print_report();

        // Fail the test if there are critical issues
        assert!(
            validation.issues.is_empty(),
            "Test structure has critical issues"
        );
    }

    fn validate_sample_files(path: &Path, validation: &mut TestStructureValidation) {
        let files = match fs::read_dir(path) {
            Ok(entries) => entries,
            Err(e) => {
                validation.add_issue(format!("Cannot read sample files directory: {}", e));
                return;
            }
        };

        let mut file_count = 0;
        let mut file_types = HashMap::new();
        let mut file_sizes = Vec::new();

        for entry in files.flatten() {
            let path = entry.path();
            if path.is_file() {
                file_count += 1;

                // Check file extension
                if let Some(ext) = path.extension() {
                    *file_types
                        .entry(ext.to_string_lossy().to_string())
                        .or_insert(0) += 1;
                }

                // Check file size
                if let Ok(metadata) = entry.metadata() {
                    file_sizes.push(metadata.len());
                }

                // Check for specific test files
                let file_name = path.file_name().unwrap().to_string_lossy();

                // Validate 3D print files
                if file_name.ends_with(".stl")
                    || file_name.ends_with(".3mf")
                    || file_name.ends_with(".obj")
                    || file_name.ends_with(".gcode")
                    || file_name.ends_with(".scad")
                {
                    validation.add_info(format!("Found 3D print file: {}", file_name));
                }

                // Validate finance files
                if file_name.to_lowercase().contains("finance")
                    || file_name.to_lowercase().contains("invoice")
                    || file_name.to_lowercase().contains("financial")
                {
                    validation.add_info(format!("Found finance file: {}", file_name));
                }
            }
        }

        validation.add_info(format!("Total sample files: {}", file_count));
        validation.add_info(format!("File types: {:?}", file_types));

        // Check for minimum required files
        if file_count < 10 {
            validation.add_warning(format!(
                "Low number of sample files ({}), consider adding more for comprehensive testing",
                file_count
            ));
        }

        // Check for diverse file types
        if file_types.len() < 5 {
            validation.add_warning(format!(
                "Limited file type diversity ({} types), consider adding more file types",
                file_types.len()
            ));
        }

        // Check for 3D print file types
        let has_3d_files = file_types
            .keys()
            .any(|ext| ["stl", "3mf", "obj", "gcode", "scad"].contains(&ext.as_str()));
        if !has_3d_files {
            validation.add_warning("No 3D print files found in sample files (expected .stl, .3mf, .obj, .gcode, or .scad)".to_string());
        }
    }

    fn validate_smart_folders(path: &Path, validation: &mut TestStructureValidation) {
        let expected_folders = vec![
            ("sample_demo_smart_folder_research", "Research"),
            ("sample_demo_smart_folder_logos", "Logos or Graphic Art"),
            ("sample_demo_smart_folder_3D_print", "3D Print"),
            ("sample_demo_smart_folder_finance", "Finance"),
        ];

        for (folder_name, display_name) in expected_folders {
            let folder_path = path.join(folder_name);
            if !folder_path.exists() {
                validation.add_issue(format!(
                    "Missing smart folder: {} ({})",
                    folder_name, display_name
                ));
            } else if !folder_path.is_dir() {
                validation.add_issue(format!("{} exists but is not a directory", folder_name));
            } else {
                // Check if folder is empty (which is expected for smart folders)
                match fs::read_dir(&folder_path) {
                    Ok(entries) => {
                        let count = entries.count();
                        if count > 0 {
                            validation.add_warning(format!(
                                "Smart folder '{}' contains {} items (should be empty for testing)",
                                folder_name, count
                            ));
                        } else {
                            validation.add_info(format!(
                                "Smart folder '{}' is correctly empty",
                                display_name
                            ));
                        }
                    }
                    Err(e) => {
                        validation
                            .add_issue(format!("Cannot read smart folder {}: {}", folder_name, e));
                    }
                }
            }
        }

        // Check for the names/descriptions file
        let names_file = path.join("sample_demo_smart_folder_names_descriptions");
        if !names_file.exists() {
            validation.add_warning("Missing smart folder names/descriptions file".to_string());
        } else {
            validation.add_info("Smart folder names/descriptions file found".to_string());
        }
    }

    fn validate_naming_conventions(path: &Path, validation: &mut TestStructureValidation) {
        let entries = match fs::read_dir(path) {
            Ok(entries) => entries,
            Err(_) => return,
        };

        let mut folder_names = Vec::new();
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                folder_names.push(entry.file_name().to_string_lossy().to_string());
            }
        }

        // Check naming pattern consistency
        let expected_prefix = "sample_demo_smart_folder_";
        for name in &folder_names {
            if !name.starts_with(expected_prefix) {
                validation.add_issue(format!(
                    "Folder '{}' doesn't follow naming convention (should start with '{}')",
                    name, expected_prefix
                ));
            }
        }

        // Check for consistent use of underscores vs camelCase
        for name in &folder_names {
            if name.contains('-') {
                validation.add_warning(format!(
                    "Folder '{}' uses hyphens instead of underscores",
                    name
                ));
            }
        }
    }

    fn test_file_categorization(
        sample_files_path: &Path,
        validation: &mut TestStructureValidation,
    ) {
        // Define expected categorizations
        let categorizations = vec![
            ("3D Print", vec![".stl", ".3mf", ".obj", ".gcode", ".scad"]),
            (
                "Finance",
                vec!["finance", "financial", "invoice", "expense"],
            ),
            ("Research", vec!["research", "llm", "vlm", "ai"]),
            ("Logos", vec![".svg", ".eps", ".ai", ".psd"]),
        ];

        for (category, patterns) in categorizations {
            let mut found_files = Vec::new();

            if let Ok(entries) = fs::read_dir(sample_files_path) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file() {
                        let file_name = path.file_name().unwrap().to_string_lossy().to_lowercase();

                        for pattern in &patterns {
                            if pattern.starts_with('.') {
                                // Extension check
                                if file_name.ends_with(pattern) {
                                    found_files.push(file_name.clone());
                                    break;
                                }
                            } else {
                                // Name contains check
                                if file_name.contains(pattern) {
                                    found_files.push(file_name.clone());
                                    break;
                                }
                            }
                        }
                    }
                }
            }

            if found_files.is_empty() {
                validation.add_warning(format!("No test files found for '{}' category", category));
            } else {
                validation.add_info(format!(
                    "Found {} files for '{}' category: {:?}",
                    found_files.len(),
                    category,
                    found_files
                ));
            }
        }
    }

    #[test]
    fn test_file_movement_simulation() {
        let fixtures_path = Path::new(TEST_FIXTURES_PATH);
        let sample_files = fixtures_path.join(SAMPLE_FILES_DIR);
        let smart_folders = fixtures_path.join(SMART_FOLDERS_DIR);

        // Create a temporary test file
        let test_file_name = "test_movement_file.txt";
        let test_file_path = sample_files.join(test_file_name);

        // Write test file
        fs::write(&test_file_path, "Test content for file movement")
            .expect("Failed to create test file");

        // Simulate moving to different smart folders
        let folders_to_test = vec![
            "sample_demo_smart_folder_finance",
            "sample_demo_smart_folder_research",
            "sample_demo_smart_folder_3D_print",
        ];

        for folder in folders_to_test {
            let target_path = smart_folders.join(folder).join(test_file_name);

            // Simulate move (copy for testing)
            match fs::copy(&test_file_path, &target_path) {
                Ok(_) => {
                    println!("✅ Successfully simulated move to {}", folder);
                    // Clean up
                    fs::remove_file(&target_path).ok();
                }
                Err(e) => {
                    println!("❌ Failed to simulate move to {}: {}", folder, e);
                }
            }
        }

        // Clean up test file
        fs::remove_file(&test_file_path).ok();
    }

    #[test]
    fn test_reset_capability() {
        let fixtures_path = Path::new(TEST_FIXTURES_PATH);
        let smart_folders_path = fixtures_path.join(SMART_FOLDERS_DIR);

        // Check that all smart folders are empty (ready for testing)
        let folders = vec![
            "sample_demo_smart_folder_research",
            "sample_demo_smart_folder_logos",
            "sample_demo_smart_folder_3D_print",
            "sample_demo_smart_folder_finance",
        ];

        let mut all_empty = true;
        for folder_name in folders {
            let folder_path = smart_folders_path.join(folder_name);
            if let Ok(entries) = fs::read_dir(&folder_path) {
                let count = entries.count();
                if count > 0 {
                    println!(
                        "⚠️ Smart folder '{}' is not empty ({} items)",
                        folder_name, count
                    );
                    all_empty = false;
                }
            }
        }

        assert!(
            all_empty,
            "Smart folders should be empty for test reusability"
        );
        println!("✅ All smart folders are empty and ready for testing");
    }
}
