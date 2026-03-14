import { test, expect } from '@playwright/test';
import { waitForWasm } from '../fixtures/helpers';

test.describe('Navigation and UI', () => {
  test('navbar brand navigates back to home page', async ({ page }) => {
    await page.goto('/d/00000000-0000-0000-0000-000000000000');
    await waitForWasm(page, '.error');

    await page.locator('.navbar-brand').click();
    await waitForWasm(page, '.mode-selector');
    expect(page.url()).toMatch(/\/$/);
  });

  test('/d/:fake-id shows error with not-found link', async ({ page }) => {
    await page.goto('/d/00000000-0000-0000-0000-000000000000');
    await waitForWasm(page, '.error');
    await expect(page.locator('.error')).toBeVisible();
    await expect(page.locator('.not-found-link')).toBeVisible();
  });

  test('/receive/:any-id renders the receive status card', async ({ page }) => {
    // Even with a non-existent session the page structure should render
    await page.goto('/receive/00000000-0000-0000-0000-000000000000');
    await waitForWasm(page, '.receive-status-card');
    await expect(page.locator('.receive-status-card')).toBeVisible();
  });

  test('theme toggle switches theme and persists across navigation', async ({
    page,
  }) => {
    await page.goto('/');
    await waitForWasm(page, '.mode-selector');

    // Read initial theme
    const initialTheme =
      (await page.evaluate(() =>
        document.documentElement.getAttribute('data-theme'),
      )) ?? 'dark';

    // Click theme toggle
    await page.locator('.theme-toggle').click();

    // Theme should have changed
    const newTheme = await page.evaluate(() =>
      document.documentElement.getAttribute('data-theme'),
    );
    expect(newTheme).not.toBe(initialTheme);

    // localStorage should be updated
    const stored = await page.evaluate(() =>
      localStorage.getItem('hermes-theme'),
    );
    expect(stored).toBe(newTheme);

    // After navigation the theme should persist
    await page.goto('/');
    await waitForWasm(page, '.mode-selector');
    const persistedTheme = await page.evaluate(() =>
      document.documentElement.getAttribute('data-theme'),
    );
    expect(persistedTheme).toBe(newTheme);
  });

  test('home page has two mode buttons', async ({ page }) => {
    await page.goto('/');
    await waitForWasm(page, '.mode-selector');
    await expect(page.locator('.mode-btn')).toHaveCount(2);
  });
});
