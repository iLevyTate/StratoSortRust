"""
Utility functions for file analysis and testing.

This module provides common utilities used across the testing framework
for the StratoRust file organization system.
"""

import os
import json
import time
import random
import string
import tempfile
from pathlib import Path
from typing import List, Dict, Any, Tuple, Optional
from contextlib import contextmanager

def generate_random_string(length: int = 10, charset: str = None) -> str:
    """Generate a random string of specified length."""
    if charset is None:
        charset = string.ascii_letters + string.digits
    return ''.join(random.choice(charset) for _ in range(length))

def generate_random_filename(extension: str = None) -> str:
    """Generate a random filename with optional extension."""
    base_name = generate_random_string(8)
    if extension:
        if not extension.startswith('.'):
            extension = '.' + extension
        return base_name + extension
    return base_name

def create_temp_file(content: str = None, suffix: str = None, directory: str = None) -> str:
    """Create a temporary file with optional content."""
    with tempfile.NamedTemporaryFile(mode='w', suffix=suffix, dir=directory, delete=False) as f:
        if content:
            f.write(content)
        return f.name

def create_temp_directory(prefix: str = "test_") -> str:
    """Create a temporary directory."""
    return tempfile.mkdtemp(prefix=prefix)

@contextmanager
def temporary_directory():
    """Context manager for temporary directory that cleans up automatically."""
    temp_dir = create_temp_directory()
    try:
        yield temp_dir
    finally:
        import shutil
        shutil.rmtree(temp_dir, ignore_errors=True)

def file_size_human_readable(size_bytes: int) -> str:
    """Convert file size to human readable format."""
    if size_bytes == 0:
        return "0 B"

    size_names = ["B", "KB", "MB", "GB", "TB"]
    i = 0
    while size_bytes >= 1024.0 and i < len(size_names) - 1:
        size_bytes /= 1024.0
        i += 1

    return f"{size_bytes:.1f} {size_names[i]}"

def calculate_directory_size(directory: str) -> int:
    """Calculate total size of all files in a directory."""
    total_size = 0
    for dirpath, dirnames, filenames in os.walk(directory):
        for filename in filenames:
            file_path = os.path.join(dirpath, filename)
            try:
                total_size += os.path.getsize(file_path)
            except (OSError, IOError):
                pass  # Skip files that can't be accessed
    return total_size

def get_file_extension(filename: str) -> str:
    """Get the file extension (without the dot)."""
    return Path(filename).suffix.lstrip('.').lower()

def is_binary_file(file_path: str, chunk_size: int = 1024) -> bool:
    """Check if a file is binary by looking for null bytes."""
    try:
        with open(file_path, 'rb') as f:
            chunk = f.read(chunk_size)
            return b'\x00' in chunk
    except (OSError, IOError):
        return True  # If we can't read it, assume it's binary

def count_lines_in_file(file_path: str) -> int:
    """Count the number of lines in a text file."""
    try:
        with open(file_path, 'r', encoding='utf-8', errors='ignore') as f:
            return sum(1 for _ in f)
    except (OSError, IOError):
        return 0

def get_common_words(text: str, min_length: int = 3, max_words: int = 10) -> List[Tuple[str, int]]:
    """Get the most common words from text."""
    import re
    from collections import Counter

    # Extract words (alphanumeric sequences)
    words = re.findall(r'\b[a-zA-Z]{' + str(min_length) + r',}\b', text.lower())

    # Count word frequencies
    word_counts = Counter(words)

    # Return top words
    return word_counts.most_common(max_words)

def similarity_score(text1: str, text2: str) -> float:
    """Calculate simple similarity score between two texts."""
    if not text1 or not text2:
        return 0.0

    # Convert to sets of words
    words1 = set(text1.lower().split())
    words2 = set(text2.lower().split())

    # Calculate Jaccard similarity
    intersection = words1.intersection(words2)
    union = words1.union(words2)

    if not union:
        return 0.0

    return len(intersection) / len(union)

