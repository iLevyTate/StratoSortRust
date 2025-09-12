#!/usr/bin/env python3
"""
File Analysis Script for StratoRust Testing

This script provides utilities for testing the file analysis functionality
of the StratoRust system. It includes functions for generating test data,
analyzing files, and validating results.
"""

import os
import sys
import json
import hashlib
import mimetypes
from pathlib import Path
from typing import List, Dict, Any, Optional
from datetime import datetime
import argparse

class FileAnalyzer:
    """Utility class for analyzing files and generating test data."""

    def __init__(self, output_dir: str = "./test_output"):
        self.output_dir = Path(output_dir)
        self.output_dir.mkdir(exist_ok=True)
        self.supported_extensions = {
            'documents': ['.txt', '.md', '.pdf', '.doc', '.docx', '.rtf'],
            'images': ['.jpg', '.jpeg', '.png', '.gif', '.bmp', '.svg'],
            'audio': ['.mp3', '.wav', '.flac', '.aac', '.ogg'],
            'video': ['.mp4', '.avi', '.mkv', '.mov', '.wmv'],
            'code': ['.py', '.rs', '.js', '.html', '.css', '.c', '.cpp'],
            'archives': ['.zip', '.tar', '.gz', '.rar', '.7z'],
            'data': ['.json', '.xml', '.csv', '.sql', '.yaml']
        }

    def analyze_file(self, file_path: str) -> Dict[str, Any]:
        """Analyze a single file and return metadata."""
        path = Path(file_path)

        if not path.exists():
            raise FileNotFoundError(f"File not found: {file_path}")

        stat = path.stat()
        mime_type, _ = mimetypes.guess_type(str(path))

        analysis = {
            'path': str(path.absolute()),
            'name': path.name,
            'size': stat.st_size,
            'extension': path.suffix.lower(),
            'mime_type': mime_type or 'unknown',
            'created': datetime.fromtimestamp(stat.st_ctime).isoformat(),
            'modified': datetime.fromtimestamp(stat.st_mtime).isoformat(),
            'category': self._categorize_file(path),
            'hash': self._calculate_hash(path),
            'is_text': self._is_text_file(path),
            'content_summary': None
        }

        # Add content analysis for text files
        if analysis['is_text'] and stat.st_size < 1024 * 1024:  # Max 1MB for text analysis
            try:
                analysis['content_summary'] = self._analyze_text_content(path)
            except Exception as e:
                analysis['content_summary'] = f"Error reading content: {str(e)}"

        return analysis

    def analyze_directory(self, directory: str, recursive: bool = True) -> Dict[str, Any]:
        """Analyze all files in a directory."""
        dir_path = Path(directory)

        if not dir_path.exists() or not dir_path.is_dir():
            raise ValueError(f"Directory not found: {directory}")

        files_analyzed = []
        errors = []
        category_stats = {}
        total_size = 0

        pattern = "**/*" if recursive else "*"

        for file_path in dir_path.glob(pattern):
            if file_path.is_file():
                try:
                    analysis = self.analyze_file(str(file_path))
                    files_analyzed.append(analysis)

                    # Update statistics
                    category = analysis['category']
                    category_stats[category] = category_stats.get(category, 0) + 1
                    total_size += analysis['size']

                except Exception as e:
                    errors.append({
                        'file': str(file_path),
                        'error': str(e)
                    })

        return {
            'directory': str(dir_path.absolute()),
            'total_files': len(files_analyzed),
            'total_size': total_size,
            'files': files_analyzed,
            'category_stats': category_stats,
            'errors': errors,
            'analyzed_at': datetime.now().isoformat()
        }

    def generate_test_report(self, analysis: Dict[str, Any], output_file: str = None) -> str:
        """Generate a formatted test report from analysis results."""
        if output_file is None:
            output_file = self.output_dir / f"analysis_report_{datetime.now().strftime('%Y%m%d_%H%M%S')}.json"
        else:
            output_file = Path(output_file)

        with open(output_file, 'w', encoding='utf-8') as f:
            json.dump(analysis, f, indent=2, ensure_ascii=False)

        print(f"Analysis report saved to: {output_file}")
        return str(output_file)

    def create_test_dataset(self, base_dir: str, num_files: int = 50) -> List[str]:
        """Create a test dataset with various file types."""
        base_path = Path(base_dir)
        base_path.mkdir(exist_ok=True)

        created_files = []

        # Create different categories of test files
        categories = {
            'documents': ['meeting_notes.txt', 'project_plan.md', 'specification.pdf'],
            'images': ['photo.jpg', 'diagram.png', 'icon.svg'],
            'code': ['main.py', 'utils.rs', 'styles.css'],
            'data': ['config.json', 'data.csv', 'schema.xml']
        }

        for category, filenames in categories.items():
            cat_dir = base_path / category
            cat_dir.mkdir(exist_ok=True)

            for filename in filenames:
                file_path = cat_dir / filename
                content = self._generate_content_for_type(category, filename)

                with open(file_path, 'w', encoding='utf-8') as f:
                    f.write(content)

                created_files.append(str(file_path))

        return created_files

    def _categorize_file(self, path: Path) -> str:
        """Categorize a file based on its extension."""
        extension = path.suffix.lower()

        for category, extensions in self.supported_extensions.items():
            if extension in extensions:
                return category

        return 'unknown'

    def _calculate_hash(self, path: Path) -> str:
        """Calculate SHA-256 hash of a file."""
        hash_sha256 = hashlib.sha256()
        try:
            with open(path, "rb") as f:
                for chunk in iter(lambda: f.read(4096), b""):
                    hash_sha256.update(chunk)
            return hash_sha256.hexdigest()
        except Exception:
            return "error_calculating_hash"

    def _is_text_file(self, path: Path) -> bool:
        """Determine if a file is likely to be a text file."""
        text_extensions = ['.txt', '.md', '.py', '.rs', '.js', '.html', '.css', '.json', '.xml', '.csv']

        if path.suffix.lower() in text_extensions:
            return True

        try:
            with open(path, 'rb') as f:
                chunk = f.read(1024)
                # Check for null bytes (indicator of binary file)
                return b'\x00' not in chunk
        except Exception:
            return False

    def _analyze_text_content(self, path: Path) -> Dict[str, Any]:
        """Analyze the content of a text file."""
        try:
            with open(path, 'r', encoding='utf-8', errors='ignore') as f:
                content = f.read()
        except Exception as e:
            return {'error': str(e)}

        lines = content.splitlines()
        words = content.split()

        # Find common keywords
        keywords = ['TODO', 'FIXME', 'NOTE', 'BUG', 'HACK', 'IMPORTANT']
        keyword_counts = {kw: content.upper().count(kw) for kw in keywords}
        keyword_counts = {k: v for k, v in keyword_counts.items() if v > 0}

        return {
            'line_count': len(lines),
            'word_count': len(words),
            'char_count': len(content),
            'keywords': keyword_counts,
            'encoding': 'utf-8',
            'first_line': lines[0] if lines else '',
            'language_hints': self._detect_language_hints(path, content)
        }

    def _detect_language_hints(self, path: Path, content: str) -> List[str]:
        """Detect programming language or document type hints."""
        hints = []

        # File extension hints
        ext_hints = {
            '.py': ['python'],
            '.rs': ['rust'],
            '.js': ['javascript'],
            '.html': ['html'],
            '.md': ['markdown'],
            '.json': ['json'],
            '.xml': ['xml']
        }

        if path.suffix.lower() in ext_hints:
            hints.extend(ext_hints[path.suffix.lower()])

        # Content-based hints
        if 'def ' in content and 'import ' in content:
            hints.append('python')
        if 'fn main()' in content or 'use std::' in content:
            hints.append('rust')
        if '#!/usr/bin/env python' in content:
            hints.append('python_script')
        if '<?xml' in content:
            hints.append('xml')
        if content.strip().startswith('{') and content.strip().endswith('}'):
            hints.append('json_like')

        return list(set(hints))  # Remove duplicates

    def _generate_content_for_type(self, category: str, filename: str) -> str:
        """Generate appropriate content for different file types."""
        if category == 'documents':
            if filename.endswith('.txt'):
                return f"This is a test document: {filename}\n\nIt contains sample text for testing purposes.\nCreated at: {datetime.now().isoformat()}"
            elif filename.endswith('.md'):
                return f"# Test Document: {filename}\n\nThis is a **markdown** document for testing.\n\n- Item 1\n- Item 2\n- Item 3"
            else:
                return f"Test content for {filename}"

        elif category == 'code':
            if filename.endswith('.py'):
                return '''#!/usr/bin/env python3
def hello_world():
    """A simple test function."""
    print("Hello, World!")

if __name__ == "__main__":
    hello_world()
'''
            elif filename.endswith('.rs'):
                return '''fn main() {
    println!("Hello, World!");
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_hello() {
        assert_eq!(2 + 2, 4);
    }
}
'''
            else:
                return f"/* Test code file: {filename} */"

        elif category == 'data':
            if filename.endswith('.json'):
                return json.dumps({
                    "name": "test_data",
                    "version": "1.0",
                    "files": [filename],
                    "created": datetime.now().isoformat()
                }, indent=2)
            elif filename.endswith('.csv'):
                return "id,name,category,size\n1,file1.txt,document,1024\n2,image.jpg,image,2048"
            else:
                return f"Test data for {filename}"

        else:
            return f"Test content for {filename} in category {category}"


