import { describe, it, expect, vi, beforeEach } from 'vitest';
import { secureInvoke, csrfManager } from '$lib/utils/csrf-protection';

// Mock the invoke function from Tauri
vi.mock('@tauri-apps/api/core', () => ({
    invoke: vi.fn()
}));

describe('CSRF Protection', () => {
    let mockInvoke: any;

    beforeEach(async () => {
        vi.clearAllMocks();
        // Get the mocked invoke function
        const tauriCore = await import('@tauri-apps/api/core');
        mockInvoke = vi.mocked(tauriCore.invoke);
    });

    describe('secureInvoke', () => {
        it('should add CSRF token to command arguments', async () => {
            mockInvoke.mockResolvedValue({ success: true });

            const result = await secureInvoke('test_command', { param: 'value' });

            expect(mockInvoke).toHaveBeenCalledWith(
                'test_command',
                expect.objectContaining({
                    param: 'value',
                    __csrf_token: expect.any(String)
                })
            );
            expect(result).toEqual({ success: true });
        });

        it('should work with commands without arguments', async () => {
            mockInvoke.mockResolvedValue({ data: 'test' });

            const result = await secureInvoke('no_args_command');

            expect(mockInvoke).toHaveBeenCalledWith(
                'no_args_command',
                expect.objectContaining({
                    __csrf_token: expect.any(String)
                })
            );
            expect(result).toEqual({ data: 'test' });
        });

        it('should invalidate token after sensitive operations', async () => {
            mockInvoke.mockResolvedValue({ success: true });

            // Get initial token
            const initialToken = csrfManager.getToken();

            // Call a sensitive operation
            await secureInvoke('delete_files', { paths: ['test.txt'] });

            // Token should be different after sensitive operation
            const newToken = csrfManager.getToken();
            expect(newToken).not.toBe(initialToken);
        });

        it('should retry once on CSRF validation failure', async () => {
            // First call fails with CSRF error, second succeeds
            mockInvoke
                .mockRejectedValueOnce(new Error('CSRF token validation failed'))
                .mockResolvedValueOnce({ success: true });

            const result = await secureInvoke('test_command', { param: 'value' });

            expect(mockInvoke).toHaveBeenCalledTimes(2);
            expect(result).toEqual({ success: true });
        });

        it('should not retry on non-CSRF errors', async () => {
            mockInvoke.mockRejectedValue(new Error('Some other error'));

            await expect(secureInvoke('test_command', { param: 'value' }))
                .rejects.toThrow('Some other error');

            expect(mockInvoke).toHaveBeenCalledTimes(1);
        });

        it('should maintain token consistency within TTL', () => {
            const token1 = csrfManager.getToken();
            const token2 = csrfManager.getToken();

            expect(token1).toBe(token2);
        });

        it('should validate token correctly', () => {
            const token = csrfManager.getToken();

            expect(csrfManager.validateToken(token)).toBe(true);
            expect(csrfManager.validateToken('invalid-token')).toBe(false);
        });

        it('should handle token invalidation properly', () => {
            const token = csrfManager.getToken();
            expect(csrfManager.validateToken(token)).toBe(true);

            csrfManager.invalidateToken();

            // Old token should be invalid
            expect(csrfManager.validateToken(token)).toBe(false);

            // New token should be generated and valid
            const newToken = csrfManager.getToken();
            expect(newToken).not.toBe(token);
            expect(csrfManager.validateToken(newToken)).toBe(true);
        });
    });

    describe('Sensitive operations detection', () => {
        const sensitiveOps = [
            'delete_files',
            'move_files',
            'apply_organization',
            'update_settings',
            'create_smart_folder',
            'delete_smart_folder'
        ];

        sensitiveOps.forEach(op => {
            it(`should invalidate token after ${op}`, async () => {
                mockInvoke.mockResolvedValue({ success: true });

                const initialToken = csrfManager.getToken();
                await secureInvoke(op, {});
                const newToken = csrfManager.getToken();

                expect(newToken).not.toBe(initialToken);
            });
        });

        it('should not invalidate token for non-sensitive operations', async () => {
            mockInvoke.mockResolvedValue({ data: 'test' });

            const initialToken = csrfManager.getToken();
            await secureInvoke('scan_directory', { path: '/' });
            const newToken = csrfManager.getToken();

            expect(newToken).toBe(initialToken);
        });
    });

    describe('Token generation', () => {
        it('should generate cryptographically secure tokens', () => {
            const tokens = new Set<string>();

            // Generate multiple tokens
            for (let i = 0; i < 100; i++) {
                csrfManager.invalidateToken();
                tokens.add(csrfManager.getToken());
            }

            // All tokens should be unique
            expect(tokens.size).toBe(100);

            // Tokens should have sufficient length
            tokens.forEach(token => {
                expect(token.length).toBeGreaterThan(32);
            });
        });

        it('should include session ID in token', () => {
            const token = csrfManager.getToken();

            // Decode the base64 token
            const decoded = atob(token);

            // Token should contain session ID, timestamp, and random bytes
            expect(decoded).toMatch(/^[0-9a-f]{64}-\d+-[0-9a-f]{64}$/);
        });
    });
});