def benchmark_function(func, *args, **kwargs) -> Tuple[Any, float]:
    """Benchmark a function and return result and execution time."""
    start_time = time.time()
    result = func(*args, **kwargs)
    end_time = time.time()
    return result, end_time - start_time

class FileSystemMock:
    """Mock file system for testing purposes."""

    def __init__(self):
        self.files: Dict[str, str] = {}
        self.directories: set = set()

    def create_file(self, path: str, content: str = "") -> None:
        """Create a mock file."""
        self.files[path] = content
        # Ensure parent directories exist
        parent = str(Path(path).parent)
        if parent != '.':
            self.directories.add(parent)

    def create_directory(self, path: str) -> None:
        """Create a mock directory."""
        self.directories.add(path)

    def file_exists(self, path: str) -> bool:
        """Check if a mock file exists."""
        return path in self.files

    def directory_exists(self, path: str) -> bool:
        """Check if a mock directory exists."""
        return path in self.directories

    def get_file_content(self, path: str) -> str:
        """Get content of a mock file."""
        return self.files.get(path, "")

    def list_files(self) -> List[str]:
        """List all mock files."""
        return list(self.files.keys())

    def list_directories(self) -> List[str]:
        """List all mock directories."""
        return list(self.directories)

class TestDataGenerator:
    """Generate test data for various scenarios."""

    @staticmethod
    def generate_text_content(paragraph_count: int = 3, sentences_per_paragraph: int = 4) -> str:
        """Generate lorem ipsum-style text content."""
        lorem_words = [
            "lorem", "ipsum", "dolor", "sit", "amet", "consectetur", "adipiscing", "elit",
            "sed", "do", "eiusmod", "tempor", "incididunt", "ut", "labore", "et", "dolore",
            "magna", "aliqua", "enim", "ad", "minim", "veniam", "quis", "nostrud",
            "exercitation", "ullamco", "laboris", "nisi", "aliquip", "ex", "ea", "commodo",
            "consequat", "duis", "aute", "irure", "in", "reprehenderit", "voluptate",
            "velit", "esse", "cillum", "fugiat", "nulla", "pariatur", "excepteur", "sint",
            "occaecat", "cupidatat", "non", "proident", "sunt", "culpa", "qui", "officia"
        ]

        paragraphs = []
        for _ in range(paragraph_count):
            sentences = []
            for _ in range(sentences_per_paragraph):
                sentence_length = random.randint(5, 15)
                words = random.choices(lorem_words, k=sentence_length)
                sentence = ' '.join(words).capitalize() + '.'
                sentences.append(sentence)
            paragraphs.append(' '.join(sentences))

        return '\n\n'.join(paragraphs)

    @staticmethod
    def generate_code_content(language: str = "python") -> str:
        """Generate sample code content."""
        templates = {
            "python": '''#!/usr/bin/env python3
"""Sample Python module for testing."""

import os
import sys
from typing import List, Dict, Any

def process_data(data: List[Dict[str, Any]]) -> Dict[str, int]:
    """Process input data and return summary."""
    result = {}
    for item in data:
        category = item.get('category', 'unknown')
        result[category] = result.get(category, 0) + 1
    return result

def main():
    """Main function."""
    test_data = [
        {'category': 'documents', 'name': 'file1.txt'},
        {'category': 'images', 'name': 'photo.jpg'},
        {'category': 'documents', 'name': 'report.pdf'}
    ]

    summary = process_data(test_data)
    print("Processing summary:", summary)

if __name__ == "__main__":
    main()
''',
            "rust": '''use std::collections::HashMap;

/// Sample Rust module for testing
pub struct DataProcessor {
    categories: HashMap<String, usize>,
}

impl DataProcessor {
    pub fn new() -> Self {
        Self {
            categories: HashMap::new(),
        }
    }

    pub fn process_item(&mut self, category: &str) {
        *self.categories.entry(category.to_string()).or_insert(0) += 1;
    }

    pub fn get_summary(&self) -> &HashMap<String, usize> {
        &self.categories
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_data_processor() {
        let mut processor = DataProcessor::new();
        processor.process_item("documents");
        processor.process_item("images");
        processor.process_item("documents");

        let summary = processor.get_summary();
        assert_eq!(summary.get("documents"), Some(&2));
        assert_eq!(summary.get("images"), Some(&1));
    }
}
''',
            "javascript": '''/**
 * Sample JavaScript module for testing
 */

class DataProcessor {
    constructor() {
        this.categories = new Map();
    }

    processItem(category) {
        const count = this.categories.get(category) || 0;
        this.categories.set(category, count + 1);
    }

    getSummary() {
        return Object.fromEntries(this.categories);
    }
}

function main() {
    const processor = new DataProcessor();

    const testData = [
        { category: 'documents', name: 'file1.txt' },
        { category: 'images', name: 'photo.jpg' },
        { category: 'documents', name: 'report.pdf' }
    ];

    testData.forEach(item => {
        processor.processItem(item.category);
    });

    console.log('Processing summary:', processor.getSummary());
}

// Export for testing
if (typeof module !== 'undefined' && module.exports) {
    module.exports = { DataProcessor };
}
'''
        }

        return templates.get(language, f"// Sample code for {language}")

    @staticmethod
    def generate_json_config() -> str:
        """Generate sample JSON configuration."""
        config = {
            "version": "1.0.0",
            "settings": {
                "theme": "dark",
                "language": "en",
                "auto_organize": True,
                "file_types": {
                    "documents": [".txt", ".pdf", ".doc", ".docx"],
                    "images": [".jpg", ".png", ".gif", ".svg"],
                    "code": [".py", ".rs", ".js", ".html", ".css"]
                }
            },
            "ai": {
                "model": "llama3.2:3b",
                "max_file_size": 104857600,
                "enable_vision": True,
                "confidence_threshold": 0.8
            },
            "performance": {
                "max_concurrent_operations": 4,
                "cache_size_mb": 100,
                "enable_indexing": True
            }
        }
        return json.dumps(config, indent=2)

