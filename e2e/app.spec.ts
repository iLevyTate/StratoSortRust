import { test, expect } from '@playwright/test';
import path from 'path';

test.describe('StratoSort E2E Tests', () => {
	test.beforeEach(async ({ page }) => {
		await page.goto('/');
		// Wait for app to initialize
		await page.waitForSelector('[data-testid="app-container"]', { timeout: 30000 });
	});

	test.describe('Application Launch', () => {
		test('should display main application window', async ({ page }) => {
			await expect(page).toHaveTitle(/StratoSort/);
			await expect(page.locator('[data-testid="sidebar"]')).toBeVisible();
			await expect(page.locator('[data-testid="main-content"]')).toBeVisible();
		});

		test('should show first-run setup for new users', async ({ page, context }) => {
			// Clear any existing settings
			await context.clearCookies();
			await page.evaluate(() => localStorage.clear());
			await page.reload();

			// Should show first-run setup
			await expect(page.locator('text=Welcome to StratoSort')).toBeVisible();
			await expect(page.locator('button:has-text("Get Started")')).toBeVisible();
		});

		test('should navigate through all main sections', async ({ page }) => {
			// Discover
			await page.click('[data-testid="nav-discover"]');
			await expect(page.locator('h1:has-text("Discover")')).toBeVisible();

			// Analyze
			await page.click('[data-testid="nav-analyze"]');
			await expect(page.locator('h1:has-text("Analyze")')).toBeVisible();

			// Organize
			await page.click('[data-testid="nav-organize"]');
			await expect(page.locator('h1:has-text("Organize")')).toBeVisible();

			// Settings
			await page.click('[data-testid="nav-settings"]');
			await expect(page.locator('h1:has-text("Settings")')).toBeVisible();
		});
	});

	test.describe('File Discovery', () => {
		test('should browse and display files', async ({ page }) => {
			await page.click('[data-testid="nav-discover"]');

			// Mock file dialog selection
			await page.evaluate(() => {
				// Mock the Tauri API response
				(window as any).__TAURI_MOCK__ = {
					scanDirectory: async () => [
						{ path: '/test/file1.txt', name: 'file1.txt', size: 1024, is_directory: false },
						{ path: '/test/file2.pdf', name: 'file2.pdf', size: 2048, is_directory: false }
					]
				};
			});

			await page.click('button:has-text("Browse")');

			// Wait for files to appear
			await expect(page.locator('text=file1.txt')).toBeVisible();
			await expect(page.locator('text=file2.pdf')).toBeVisible();
		});

		test('should support drag and drop', async ({ page }) => {
			await page.click('[data-testid="nav-discover"]');

			const dropZone = page.locator('[data-testid="drop-zone"]');
			await expect(dropZone).toBeVisible();

			// Create a data transfer for drag and drop
			const dataTransfer = await page.evaluateHandle(() => new DataTransfer());

			// Simulate file drop
			await dropZone.dispatchEvent('drop', { dataTransfer });

			// Verify drop zone reacts
			await expect(dropZone).toHaveClass(/drop-active/);
		});

		test('should search files', async ({ page }) => {
			await page.click('[data-testid="nav-discover"]');

			// Add some test files first
			await page.evaluate(() => {
				(window as any).__TAURI_MOCK__ = {
					scanDirectory: async () => [
						{ path: '/test/document.pdf', name: 'document.pdf', size: 1024, is_directory: false },
						{ path: '/test/image.jpg', name: 'image.jpg', size: 2048, is_directory: false },
						{ path: '/test/notes.txt', name: 'notes.txt', size: 512, is_directory: false }
					]
				};
			});

			await page.click('button:has-text("Browse")');
			await page.waitForSelector('text=document.pdf');

			// Search for specific file
			await page.fill('[data-testid="search-input"]', 'image');

			// Should show only matching file
			await expect(page.locator('text=image.jpg')).toBeVisible();
			await expect(page.locator('text=document.pdf')).not.toBeVisible();
			await expect(page.locator('text=notes.txt')).not.toBeVisible();
		});

		test('should select multiple files', async ({ page }) => {
			await page.click('[data-testid="nav-discover"]');

			// Add test files
			await page.evaluate(() => {
				(window as any).__TAURI_MOCK__ = {
					scanDirectory: async () => [
						{ path: '/test/file1.txt', name: 'file1.txt', size: 1024, is_directory: false },
						{ path: '/test/file2.txt', name: 'file2.txt', size: 1024, is_directory: false },
						{ path: '/test/file3.txt', name: 'file3.txt', size: 1024, is_directory: false }
					]
				};
			});

			await page.click('button:has-text("Browse")');
			await page.waitForSelector('text=file1.txt');

			// Select files
			await page.check('[data-testid="file-checkbox-0"]');
			await page.check('[data-testid="file-checkbox-1"]');

			// Verify selection count
			await expect(page.locator('text=2 files selected')).toBeVisible();

			// Select all
			await page.click('button:has-text("Select All")');
			await expect(page.locator('text=3 files selected')).toBeVisible();
		});
	});

	test.describe('File Analysis', () => {
		test('should analyze selected files', async ({ page }) => {
			// Navigate to discover and select files
			await page.click('[data-testid="nav-discover"]');

			await page.evaluate(() => {
				(window as any).__TAURI_MOCK__ = {
					scanDirectory: async () => [
						{ path: '/test/report.pdf', name: 'report.pdf', size: 1024, is_directory: false }
					],
					analyzeFile: async () => ({
						file_path: '/test/report.pdf',
						categories: ['Documents', 'Reports'],
						tags: ['quarterly', 'finance'],
						confidence_score: 0.85
					})
				};
			});

			await page.click('button:has-text("Browse")');
			await page.waitForSelector('text=report.pdf');
			await page.check('[data-testid="file-checkbox-0"]');

			// Navigate to analyze
			await page.click('[data-testid="nav-analyze"]');
			await page.click('button:has-text("Start Analysis")');

			// Wait for analysis to complete
			await expect(page.locator('[data-testid="analysis-progress"]')).toBeVisible();
			await expect(page.locator('text=Analysis Complete')).toBeVisible({ timeout: 10000 });

			// Verify results
			await expect(page.locator('text=Documents')).toBeVisible();
			await expect(page.locator('text=Reports')).toBeVisible();
			await expect(page.locator('text=85%')).toBeVisible(); // Confidence score
		});

		test('should filter analysis results', async ({ page }) => {
			await page.click('[data-testid="nav-analyze"]');

			// Mock analysis results
			await page.evaluate(() => {
				(window as any).__ANALYSIS_RESULTS__ = [
					{ file_name: 'doc1.pdf', categories: ['Documents'], confidence_score: 0.9 },
					{ file_name: 'image1.jpg', categories: ['Images'], confidence_score: 0.85 },
					{ file_name: 'doc2.txt', categories: ['Documents'], confidence_score: 0.8 }
				];
			});

			// Filter by category
			await page.selectOption('[data-testid="category-filter"]', 'Documents');

			// Should show only documents
			await expect(page.locator('text=doc1.pdf')).toBeVisible();
			await expect(page.locator('text=doc2.txt')).toBeVisible();
			await expect(page.locator('text=image1.jpg')).not.toBeVisible();
		});
	});

	test.describe('File Organization', () => {
		test('should generate organization suggestions', async ({ page }) => {
			await page.click('[data-testid="nav-organize"]');

			// Mock suggestions
			await page.evaluate(() => {
				(window as any).__TAURI_MOCK__ = {
					generateSuggestions: async () => [
						{
							file_path: '/test/report.pdf',
							suggested_path: '/organized/Documents/Reports/Q4_report.pdf',
							confidence: 0.9
						}
					]
				};
			});

			await page.click('button:has-text("Generate Suggestions")');

			// Wait for suggestions
			await expect(page.locator('text=/organized/Documents/Reports/')).toBeVisible();
			await expect(page.locator('text=90%')).toBeVisible(); // Confidence
		});

		test('should create smart folders', async ({ page }) => {
			await page.click('[data-testid="nav-organize"]');
			await page.click('button:has-text("Smart Folders")');
			await page.click('button:has-text("New Smart Folder")');

			// Fill in smart folder details
			await page.fill('[data-testid="folder-name"]', 'Financial Documents');

			// Add rule
			await page.click('button:has-text("Add Rule")');
			await page.selectOption('[data-testid="rule-field"]', 'name');
			await page.selectOption('[data-testid="rule-operator"]', 'contains');
			await page.fill('[data-testid="rule-value"]', 'invoice');

			// Save
			await page.click('button:has-text("Save")');

			// Verify smart folder created
			await expect(page.locator('text=Financial Documents')).toBeVisible();
		});

		test('should apply organization', async ({ page }) => {
			await page.click('[data-testid="nav-organize"]');

			// Mock organization operation
			await page.evaluate(() => {
				(window as any).__TAURI_MOCK__ = {
					applyOrganization: async () => ({
						successful: 3,
						failed: 0
					})
				};
			});

			await page.click('button:has-text("Apply Organization")');

			// Confirm dialog
			await page.click('button:has-text("Confirm")');

			// Wait for success message
			await expect(page.locator('text=Successfully organized 3 files')).toBeVisible();
		});
	});

	test.describe('Settings', () => {
		test('should update theme preference', async ({ page }) => {
			await page.click('[data-testid="nav-settings"]');

			// Change theme
			await page.selectOption('[data-testid="theme-select"]', 'dark');

			// Save settings
			await page.click('button:has-text("Save")');

			// Verify theme applied
			await expect(page.locator('body')).toHaveClass(/dark/);
		});

		test('should configure AI settings', async ({ page }) => {
			await page.click('[data-testid="nav-settings"]');
			await page.click('[data-testid="tab-ai"]');

			// Update Ollama settings
			await page.fill('[data-testid="ollama-host"]', 'http://192.168.1.100:11434');
			await page.selectOption('[data-testid="ollama-model"]', 'llama3.2:3b');

			// Test connection
			await page.click('button:has-text("Test Connection")');

			// Mock successful connection
			await page.evaluate(() => {
				(window as any).__TAURI_MOCK__ = {
					testOllamaConnection: async () => ({ isRunning: true })
				};
			});

			await expect(page.locator('text=Connected')).toBeVisible();
		});

		test('should manage privacy settings', async ({ page }) => {
			await page.click('[data-testid="nav-settings"]');
			await page.click('[data-testid="tab-privacy"]');

			// Toggle telemetry
			const telemetrySwitch = page.locator('[data-testid="telemetry-switch"]');
			await telemetrySwitch.click();

			// Toggle crash reports
			const crashReportsSwitch = page.locator('[data-testid="crash-reports-switch"]');
			await crashReportsSwitch.click();

			// Save settings
			await page.click('button:has-text("Save")');

			await expect(page.locator('text=Settings saved')).toBeVisible();
		});
	});

	test.describe('Keyboard Navigation', () => {
		test('should support keyboard shortcuts', async ({ page }) => {
			// Test Ctrl+O for open/browse
			await page.keyboard.press('Control+O');
			await expect(page.locator('[data-testid="file-dialog"]')).toBeVisible();
			await page.keyboard.press('Escape');

			// Test Ctrl+A for select all
			await page.click('[data-testid="nav-discover"]');
			await page.keyboard.press('Control+A');
			await expect(page.locator('text=All files selected')).toBeVisible();

			// Test navigation with Tab
			await page.keyboard.press('Tab');
			await expect(page.locator(':focus')).toHaveAttribute('data-testid', 'search-input');
		});

		test('should navigate sidebar with arrow keys', async ({ page }) => {
			await page.locator('[data-testid="nav-discover"]').focus();

			// Navigate down
			await page.keyboard.press('ArrowDown');
			await expect(page.locator('[data-testid="nav-analyze"]')).toBeFocused();

			// Navigate down again
			await page.keyboard.press('ArrowDown');
			await expect(page.locator('[data-testid="nav-organize"]')).toBeFocused();

			// Navigate up
			await page.keyboard.press('ArrowUp');
			await expect(page.locator('[data-testid="nav-analyze"]')).toBeFocused();
		});
	});

	test.describe('Error Handling', () => {
		test('should handle network errors gracefully', async ({ page }) => {
			await page.click('[data-testid="nav-discover"]');

			// Mock network error
			await page.evaluate(() => {
				(window as any).__TAURI_MOCK__ = {
					scanDirectory: async () => {
						throw new Error('Network error: Unable to connect');
					}
				};
			});

			await page.click('button:has-text("Browse")');

			// Should show error message
			await expect(page.locator('text=Network error')).toBeVisible();

			// Should show retry button
			await expect(page.locator('button:has-text("Retry")')).toBeVisible();
		});

		test('should handle Ollama connection failure', async ({ page }) => {
			await page.click('[data-testid="nav-settings"]');
			await page.click('[data-testid="tab-ai"]');

			// Mock connection failure
			await page.evaluate(() => {
				(window as any).__TAURI_MOCK__ = {
					testOllamaConnection: async () => ({ isRunning: false })
				};
			});

			await page.click('button:has-text("Test Connection")');

			// Should show error state
			await expect(page.locator('text=Not Connected')).toBeVisible();
			await expect(page.locator('text=Please check your Ollama installation')).toBeVisible();
		});
	});

	test.describe('Performance', () => {
		test('should handle large file lists efficiently', async ({ page }) => {
			await page.click('[data-testid="nav-discover"]');

			// Mock large file list
			await page.evaluate(() => {
				const files = Array.from({ length: 1000 }, (_, i) => ({
					path: `/test/file${i}.txt`,
					name: `file${i}.txt`,
					size: Math.random() * 10000,
					is_directory: false
				}));

				(window as any).__TAURI_MOCK__ = {
					scanDirectory: async () => files
				};
			});

			const startTime = Date.now();
			await page.click('button:has-text("Browse")');

			// Wait for files to load
			await page.waitForSelector('text=file0.txt');
			const loadTime = Date.now() - startTime;

			// Should load within reasonable time (5 seconds)
			expect(loadTime).toBeLessThan(5000);

			// Should use virtualization (not all items rendered)
			const visibleItems = await page.locator('[data-testid^="file-row-"]').count();
			expect(visibleItems).toBeLessThan(100); // Only visible items should be rendered
		});
	});
});

