import { test, expect } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';

test.describe('StratoSort Accessibility Tests', () => {
	test('main page should be accessible', async ({ page }) => {
		await page.goto('/');

		// Wait for app to load
		await page.waitForSelector('[role="main"]', { timeout: 30000 });

		const results = await new AxeBuilder({ page }).analyze();

		// Check for accessibility violations
		expect(results.violations).toEqual([]);
	});

	test('keyboard navigation should work', async ({ page }) => {
		await page.goto('/');

		// Wait for app to load
		await page.waitForSelector('[role="main"]', { timeout: 30000 });

		// Test tab navigation
		await page.keyboard.press('Tab');
		const firstFocused = await page.evaluate(() => document.activeElement?.tagName);
		expect(firstFocused).toBeTruthy();

		// Test escape key for modals
		await page.keyboard.press('?'); // Open keyboard shortcuts
		await page.waitForSelector('[role="dialog"]');
		await page.keyboard.press('Escape');
		await expect(page.locator('[role="dialog"]')).toBeHidden();
	});

	test('proper ARIA labels should be present', async ({ page }) => {
		await page.goto('/');

		// Wait for app to load
		await page.waitForSelector('[role="main"]', { timeout: 30000 });

		// Check for main navigation
		const nav = await page.locator('[role="navigation"]');
		await expect(nav).toBeVisible();

		// Check for proper button labels
		const buttons = await page.locator('button').all();
		for (const button of buttons) {
			const text = await button.textContent();
			const ariaLabel = await button.getAttribute('aria-label');
			expect(text || ariaLabel).toBeTruthy();
		}
	});

	test('color contrast should meet WCAG standards', async ({ page }) => {
		await page.goto('/');

		// Wait for app to load
		await page.waitForSelector('[role="main"]', { timeout: 30000 });

		const results = await new AxeBuilder({ page })
			.withTags(['wcag2aa', 'wcag2aaa'])
			.analyze();

		// Filter to only check color contrast violations
		const contrastViolations = results.violations.filter(
			v => v.id === 'color-contrast'
		);

		expect(contrastViolations).toHaveLength(0);
	});

	test('forms should have proper labels', async ({ page }) => {
		await page.goto('/');

		// Navigate to settings page
		await page.click('text=Settings');
		await page.waitForSelector('form', { timeout: 5000 });

		// Check all inputs have labels
		const inputs = await page.locator('input:not([type="hidden"])').all();
		for (const input of inputs) {
			const id = await input.getAttribute('id');
			if (id) {
				const label = await page.locator(`label[for="${id}"]`);
				await expect(label).toBeVisible();
			} else {
				// Input should be wrapped in a label
				const parent = await input.evaluateHandle(el => el.parentElement);
				const tagName = await parent.evaluate(el => el?.tagName);
				expect(tagName).toBe('LABEL');
			}
		}
	});
});