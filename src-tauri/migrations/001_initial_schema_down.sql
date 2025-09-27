-- Rollback script for initial schema
-- Version: 1
-- WARNING: This will delete all data!

-- Drop all indexes first
DROP INDEX IF EXISTS idx_files_path;
DROP INDEX IF EXISTS idx_files_name;
DROP INDEX IF EXISTS idx_files_extension;
DROP INDEX IF EXISTS idx_files_size;
DROP INDEX IF EXISTS idx_files_modified;
DROP INDEX IF EXISTS idx_files_parent;

DROP INDEX IF EXISTS idx_folders_path;
DROP INDEX IF EXISTS idx_folders_parent;

DROP INDEX IF EXISTS idx_org_history_file;
DROP INDEX IF EXISTS idx_org_history_date;

DROP INDEX IF EXISTS idx_documents_file;

-- Drop all tables
DROP TABLE IF EXISTS user_patterns;
DROP TABLE IF EXISTS settings;
DROP TABLE IF EXISTS documents;
DROP TABLE IF EXISTS organization_history;
DROP TABLE IF EXISTS folders;
DROP TABLE IF EXISTS files;