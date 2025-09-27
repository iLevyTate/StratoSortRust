-- Initial database schema for StratoSort
-- Version: 1
-- Description: Core tables for file organization system

-- Files table
CREATE TABLE IF NOT EXISTS files (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    path TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    extension TEXT,
    size INTEGER NOT NULL,
    created_at TEXT NOT NULL,
    modified_at TEXT NOT NULL,
    accessed_at TEXT,
    is_directory BOOLEAN DEFAULT 0,
    parent_directory TEXT,
    mime_type TEXT,
    checksum TEXT,
    metadata TEXT,
    tags TEXT,
    indexed_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Folders table
CREATE TABLE IF NOT EXISTS folders (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    path TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    parent_id INTEGER,
    created_at TEXT NOT NULL,
    modified_at TEXT NOT NULL,
    total_size INTEGER DEFAULT 0,
    file_count INTEGER DEFAULT 0,
    subfolder_count INTEGER DEFAULT 0,
    depth INTEGER DEFAULT 0,
    metadata TEXT,
    FOREIGN KEY (parent_id) REFERENCES folders(id) ON DELETE CASCADE
);

-- Organization history
CREATE TABLE IF NOT EXISTS organization_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    file_id INTEGER NOT NULL,
    source_path TEXT NOT NULL,
    destination_path TEXT NOT NULL,
    operation_type TEXT NOT NULL,
    organized_at TEXT NOT NULL,
    organization_rule TEXT,
    success BOOLEAN DEFAULT 1,
    error_message TEXT,
    metadata TEXT,
    FOREIGN KEY (file_id) REFERENCES files(id) ON DELETE CASCADE
);

-- Documents table for content indexing
CREATE TABLE IF NOT EXISTS documents (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    file_id INTEGER NOT NULL UNIQUE,
    content TEXT,
    extracted_at TEXT NOT NULL,
    word_count INTEGER,
    language TEXT,
    summary TEXT,
    keywords TEXT,
    FOREIGN KEY (file_id) REFERENCES files(id) ON DELETE CASCADE
);

-- Settings table
CREATE TABLE IF NOT EXISTS settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    category TEXT,
    description TEXT,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- User patterns for organization
CREATE TABLE IF NOT EXISTS user_patterns (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    pattern TEXT NOT NULL,
    action TEXT NOT NULL,
    confidence REAL DEFAULT 0.5,
    usage_count INTEGER DEFAULT 0,
    created_at TEXT NOT NULL,
    last_used_at TEXT,
    is_active BOOLEAN DEFAULT 1
);

-- Create indexes for performance
CREATE INDEX IF NOT EXISTS idx_files_path ON files(path);
CREATE INDEX IF NOT EXISTS idx_files_name ON files(name);
CREATE INDEX IF NOT EXISTS idx_files_extension ON files(extension);
CREATE INDEX IF NOT EXISTS idx_files_size ON files(size);
CREATE INDEX IF NOT EXISTS idx_files_modified ON files(modified_at);
CREATE INDEX IF NOT EXISTS idx_files_parent ON files(parent_directory);

CREATE INDEX IF NOT EXISTS idx_folders_path ON folders(path);
CREATE INDEX IF NOT EXISTS idx_folders_parent ON folders(parent_id);

CREATE INDEX IF NOT EXISTS idx_org_history_file ON organization_history(file_id);
CREATE INDEX IF NOT EXISTS idx_org_history_date ON organization_history(organized_at);

CREATE INDEX IF NOT EXISTS idx_documents_file ON documents(file_id);

-- Insert default settings
INSERT OR IGNORE INTO settings (key, value, category, description)
VALUES
    ('auto_organize', 'false', 'organization', 'Enable automatic file organization'),
    ('watch_folders', '[]', 'organization', 'Folders to watch for changes'),
    ('organization_rules', '{}', 'organization', 'Custom organization rules'),
    ('theme', 'system', 'appearance', 'Application theme'),
    ('language', 'en', 'general', 'Application language'),
    ('show_hidden_files', 'false', 'files', 'Show hidden files in listings'),
    ('file_preview', 'true', 'files', 'Enable file preview'),
    ('ai_provider', 'ollama', 'ai', 'AI service provider'),
    ('ollama_model', 'llama2', 'ai', 'Ollama model to use'),
    ('embeddings_enabled', 'true', 'ai', 'Enable semantic search with embeddings');