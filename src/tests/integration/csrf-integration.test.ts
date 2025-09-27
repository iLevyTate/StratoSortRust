import { describe, it, expect, vi, beforeEach } from 'vitest';
import * as tauriApi from '$lib/api/tauri';

// Mock the underlying Tauri invoke
vi.mock('@tauri-apps/api/core', () => ({
    invoke: vi.fn()
}));

describe('CSRF Integration - Frontend to Backend', () => {
    let mockInvoke: any;

    beforeEach(async () => {
        vi.clearAllMocks();
        const tauriCore = await import('@tauri-apps/api/core');
        mockInvoke = vi.mocked(tauriCore.invoke);
    });

    describe('File Operations with CSRF', () => {
        it('should include CSRF token in scanDirectory calls', async () => {
            mockInvoke.mockResolvedValue([
                { name: 'file1.txt', path: '/path/file1.txt', size: 100 }
            ]);

            await tauriApi.scanDirectory('/test/path', true);

            expect(mockInvoke).toHaveBeenCalledWith(
                'scan_directory',
                expect.objectContaining({
                    path: '/test/path',
                    recursive: true,
                    __csrf_token: expect.any(String)
                })
            );
        });

        it('should include CSRF token in analyzeFile calls', async () => {
            mockInvoke.mockResolvedValue({
                file_type: 'text',
                content_preview: 'test content'
            });

            await tauriApi.analyzeFile('/test/file.txt');

            // The analyzeFile function calls multiple commands
            expect(mockInvoke).toHaveBeenCalledWith(
                'get_file_info_command',
                expect.objectContaining({
                    path: '/test/file.txt',
                    __csrf_token: expect.any(String)
                })
            );
        });
    });

    describe('Organization Operations with CSRF', () => {
        it('should include CSRF token in suggestFileOrganization calls', async () => {
            mockInvoke.mockResolvedValue([
                { source: '/old/path', destination: '/new/path', confidence: 0.9 }
            ]);

            await tauriApi.suggestFileOrganization(['/file1.txt', '/file2.txt']);

            expect(mockInvoke).toHaveBeenCalledWith(
                'suggest_file_organization',
                expect.objectContaining({
                    paths: ['/file1.txt', '/file2.txt'],
                    __csrf_token: expect.any(String)
                })
            );
        });

        it('should include CSRF token in applyOrganization calls', async () => {
            mockInvoke.mockResolvedValue({
                successful: 1,
                failed: 0,
                errors: []
            });

            const operations = [
                { source: '/old/file.txt', destination: '/new/file.txt' }
            ];

            await tauriApi.applyOrganization(operations);

            expect(mockInvoke).toHaveBeenCalledWith(
                'apply_organization',
                expect.objectContaining({
                    operations,
                    __csrf_token: expect.any(String)
                })
            );
        });
    });

    describe('AI Operations with CSRF', () => {
        it('should include CSRF token in checkOllamaStatus calls', async () => {
            mockInvoke.mockResolvedValue({
                available: true,
                version: '0.1.0',
                models: ['llama2']
            });

            await tauriApi.checkOllamaStatus();

            expect(mockInvoke).toHaveBeenCalledWith(
                'check_ollama_status',
                expect.objectContaining({
                    __csrf_token: expect.any(String)
                })
            );
        });

        it('should include CSRF token in connectOllama calls', async () => {
            mockInvoke.mockResolvedValue({
                connected: true,
                model_name: 'llama2'
            });

            await tauriApi.connectOllama('http://localhost:11434');

            expect(mockInvoke).toHaveBeenCalledWith(
                'connect_ollama',
                expect.objectContaining({
                    host: 'http://localhost:11434',
                    __csrf_token: expect.any(String)
                })
            );
        });
    });

    describe('Settings Operations with CSRF', () => {
        it('should include CSRF token in getAppSettings calls', async () => {
            mockInvoke.mockResolvedValue({
                theme: 'dark',
                language: 'en'
            });

            await tauriApi.getAppSettings();

            expect(mockInvoke).toHaveBeenCalledWith(
                'get_app_settings',
                expect.objectContaining({
                    __csrf_token: expect.any(String)
                })
            );
        });

        it('should include CSRF token in updateSettings calls', async () => {
            mockInvoke.mockResolvedValue({ success: true });

            await tauriApi.updateSettings({ theme: 'light' });

            expect(mockInvoke).toHaveBeenCalledWith(
                'update_settings',
                expect.objectContaining({
                    settings: { theme: 'light' },
                    __csrf_token: expect.any(String)
                })
            );
        });
    });

    describe('Search Operations with CSRF', () => {
        it('should include CSRF token in semanticSearch calls', async () => {
            mockInvoke.mockResolvedValue([
                { path: '/file.txt', relevance: 0.95, snippet: 'test' }
            ]);

            await tauriApi.semanticSearch('test query', 10);

            expect(mockInvoke).toHaveBeenCalledWith(
                'semantic_search',
                expect.objectContaining({
                    query: 'test query',
                    limit: 10,
                    __csrf_token: expect.any(String)
                })
            );
        });

        it('should include CSRF token in quickSearch calls', async () => {
            mockInvoke.mockResolvedValue([
                { path: '/file.txt', relevance: 0.8 }
            ]);

            await tauriApi.quickSearch('test');

            expect(mockInvoke).toHaveBeenCalledWith(
                'quick_search',
                expect.objectContaining({
                    query: 'test',
                    __csrf_token: expect.any(String)
                })
            );
        });
    });

    describe('Error Handling with CSRF', () => {
        it('should handle CSRF token validation errors', async () => {
            // First call fails with CSRF error, second succeeds
            mockInvoke
                .mockRejectedValueOnce(new Error('CSRF token validation failed'))
                .mockResolvedValueOnce({ success: true });

            await tauriApi.scanDirectory('/test');

            // Should retry once
            expect(mockInvoke).toHaveBeenCalledTimes(2);

            // Both calls should include CSRF token
            expect(mockInvoke).toHaveBeenNthCalledWith(1,
                'scan_directory',
                expect.objectContaining({
                    __csrf_token: expect.any(String)
                })
            );
            expect(mockInvoke).toHaveBeenNthCalledWith(2,
                'scan_directory',
                expect.objectContaining({
                    __csrf_token: expect.any(String)
                })
            );
        });

        it('should not expose CSRF token in error messages', async () => {
            mockInvoke.mockRejectedValue(new Error('Invalid request'));

            try {
                await tauriApi.scanDirectory('/test');
                expect.fail('Should have thrown an error');
            } catch (error: any) {
                // Error message should not contain the CSRF token
                expect(error.message).not.toContain('__csrf_token');
            }
        });
    });

    describe('Batch Operations with CSRF', () => {
        it('should include CSRF token in batch operations', async () => {
            mockInvoke.mockResolvedValue([
                { path: '/file1.txt', analysis: 'text file' },
                { path: '/file2.txt', analysis: 'image file' }
            ]);

            await tauriApi.batchAnalyzeFiles(['/file1.txt', '/file2.txt']);

            expect(mockInvoke).toHaveBeenCalledWith(
                'batch_analyze_files',
                expect.objectContaining({
                    paths: ['/file1.txt', '/file2.txt'],
                    __csrf_token: expect.any(String)
                })
            );
        });
    });
});