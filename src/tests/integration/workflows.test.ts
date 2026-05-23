import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, waitFor, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { get } from 'svelte/store';
import App from '../../App.svelte';
import * as tauriApi from '$lib/api/tauri';
import {
	scannedFiles,
	selectedFiles,
	analysisResults,
	organizationSuggestions,
	currentPage,
	appSettings
} from '$lib/stores';
import {
	mockFileInfo,
	mockFileAnalysis,
	mockOllamaStatus,
	mockAppSettings,
	mockSmartFolder,
	simulateProgress,
	simulateOperationComplete
} from '../mocks/tauri-api';

// Mock the Tauri API
vi.mock('$lib/api/tauri');

describe('User Workflows Integration Tests', () => {
	let user: ReturnType<typeof userEvent.setup>;

	beforeEach(() => {
		vi.clearAllMocks();
		user = userEvent.setup();

		// Reset stores
		scannedFiles.set([]);
		selectedFiles.set([]);
		analysisResults.set([]);
		organizationSuggestions.set([]);
		currentPage.set('discover');

		// Setup default mocks
		setupDefaultMocks();
	});

	afterEach(() => {
		vi.clearAllMocks();
	});

	function setupDefaultMocks() {
		vi.mocked(tauriApi.checkFirstRunStatus).mockResolvedValue({
			is_first_run: false,
			setup_completed: true
		});

		vi.mocked(tauriApi.getAppSettings).mockResolvedValue(mockAppSettings());
		vi.mocked(tauriApi.saveAppSettings).mockResolvedValue(undefined);
		vi.mocked(tauriApi.checkOllamaStatus).mockResolvedValue(mockOllamaStatus());
		vi.mocked(tauriApi.listModels).mockResolvedValue(['llama3.2:3b', 'llava:7b']);
		vi.mocked(tauriApi.getNotifications).mockResolvedValue([]);
		vi.mocked(tauriApi.getSmartFolders).mockResolvedValue([mockSmartFolder()]);
		vi.mocked(tauriApi.frontendReady).mockResolvedValue(undefined);

		// Mock event listeners
		vi.mocked(tauriApi.listenToProgressEvents).mockResolvedValue(() => {});
		vi.mocked(tauriApi.listenToOperationComplete).mockResolvedValue(() => {});
		vi.mocked(tauriApi.listenToOperationError).mockResolvedValue(() => {});
		vi.mocked(tauriApi.listenToFileChanges).mockResolvedValue(() => {});
		vi.mocked(tauriApi.listenToOllamaStatus).mockResolvedValue(() => {});
		vi.mocked(tauriApi.listenToNotifications).mockResolvedValue(() => {});
	}

	describe('Complete File Organization Workflow', () => {
		it('should complete full workflow: Discover → Analyze → Organize', async () => {
			// Setup mocks for the complete workflow
			const mockFiles = [
				mockFileInfo({ path: '/docs/report.pdf', name: 'report.pdf', mime_type: 'application/pdf' }),
				mockFileInfo({ path: '/docs/notes.txt', name: 'notes.txt', mime_type: 'text/plain' }),
				mockFileInfo({ path: '/docs/image.jpg', name: 'image.jpg', mime_type: 'image/jpeg' })
			];

			const mockAnalyses = [
				mockFileAnalysis({
					path: '/docs/report.pdf',
					category: 'Documents',
					tags: ['quarterly', 'finance']
				}),
				mockFileAnalysis({
					path: '/docs/notes.txt',
					category: 'Documents',
					tags: ['meeting', 'todo']
				}),
				mockFileAnalysis({
					path: '/docs/image.jpg',
					category: 'Images',
					tags: ['screenshot', 'ui']
				})
			];

			vi.mocked(tauriApi.openDirectoryDialog).mockResolvedValue('/docs');
			vi.mocked(tauriApi.scanDirectory).mockResolvedValue(mockFiles);
			vi.mocked(tauriApi.analyzeFilesBatch).mockResolvedValue(mockAnalyses);
			vi.mocked(tauriApi.generateOrganizationSuggestions).mockResolvedValue([
				{
					source_path: '/docs/report.pdf',
					target_folder: '/organized/Documents/Reports',
					confidence: 0.9,
					reason: 'Financial report should be in Reports folder'
				},
				{
					source_path: '/docs/notes.txt',
					target_folder: '/organized/Documents/Notes',
					confidence: 0.85,
					reason: 'Meeting notes belong in Notes folder'
				},
				{
					source_path: '/docs/image.jpg',
					target_folder: '/organized/Images/Screenshots',
					confidence: 0.8,
					reason: 'UI screenshot should be organized with other screenshots'
				}
			]);
			vi.mocked(tauriApi.applyOrganization).mockResolvedValue({
				success: true,
				message: 'Organization completed',
				applied: 3,
				failed: 0,
				errors: []
			});

			// Render the app
			render(App);

			// Wait for initial load
			await waitFor(() => {
				expect(screen.getByText(/discover/i)).toBeInTheDocument();
			});

			// Step 1: Discover files
			expect(get(currentPage)).toBe('discover');

			const browseButton = await screen.findByRole('button', { name: /browse/i });
			await user.click(browseButton);

			await waitFor(() => {
				expect(tauriApi.scanDirectory).toHaveBeenCalledWith('/docs', false);
				expect(get(scannedFiles)).toHaveLength(3);
			});

			// Select all files
			const selectAllButton = await screen.findByRole('button', { name: /select all/i });
			await user.click(selectAllButton);

			expect(get(selectedFiles)).toHaveLength(3);

			// Step 2: Navigate to Analyze
			const analyzeNavButton = await screen.findByRole('button', { name: /analyze/i });
			await user.click(analyzeNavButton);

			expect(get(currentPage)).toBe('analyze');

			// Start analysis
			const startAnalysisButton = await screen.findByRole('button', { name: /start analysis/i });
			await user.click(startAnalysisButton);

			// Simulate progress events
			simulateProgress('analyze-batch-1', 33, 'Analyzing report.pdf...');
			simulateProgress('analyze-batch-1', 66, 'Analyzing notes.txt...');
			simulateProgress('analyze-batch-1', 100, 'Analyzing image.jpg...');
			simulateOperationComplete('analyze-batch-1');

			await waitFor(() => {
				expect(tauriApi.analyzeFilesBatch).toHaveBeenCalledWith([
					'/docs/report.pdf',
					'/docs/notes.txt',
					'/docs/image.jpg'
				]);
				expect(get(analysisResults)).toHaveLength(3);
			});

			// Verify analysis results are displayed
			expect(screen.getByText(/quarterly/i)).toBeInTheDocument();
			expect(screen.getByText(/meeting/i)).toBeInTheDocument();
			expect(screen.getByText(/screenshot/i)).toBeInTheDocument();

			// Step 3: Navigate to Organize
			const organizeNavButton = await screen.findByRole('button', { name: /organize/i });
			await user.click(organizeNavButton);

			expect(get(currentPage)).toBe('organize');

			// Generate organization suggestions
			const generateSuggestionsButton = await screen.findByRole('button', {
				name: /generate suggestions/i
			});
			await user.click(generateSuggestionsButton);

			await waitFor(() => {
				expect(tauriApi.generateOrganizationSuggestions).toHaveBeenCalled();
				expect(get(organizationSuggestions)).toHaveLength(3);
			});

			// Review suggestions
			expect(screen.getByText(/Reports\/Q4_report.pdf/i)).toBeInTheDocument();
			expect(screen.getByText(/Notes\/meeting_notes.txt/i)).toBeInTheDocument();
			expect(screen.getByText(/Screenshots\/ui_screenshot.jpg/i)).toBeInTheDocument();

			// Apply organization
			const applyButton = await screen.findByRole('button', { name: /apply organization/i });
			await user.click(applyButton);

			// Simulate organization progress
			simulateProgress('organize-1', 50, 'Moving files...');
			simulateOperationComplete('organize-1');

			await waitFor(() => {
				expect(tauriApi.applyOrganization).toHaveBeenCalled();
				expect(screen.getByText(/successfully organized 3 files/i)).toBeInTheDocument();
			});
		});
	});

	describe('Smart Folder Workflow', () => {
		it('should create and apply smart folder rules', async () => {
			const mockFiles = [
				mockFileInfo({ path: '/docs/invoice.pdf', name: 'invoice.pdf' }),
				mockFileInfo({ path: '/docs/receipt.pdf', name: 'receipt.pdf' }),
				mockFileInfo({ path: '/docs/contract.docx', name: 'contract.docx' })
			];

			vi.mocked(tauriApi.scanDirectory).mockResolvedValue(mockFiles);
			vi.mocked(tauriApi.createSmartFolder).mockResolvedValue(
				mockSmartFolder({
					id: 'finance-folder',
					name: 'Financial Documents',
					rules: [
						{
							id: 'rule-1',
							rule_type: 'FileName',
							condition: {
								field: 'name',
								operator: 'Contains',
								value: 'invoice'
							},
							action: {
								action_type: 'Move',
								target_folder: '/invoices'
							},
							priority: 1,
							enabled: true
						},
						{
							id: 'rule-2',
							rule_type: 'FileName',
							condition: {
								field: 'name',
								operator: 'Contains',
								value: 'receipt'
							},
							action: {
								action_type: 'Move',
								target_folder: '/receipts'
							},
							priority: 2,
							enabled: true
						}
					]
				})
			);
			vi.mocked(tauriApi.applySmartFolderRules).mockResolvedValue([
				{
					id: 'preview-1',
					file_path: '/docs/invoice1.pdf',
					current_location: '/docs',
					suggested_location: '/invoices',
					reason: 'Contains "invoice" in filename',
					confidence: 0.9
				},
				{
					id: 'preview-2',
					file_path: '/docs/receipt1.pdf',
					current_location: '/docs',
					suggested_location: '/receipts',
					reason: 'Contains "receipt" in filename',
					confidence: 0.9
				}
			]);

			render(App);

			// Navigate to organize page
			const organizeButton = await screen.findByRole('button', { name: /organize/i });
			await user.click(organizeButton);

			// Open smart folders manager
			const smartFoldersButton = await screen.findByRole('button', { name: /smart folders/i });
			await user.click(smartFoldersButton);

			// Create new smart folder
			const newFolderButton = await screen.findByRole('button', { name: /new smart folder/i });
			await user.click(newFolderButton);

			// Configure smart folder
			const nameInput = await screen.findByLabelText(/folder name/i);
			await user.type(nameInput, 'Financial Documents');

			// Add rule for invoices
			const addRuleButton = await screen.findByRole('button', { name: /add rule/i });
			await user.click(addRuleButton);

			const fieldSelect = await screen.findByLabelText(/field/i);
			await user.selectOptions(fieldSelect, 'name');

			const operatorSelect = await screen.findByLabelText(/operator/i);
			await user.selectOptions(operatorSelect, 'contains');

			const valueInput = await screen.findByLabelText(/value/i);
			await user.type(valueInput, 'invoice');

			// Save smart folder
			const saveButton = await screen.findByRole('button', { name: /save/i });
			await user.click(saveButton);

			await waitFor(() => {
				expect(tauriApi.createSmartFolder).toHaveBeenCalled();
			});

			// Apply smart folder rules
			const applyRulesButton = await screen.findByRole('button', { name: /apply rules/i });
			await user.click(applyRulesButton);

			await waitFor(() => {
				expect(tauriApi.applySmartFolderRules).toHaveBeenCalledWith('finance-folder');
				expect(screen.getByText(/processed 2 files/i)).toBeInTheDocument();
			});
		});
	});

	describe('Batch Operations Workflow', () => {
		it('should perform batch file operations with undo/redo', async () => {
			const mockFiles = [
				mockFileInfo({ path: '/old/file1.txt', name: 'file1.txt' }),
				mockFileInfo({ path: '/old/file2.txt', name: 'file2.txt' }),
				mockFileInfo({ path: '/old/file3.txt', name: 'file3.txt' })
			];

			vi.mocked(tauriApi.scanDirectory).mockResolvedValue(mockFiles);
			vi.mocked(tauriApi.openDirectoryDialog)
				.mockResolvedValueOnce('/old')
				.mockResolvedValueOnce('/new');
			vi.mocked(tauriApi.moveFilesBatch).mockResolvedValue({
				total: 3,
				successful: 3,
				failed: 0,
				results: [
					{ path: '/old/file1.txt', success: true },
					{ path: '/old/file2.txt', success: true },
					{ path: '/old/file3.txt', success: true }
				]
			});
			vi.mocked(tauriApi.getHistory).mockResolvedValue([
				{
					id: 'op-1',
					timestamp: new Date().toISOString(),
					operation: 'move_batch',
					description: 'Moved 3 files',
					reversible: true
				}
			]);
			vi.mocked(tauriApi.undoOperation).mockResolvedValue(true);

			render(App);

			// Discover files
			const browseButton = await screen.findByRole('button', { name: /browse/i });
			await user.click(browseButton);

			await waitFor(() => {
				expect(get(scannedFiles)).toHaveLength(3);
			});

			// Select all files
			const selectAllButton = await screen.findByRole('button', { name: /select all/i });
			await user.click(selectAllButton);

			// Move files
			const moveButton = await screen.findByRole('button', { name: /move selected/i });
			await user.click(moveButton);

			await waitFor(() => {
				expect(tauriApi.moveFilesBatch).toHaveBeenCalledWith(
					['/old/file1.txt', '/old/file2.txt', '/old/file3.txt'],
					'/new'
				);
			});

			// Check history
			const historyButton = await screen.findByRole('button', { name: /history/i });
			await user.click(historyButton);

			await waitFor(() => {
				expect(screen.getByText(/moved 3 files/i)).toBeInTheDocument();
			});

			// Undo operation
			const undoButton = await screen.findByRole('button', { name: /undo/i });
			await user.click(undoButton);

			await waitFor(() => {
				expect(tauriApi.undoOperation).toHaveBeenCalled();
				expect(screen.getByText(/batch move undone/i)).toBeInTheDocument();
			});
		});
	});

	describe('Error Recovery Workflow', () => {
		it('should handle and recover from errors gracefully', async () => {
			// Simulate various error scenarios
			vi.mocked(tauriApi.scanDirectory)
				.mockRejectedValueOnce(new Error('Permission denied'))
				.mockResolvedValueOnce([mockFileInfo()]);

			vi.mocked(tauriApi.checkOllamaStatus).mockResolvedValue({
				isRunning: false,
				version: '',
				models: []
			});

			render(App);

			// Try to scan directory (will fail)
			const browseButton = await screen.findByRole('button', { name: /browse/i });
			await user.click(browseButton);

			await waitFor(() => {
				expect(screen.getByText(/permission denied/i)).toBeInTheDocument();
			});

			// Retry operation
			const retryButton = await screen.findByRole('button', { name: /retry/i });
			await user.click(retryButton);

			await waitFor(() => {
				expect(get(scannedFiles)).toHaveLength(1);
			});

			// Check Ollama connection status
			const settingsButton = await screen.findByRole('button', { name: /settings/i });
			await user.click(settingsButton);

			await waitFor(() => {
				expect(screen.getByText(/ollama.*not connected/i)).toBeInTheDocument();
			});

			// Attempt to reconnect
			const reconnectButton = await screen.findByRole('button', { name: /reconnect/i });

			vi.mocked(tauriApi.checkOllamaStatus).mockResolvedValue(mockOllamaStatus());

			await user.click(reconnectButton);

			await waitFor(() => {
				expect(screen.getByText(/ollama.*connected/i)).toBeInTheDocument();
			});
		});
	});

	describe('Settings Persistence Workflow', () => {
		it('should persist and apply user settings across sessions', async () => {
			const customSettings = {
				...mockAppSettings(),
				theme: 'dark',
				auto_analyze_on_add: true,
				notification_duration: 5000
			};

			vi.mocked(tauriApi.getAppSettings).mockResolvedValue(customSettings);

			render(App);

			// Navigate to settings
			const settingsButton = await screen.findByRole('button', { name: /settings/i });
			await user.click(settingsButton);

			await waitFor(() => {
				expect(get(currentPage)).toBe('settings');
			});

			// Verify settings are loaded
			const themeSelect = await screen.findByLabelText(/theme/i);
			expect(themeSelect).toHaveValue('dark');

			const autoAnalyzeSwitch = await screen.findByLabelText(/auto.*analyze/i);
			expect(autoAnalyzeSwitch).toBeChecked();

			// Change settings
			await user.selectOptions(themeSelect, 'light');

			const saveButton = await screen.findByRole('button', { name: /save/i });
			await user.click(saveButton);

			await waitFor(() => {
				expect(tauriApi.saveAppSettings).toHaveBeenCalledWith(
					expect.objectContaining({
						theme: 'light',
						auto_analyze_on_add: true
					})
				);
			});

			// Verify settings are applied
			expect(get(appSettings).theme).toBe('light');
		});
	});
});