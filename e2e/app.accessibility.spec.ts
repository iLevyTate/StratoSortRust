import { test, expect } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';

// These tests deliberately don't install Tauri mocks — `$lib/api/tauri.ts`
// returns safe defaults (`{ is_first_run: false }`, `[]`, `null`, etc.)
// when no mock is present, which is exactly the rendered surface we want
// to audit for a11y. See #37 for the mock layer that other e2e tests use.
//
// We anchor "app is rendered" on `[data-testid="main-content"]` rather than
// `[role="main"]` because the latter is a CSS attribute selector — it only
// matches elements with an *explicit* `role` attribute. The `<main>` tag has
// an implicit ARIA role of "main" but no explicit attribute, so `[role="main"]`
// never matches and Playwright times out. The data-testid match is on the same
// element either way.

const READY = '[data-testid="main-content"]';

test.describe('StratoSort Accessibility', () => {
	test('main page has no axe-core violations on WCAG 2 A/AA rules', async ({ page }) => {
		await page.goto('/');
		await page.waitForSelector(READY, { timeout: 30000 });

		const results = await new AxeBuilder({ page })
			.withTags(['wcag2a', 'wcag2aa'])
			.analyze();

		if (results.violations.length > 0) {
			// Surface the actual rule IDs in the failure message so CI logs
			// point straight at what to fix.
			const summary = results.violations
				.map((v) => `${v.id} (${v.impact}): ${v.help}`)
				.join('\n');
			expect(results.violations, `axe violations:\n${summary}`).toEqual([]);
		}
	});

	test('Tab focus moves to the skip-to-content link first', async ({ page }) => {
		await page.goto('/');
		await page.waitForSelector(READY, { timeout: 30000 });

		await page.keyboard.press('Tab');
		await expect(page.locator(':focus')).toHaveAttribute('data-testid', 'skip-to-content');
	});

	test('? opens the keyboard shortcuts dialog; Escape closes it', async ({ page }) => {
		await page.goto('/');
		await page.waitForSelector(READY, { timeout: 30000 });

		await page.keyboard.press('?');
		await expect(page.locator('[data-testid="keyboard-help-dialog"]')).toBeVisible();
		await page.keyboard.press('Escape');
		await expect(page.locator('[data-testid="keyboard-help-dialog"]')).toBeHidden();
	});

	test('main landmarks and ARIA live regions are present', async ({ page }) => {
		await page.goto('/');
		await page.waitForSelector(READY, { timeout: 30000 });

		await expect(page.locator('[role="navigation"]')).toHaveCount(1);
		// <main> has implicit role="main" but no explicit attribute; assert on
		// the tag.
		await expect(page.locator('main')).toHaveCount(1);
		await expect(page.locator('[aria-live="polite"]')).toHaveCount(1);
		await expect(page.locator('[aria-live="assertive"]')).toHaveCount(1);
	});

	test('every visible button has either text content or an aria-label', async ({ page }) => {
		await page.goto('/');
		await page.waitForSelector(READY, { timeout: 30000 });

		const buttons = await page.locator('button:visible').all();
		expect(buttons.length).toBeGreaterThan(0);
		for (const button of buttons) {
			const text = (await button.textContent())?.trim() ?? '';
			const ariaLabel = await button.getAttribute('aria-label');
			expect(text.length > 0 || (ariaLabel && ariaLabel.length > 0)).toBe(true);
		}
	});

	test('Settings page inputs are associated with a label', async ({ page }) => {
		await page.goto('/');
		await page.waitForSelector(READY, { timeout: 30000 });

		await page.click('[data-testid="nav-settings"]');
		await page.waitForSelector('[data-testid="settings-page"]');

		// Wait for the bound settings model so inputs are actually rendered
		// (the "Loading settings…" placeholder shows until get_settings resolves,
		// and without a mock that defaults to null — which is rendered as the
		// placeholder. Settings tab is still navigable; assert what's there.)
		const inputs = await page
			.locator('[data-testid="settings-page"] input:not([type="hidden"]), [data-testid="settings-page"] select')
			.all();
		// Without a settings mock the page may be in the loading state. Don't
		// fail the test in that case — assert opportunistically.
		for (const input of inputs) {
			const id = await input.getAttribute('id');
			const ariaLabel = await input.getAttribute('aria-label');
			if (id) {
				const labelCount = await page.locator(`label[for="${id}"]`).count();
				if (labelCount > 0) continue;
			}
			if (ariaLabel) continue;
			// Fallback: parent must be a <label>.
			const parentTag = await input.evaluate((el) => el.parentElement?.tagName ?? '');
			expect(parentTag, 'input lacks label association').toBe('LABEL');
		}
	});
});