test.describe('Accessibility', () => {
	test('should meet WCAG 2.1 AA standards', async ({ page }) => {
		await page.goto('/');

		// Check for proper heading structure
		const h1Count = await page.locator('h1').count();
		expect(h1Count).toBe(1);

		// Check all images have alt text
		const images = page.locator('img');
		const imageCount = await images.count();
		for (let i = 0; i < imageCount; i++) {
			await expect(images.nth(i)).toHaveAttribute('alt', /.+/);
		}

		// Check all form inputs have labels
		const inputs = page.locator('input, select, textarea');
		const inputCount = await inputs.count();
		for (let i = 0; i < inputCount; i++) {
			const input = inputs.nth(i);
			const id = await input.getAttribute('id');
			if (id) {
				const label = page.locator(`label[for="${id}"]`);
				await expect(label).toHaveCount(1);
			} else {
				// Should have aria-label if no id/label pair
				await expect(input).toHaveAttribute('aria-label', /.+/);
			}
		}

		// Check color contrast (this would need axe-core or similar)
		// For now, just check that high contrast mode is available
		await page.click('[data-testid="nav-settings"]');
		await expect(page.locator('[data-testid="high-contrast-toggle"]')).toBeVisible();
	});

	test('should be fully navigable with keyboard only', async ({ page }) => {
		await page.goto('/');

		// Start at the top of the page
		await page.keyboard.press('Tab');

		// Should focus skip link
		await expect(page.locator(':focus')).toHaveAttribute('data-testid', 'skip-to-content');

		// Tab through main navigation
		for (const nav of ['discover', 'analyze', 'organize', 'settings']) {
			await page.keyboard.press('Tab');
			await expect(page.locator(':focus')).toHaveAttribute('data-testid', `nav-${nav}`);
		}

		// Activate with Enter
		await page.keyboard.press('Enter');
		await expect(page.locator('h1:has-text("Settings")')).toBeVisible();
	});

	test('should work with screen readers', async ({ page }) => {
		await page.goto('/');

		// Check for ARIA landmarks
		await expect(page.locator('[role="navigation"]')).toHaveCount(1);
		await expect(page.locator('[role="main"]')).toHaveCount(1);

		// Check for ARIA live regions
		await expect(page.locator('[aria-live="polite"]')).toBeVisible();
		await expect(page.locator('[aria-live="assertive"]')).toBeVisible();

		// Check for proper ARIA labels
		const buttons = page.locator('button');
		const buttonCount = await buttons.count();
		for (let i = 0; i < buttonCount; i++) {
			const button = buttons.nth(i);
			const text = await button.textContent();
			const ariaLabel = await button.getAttribute('aria-label');
			expect(text || ariaLabel).toBeTruthy();
		}
	});
});