def run_performance_test(test_func, iterations: int = 10) -> Dict[str, float]:
    """Run a performance test and return statistics."""
    times = []

    for _ in range(iterations):
        _, duration = benchmark_function(test_func)
        times.append(duration)

    return {
        'min': min(times),
        'max': max(times),
        'avg': sum(times) / len(times),
        'total': sum(times),
        'iterations': iterations
    }

def validate_test_results(expected: Dict[str, Any], actual: Dict[str, Any], tolerance: float = 0.01) -> List[str]:
    """Validate test results and return list of mismatches."""
    errors = []

    for key, expected_value in expected.items():
        if key not in actual:
            errors.append(f"Missing key: {key}")
            continue

        actual_value = actual[key]

        if isinstance(expected_value, (int, float)) and isinstance(actual_value, (int, float)):
            if abs(expected_value - actual_value) > tolerance:
                errors.append(f"Value mismatch for {key}: expected {expected_value}, got {actual_value}")
        elif expected_value != actual_value:
            errors.append(f"Value mismatch for {key}: expected {expected_value}, got {actual_value}")

    return errors

# Test utilities
def assert_file_exists(file_path: str) -> None:
    """Assert that a file exists."""
    if not os.path.exists(file_path):
        raise AssertionError(f"File does not exist: {file_path}")

def assert_file_content_contains(file_path: str, content: str) -> None:
    """Assert that a file contains specific content."""
    assert_file_exists(file_path)
    with open(file_path, 'r', encoding='utf-8') as f:
        file_content = f.read()
    if content not in file_content:
        raise AssertionError(f"File {file_path} does not contain: {content}")

def assert_directory_contains_files(directory: str, filenames: List[str]) -> None:
    """Assert that a directory contains specific files."""
    if not os.path.isdir(directory):
        raise AssertionError(f"Directory does not exist: {directory}")

    actual_files = set(os.listdir(directory))
    missing_files = [f for f in filenames if f not in actual_files]

    if missing_files:
        raise AssertionError(f"Directory {directory} missing files: {missing_files}")