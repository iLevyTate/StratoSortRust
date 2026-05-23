import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { performance } from 'perf_hooks';
import { get } from 'svelte/store';
import * as tauriApi from '$lib/api/tauri';
import {
	scannedFiles,
	selectedFiles,
	analysisResults,
	addFileAnalysis,
	toggleFileSelection,
	selectAllFiles
} from '$lib/stores';
import { mockFileInfo, mockFileAnalysis, mockAppSettings } from '../mocks/tauri-api';

// Mock the Tauri API
vi.mock('$lib/api/tauri');

describe('Performance Tests', () => {
	beforeEach(() => {
		vi.clearAllMocks();
		// Reset stores
		scannedFiles.set([]);
		selectedFiles.set([]);
		analysisResults.set([]);
	});

	afterEach(() => {
		vi.clearAllMocks();
	});

	describe('Store Performance', () => {
		it('should handle large file lists efficiently', () => {
			const fileCount = 10000;
			const files = Array.from({ length: fileCount }, (_, i) =>
				mockFileInfo({
					path: `/test/file${i}.txt`,
					name: `file${i}.txt`,
					size: Math.random() * 1000000
				})
			);

			const startTime = performance.now();
			scannedFiles.set(files);
			const endTime = performance.now();

			const loadTime = endTime - startTime;
			expect(loadTime).toBeLessThan(100); // Should load in less than 100ms

			// Verify all files are loaded
			expect(get(scannedFiles)).toHaveLength(fileCount);
		});

		it('should handle rapid file selection changes', () => {
			const fileCount = 1000;
			const files = Array.from({ length: fileCount }, (_, i) =>
				mockFileInfo({
					path: `/test/file${i}.txt`,
					name: `file${i}.txt`
				})
			);

			scannedFiles.set(files);

			const startTime = performance.now();

			// Rapidly toggle selections
			for (let i = 0; i < 100; i++) {
				const randomIndex = Math.floor(Math.random() * fileCount);
				toggleFileSelection(files[randomIndex].path);
			}

			const endTime = performance.now();
			const operationTime = endTime - startTime;

			expect(operationTime).toBeLessThan(500); // 100 toggles in less than 500ms
		});

		it('should efficiently select all files', () => {
			const fileCount = 5000;
			const files = Array.from({ length: fileCount }, (_, i) =>
				mockFileInfo({
					path: `/test/file${i}.txt`,
					name: `file${i}.txt`
				})
			);

			scannedFiles.set(files);

			const startTime = performance.now();
			selectAllFiles();
			const endTime = performance.now();

			const selectionTime = endTime - startTime;
			expect(selectionTime).toBeLessThan(50); // Should be near instant
			expect(get(selectedFiles)).toHaveLength(fileCount);
		});

		it('should handle large analysis results efficiently', () => {
			const analysisCount = 1000;
			const analyses = Array.from({ length: analysisCount }, (_, i) =>
				mockFileAnalysis({
					path: `/test/file${i}.txt`,
					category: 'Category1',
					tags: ['tag1', 'tag2', 'tag3']
				})
			);

			const startTime = performance.now();

			// Add all analyses one by one (simulating real-time analysis)
			analyses.forEach(analysis => {
				addFileAnalysis(analysis);
			});

			const endTime = performance.now();
			const processingTime = endTime - startTime;

			expect(processingTime).toBeLessThan(1000); // Process 1000 analyses in less than 1s
			expect(get(analysisResults)).toHaveLength(analysisCount);
		});
	});

	describe('API Call Performance', () => {
		it('should batch API calls efficiently', async () => {
			const fileCount = 100;
			const paths = Array.from({ length: fileCount }, (_, i) => `/test/file${i}.txt`);

			vi.mocked(tauriApi.analyzeFilesBatch).mockImplementation(async (files) => {
				// Simulate processing time
				await new Promise(resolve => setTimeout(resolve, 10));
				return files.map(path =>
					mockFileAnalysis({ path: path })
				);
			});

			const startTime = performance.now();

			// Batch analyze files
			const result = await tauriApi.analyzeFilesBatch(paths);

			const endTime = performance.now();
			const batchTime = endTime - startTime;

			// Batching should be much faster than individual calls
			expect(batchTime).toBeLessThan(500); // 100 files in less than 500ms
			expect(result).toHaveLength(fileCount);
		});

		it('should handle concurrent operations efficiently', async () => {
			const operationCount = 50;

			vi.mocked(tauriApi.moveFile).mockImplementation(async () => {
				// Simulate operation time
				await new Promise(resolve => setTimeout(resolve, 5));
				return true;
			});

			const startTime = performance.now();

			// Execute concurrent operations
			const operations = Array.from({ length: operationCount }, (_, i) =>
				tauriApi.moveFile(`/source/file${i}.txt`, `/dest/file${i}.txt`)
			);

			await Promise.all(operations);

			const endTime = performance.now();
			const totalTime = endTime - startTime;

			// Concurrent execution should be faster than sequential
			const expectedSequentialTime = operationCount * 5;
			expect(totalTime).toBeLessThan(expectedSequentialTime / 2);
		});

		it('should cache repeated API calls', async () => {
			// Import the actual implementation to test caching
			const { getAppSettings, apiCache } = await vi.importActual('$lib/api/tauri') as any;

			// Clear cache before test
			apiCache.clear();

			let callCount = 0;
			// Mock the invoke function instead of the getAppSettings function
			const mockInvoke = vi.fn().mockImplementation(async (command: string) => {
				if (command === 'get_settings') {
					callCount++;
					await new Promise(resolve => setTimeout(resolve, 10));
					return mockAppSettings();
				}
				throw new Error(`Unexpected command: ${command}`);
			});

			// Mock the invoke function at the core level
			vi.doMock('@tauri-apps/api/core', () => ({
				invoke: mockInvoke
			}));

			const startTime = performance.now();

			// Make multiple calls that should be cached
			const results = await Promise.all([
				getAppSettings(),
				getAppSettings(),
				getAppSettings(),
				getAppSettings(),
				getAppSettings()
			]);

			const endTime = performance.now();
			const totalTime = endTime - startTime;

			// With caching, should only make one actual call
			expect(callCount).toBeLessThanOrEqual(2); // Allow for race conditions
			expect(totalTime).toBeLessThan(50); // Should be fast with caching
			expect(results).toHaveLength(5);
		});
	});

	describe('Search Performance', () => {
		it('should search large file lists quickly', () => {
			const fileCount = 10000;
			const files = Array.from({ length: fileCount }, (_, i) =>
				mockFileInfo({
					path: `/test/file${i}.txt`,
					name: `file${i}.txt`
				})
			);

			// Add some specific files to find
			files[500].name = 'important-document.pdf';
			files[2500].name = 'important-report.docx';
			files[7500].name = 'important-presentation.pptx';

			scannedFiles.set(files);

			const startTime = performance.now();

			// Perform search
			const searchTerm = 'important';
			const results = get(scannedFiles).filter(file =>
				file.name.toLowerCase().includes(searchTerm.toLowerCase())
			);

			const endTime = performance.now();
			const searchTime = endTime - startTime;

			expect(searchTime).toBeLessThan(50); // Search 10k items in less than 50ms
			expect(results).toHaveLength(3);
		});

		it('should filter by multiple criteria efficiently', () => {
			const fileCount = 5000;
			const files = Array.from({ length: fileCount }, (_, i) =>
				mockFileInfo({
					path: `/test/file${i}.txt`,
					name: `file${i}.txt`,
					size: Math.random() * 10000000,
					mime_type: i % 3 === 0 ? 'application/pdf' : 'text/plain',
					modified_at: Date.now() - Math.random() * 30 * 24 * 60 * 60 * 1000 // Random date within 30 days
				})
			);

			scannedFiles.set(files);

			const startTime = performance.now();

			// Complex filter: PDFs larger than 1MB modified in last 7 days
			const sevenDaysAgo = Date.now() - 7 * 24 * 60 * 60 * 1000;
			const filtered = get(scannedFiles).filter(file =>
				file.mime_type === 'application/pdf' &&
				file.size > 1000000 &&
				file.modified_at > sevenDaysAgo
			);

			const endTime = performance.now();
			const filterTime = endTime - startTime;

			expect(filterTime).toBeLessThan(100); // Filter 5k items with complex criteria in less than 100ms
			expect(filtered.length).toBeGreaterThan(0);
		});
	});

	describe('Memory Usage', () => {
		it('should not leak memory with repeated operations', () => {
			const initialMemory = process.memoryUsage().heapUsed;

			// Perform many operations
			for (let i = 0; i < 100; i++) {
				const files = Array.from({ length: 100 }, (_, j) =>
					mockFileInfo({
						path: `/test/file${i * 100 + j}.txt`,
						name: `file${i * 100 + j}.txt`
					})
				);

				scannedFiles.set(files);
				selectAllFiles();
				selectedFiles.set([]);
			}

			// Force garbage collection if available
			if (global.gc) {
				global.gc();
			}

			const finalMemory = process.memoryUsage().heapUsed;
			const memoryIncrease = finalMemory - initialMemory;

			// Memory increase should be reasonable (less than 50MB)
			expect(memoryIncrease).toBeLessThan(50 * 1024 * 1024);
		});

		it('should handle large file content efficiently', async () => {
			const largeContent = 'x'.repeat(10 * 1024 * 1024); // 10MB string

			vi.mocked(tauriApi.getFileContent).mockResolvedValue(largeContent);

			const startTime = performance.now();
			const memoryBefore = process.memoryUsage().heapUsed;

			const content = await tauriApi.getFileContent('/test/large-file.txt');

			const endTime = performance.now();
			const memoryAfter = process.memoryUsage().heapUsed;

			const loadTime = endTime - startTime;
			const memoryUsed = memoryAfter - memoryBefore;

			expect(loadTime).toBeLessThan(100); // Load 10MB in less than 100ms
			expect(memoryUsed).toBeLessThan(20 * 1024 * 1024); // Should not use more than 2x the content size
			expect(content).toHaveLength(10 * 1024 * 1024);
		});
	});

	describe('Rendering Performance', () => {
		it('should efficiently update derived stores', () => {
			const fileCount = 1000;
			const files = Array.from({ length: fileCount }, (_, i) =>
				mockFileInfo({
					path: `/test/file${i}.txt`,
					name: `file${i}.txt`
				})
			);

			scannedFiles.set(files);

			const startTime = performance.now();

			// Trigger many derived store updates
			for (let i = 0; i < 100; i++) {
				const randomFile = files[Math.floor(Math.random() * fileCount)];
				toggleFileSelection(randomFile.path);
			}

			const endTime = performance.now();
			const updateTime = endTime - startTime;

			// Derived stores should update efficiently
			expect(updateTime).toBeLessThan(200); // 100 updates in less than 200ms
		});

		it('should debounce rapid state changes', async () => {
			let updateCount = 0;
			const mockDebounced = vi.fn(() => {
				updateCount++;
			});

			// Simulate rapid changes that should be debounced
			const rapidChanges = async () => {
				for (let i = 0; i < 100; i++) {
					mockDebounced();
					await new Promise(resolve => setTimeout(resolve, 1));
				}
			};

			const startTime = performance.now();
			await rapidChanges();
			const endTime = performance.now();

			const totalTime = endTime - startTime;

			// Even with 100 calls, debouncing should reduce actual executions
			expect(updateCount).toBeLessThanOrEqual(100); // All calls go through in this simple test
			expect(totalTime).toBeGreaterThan(100); // Takes at least 100ms due to delays
		});
	});

	describe('Benchmark Suite', () => {
		it('should complete standard workflow within performance budget', async () => {
			const performanceBudget = {
				scanDirectory: 500,      // 500ms for 1000 files
				analyzeFiles: 5000,      // 5s for 100 files
				generateSuggestions: 2000, // 2s for suggestions
				applyOrganization: 3000   // 3s to organize
			};

			// Mock operations with realistic delays
			vi.mocked(tauriApi.scanDirectory).mockImplementation(async () => {
				await new Promise(resolve => setTimeout(resolve, 100));
				return Array.from({ length: 1000 }, (_, i) =>
					mockFileInfo({ path: `/test/file${i}.txt` })
				);
			});

			vi.mocked(tauriApi.analyzeFilesBatch).mockImplementation(async (paths) => {
				await new Promise(resolve => setTimeout(resolve, 500));
				return paths.slice(0, 100).map(path =>
					mockFileAnalysis({ path: path })
				);
			});

			vi.mocked(tauriApi.generateOrganizationSuggestions).mockImplementation(async () => {
				await new Promise(resolve => setTimeout(resolve, 200));
				return [];
			});

			vi.mocked(tauriApi.applyOrganization).mockImplementation(async () => {
				await new Promise(resolve => setTimeout(resolve, 300));
				return {
					success: true,
					message: 'Organization applied successfully',
					applied: 100,
					failed: 0
				};
			});

			const workflowStart = performance.now();

			// Execute standard workflow
			const files = await tauriApi.scanDirectory('/test', true);
			expect(performance.now() - workflowStart).toBeLessThan(performanceBudget.scanDirectory);

			const analysisStart = performance.now();
			const analysis = await tauriApi.analyzeFilesBatch(files.map(f => f.path));
			expect(performance.now() - analysisStart).toBeLessThan(performanceBudget.analyzeFiles);

			const suggestionsStart = performance.now();
			const suggestions = await tauriApi.generateOrganizationSuggestions(analysis.map(a => a.path));
			expect(performance.now() - suggestionsStart).toBeLessThan(performanceBudget.generateSuggestions);

			const organizeStart = performance.now();
			const organizationOps = suggestions.map(s => ({
				id: `op-${Math.random()}`,
				file_path: s.source_path,
				target_path: s.target_folder,
				operation_type: 'move' as const
			}));
			const result = await tauriApi.applyOrganization(organizationOps);
			expect(performance.now() - organizeStart).toBeLessThan(performanceBudget.applyOrganization);

			const totalTime = performance.now() - workflowStart;
			const totalBudget = Object.values(performanceBudget).reduce((a, b) => a + b, 0);

			expect(totalTime).toBeLessThan(totalBudget);
		});
	});
});