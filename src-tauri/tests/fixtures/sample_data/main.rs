use std::env;
use std::fs;
use std::path::Path;

/// Main entry point for the file organization CLI tool
fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    env_logger::init();
    
    // Parse command line arguments
    let args: Vec<String> = env::args().collect();
    
    if args.len() < 2 {
        print_usage();
        return Ok(());
    }
    
    let command = &args[1];
    
    match command.as_str() {
        "analyze" => {
            if args.len() < 3 {
                eprintln!("Error: analyze command requires a file path");
                return Ok(());
            }
            analyze_file(&args[2])?;
        }
        "organize" => {
            if args.len() < 3 {
                eprintln!("Error: organize command requires a directory path");
                return Ok(());
            }
            organize_directory(&args[2])?;
        }
        "search" => {
            if args.len() < 3 {
                eprintln!("Error: search command requires a query");
                return Ok(());
            }
            search_files(&args[2])?;
        }
        "help" => {
            print_usage();
        }
        _ => {
            eprintln!("Error: Unknown command '{}'", command);
            print_usage();
        }
    }
    
    Ok(())
}

/// Print usage information
fn print_usage() {
    println!("File Organization Tool");
    println!("Usage: fileorg <command> [arguments]");
    println!("");
    println!("Commands:");
    println!("  analyze <file>     Analyze a single file");
    println!("  organize <dir>     Organize files in a directory");
    println!("  search <query>     Search for files by content");
    println!("  help              Show this help message");
    println!("");
    println!("Examples:");
    println!("  fileorg analyze document.pdf");
    println!("  fileorg organize ~/Downloads");
    println!("  fileorg search \"project documentation\"");
}

/// Analyze a single file
fn analyze_file(file_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let path = Path::new(file_path);
    
    if !path.exists() {
        eprintln!("Error: File '{}' does not exist", file_path);
        return Ok(());
    }
    
    println!("Analyzing file: {}", file_path);
    
    // Get file metadata
    let metadata = fs::metadata(path)?;
    let file_size = metadata.len();
    let is_dir = metadata.is_dir();
    
    println!("File size: {} bytes", file_size);
    println!("Is directory: {}", is_dir);
    
    // Determine file type
    if let Some(extension) = path.extension() {
        println!("File extension: {}", extension.to_string_lossy());
        
        let file_type = match extension.to_string_lossy().to_lowercase().as_str() {
            "txt" | "md" | "rst" => "Document",
            "pdf" | "doc" | "docx" => "Document",
            "jpg" | "jpeg" | "png" | "gif" => "Image",
            "mp3" | "wav" | "flac" => "Audio",
            "mp4" | "avi" | "mkv" => "Video",
            "rs" | "py" | "js" | "c" | "cpp" => "Code",
            "zip" | "tar" | "gz" => "Archive",
            _ => "Unknown",
        };
        
        println!("File type: {}", file_type);
    } else {
        println!("No file extension detected");
    }
    
    // If it's a text file, analyze content
    if is_text_file(path) {
        analyze_text_content(path)?;
    }
    
    Ok(())
}

/// Organize files in a directory
fn organize_directory(dir_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let path = Path::new(dir_path);
    
    if !path.exists() || !path.is_dir() {
        eprintln!("Error: Directory '{}' does not exist or is not a directory", dir_path);
        return Ok(());
    }
    
    println!("Organizing directory: {}", dir_path);
    
    // Read directory contents
    let entries = fs::read_dir(path)?;
    let mut file_count = 0;
    let mut dir_count = 0;
    
    for entry in entries {
        let entry = entry?;
        let entry_path = entry.path();
        
        if entry_path.is_dir() {
            dir_count += 1;
            println!("Directory: {}", entry_path.display());
        } else {
            file_count += 1;
            println!("File: {}", entry_path.display());
            
            // Suggest organization
            if let Some(suggested_folder) = suggest_folder(&entry_path) {
                println!("  → Suggested folder: {}", suggested_folder);
            }
        }
    }
    
    println!("Found {} files and {} directories", file_count, dir_count);
    Ok(())
}

/// Search for files by content
fn search_files(query: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("Searching for files containing: '{}'", query);
    
    // This would integrate with the AI search functionality
    // For now, just simulate some results
    println!("Search results:");
    println!("  📄 document.txt - Contains relevant information about {}", query);
    println!("  📊 data.csv - Has entries matching {}", query);
    println!("  📝 notes.md - Mentions {} in context", query);
    
    Ok(())
}

/// Check if a file is likely to be a text file
fn is_text_file(path: &Path) -> bool {
    if let Some(extension) = path.extension() {
        match extension.to_string_lossy().to_lowercase().as_str() {
            "txt" | "md" | "rst" | "log" | "json" | "xml" | "csv" => true,
            "rs" | "py" | "js" | "html" | "css" | "c" | "cpp" | "h" => true,
            _ => false,
        }
    } else {
        false
    }
}

/// Analyze text content of a file
fn analyze_text_content(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let content = fs::read_to_string(path)?;
    let line_count = content.lines().count();
    let word_count = content.split_whitespace().count();
    let char_count = content.chars().count();
    
    println!("Text analysis:");
    println!("  Lines: {}", line_count);
    println!("  Words: {}", word_count);
    println!("  Characters: {}", char_count);
    
    // Look for common keywords
    let keywords = vec!["TODO", "FIXME", "NOTE", "IMPORTANT", "BUG"];
    for keyword in keywords {
        let count = content.matches(keyword).count();
        if count > 0 {
            println!("  Found '{}': {} times", keyword, count);
        }
    }
    
    Ok(())
}

/// Suggest a folder for organizing a file
fn suggest_folder(path: &Path) -> Option<String> {
    if let Some(extension) = path.extension() {
        match extension.to_string_lossy().to_lowercase().as_str() {
            "txt" | "md" | "pdf" | "doc" | "docx" => Some("Documents".to_string()),
            "jpg" | "jpeg" | "png" | "gif" | "bmp" => Some("Images".to_string()),
            "mp3" | "wav" | "flac" | "aac" => Some("Music".to_string()),
            "mp4" | "avi" | "mkv" | "mov" => Some("Videos".to_string()),
            "rs" | "py" | "js" | "c" | "cpp" | "h" => Some("Code".to_string()),
            "zip" | "tar" | "gz" | "rar" => Some("Archives".to_string()),
            _ => None,
        }
    } else {
        None
    }
}