def main():
    """Main function for command-line usage."""
    parser = argparse.ArgumentParser(description="File Analysis Testing Utility")
    parser.add_argument("command", choices=['analyze', 'create_dataset', 'analyze_dir'],
                       help="Command to execute")
    parser.add_argument("path", help="File or directory path")
    parser.add_argument("--output", "-o", help="Output file for results")
    parser.add_argument("--recursive", "-r", action="store_true",
                       help="Recursive directory analysis")

    args = parser.parse_args()

    analyzer = FileAnalyzer()

    if args.command == 'analyze':
        try:
            result = analyzer.analyze_file(args.path)
            if args.output:
                with open(args.output, 'w') as f:
                    json.dump(result, f, indent=2)
                print(f"Analysis saved to: {args.output}")
            else:
                print(json.dumps(result, indent=2))
        except Exception as e:
            print(f"Error analyzing file: {e}", file=sys.stderr)
            sys.exit(1)

    elif args.command == 'analyze_dir':
        try:
            result = analyzer.analyze_directory(args.path, args.recursive)
            output_file = analyzer.generate_test_report(result, args.output)
            print(f"Directory analysis completed. Report: {output_file}")
        except Exception as e:
            print(f"Error analyzing directory: {e}", file=sys.stderr)
            sys.exit(1)

    elif args.command == 'create_dataset':
        try:
            created_files = analyzer.create_test_dataset(args.path)
            print(f"Created test dataset with {len(created_files)} files in: {args.path}")
            for file_path in created_files:
                print(f"  {file_path}")
        except Exception as e:
            print(f"Error creating dataset: {e}", file=sys.stderr)
            sys.exit(1)


if __name__ == "__main__":
    main()