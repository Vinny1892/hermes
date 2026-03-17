import { test, expect } from '@playwright/test';
import { loginViaUi, logout, waitForWasm, E2E_ADMIN_EMAIL, E2E_ADMIN_PASSWORD } from '../fixtures/helpers';

test.describe('Authentication', () => {
  // ── Login page ──────────────────────────────────────────────────────────────

  test('unauthenticated visit to / redirects to /login', async ({ page }) => {
    // Ensure no token is present
    await page.goto('/login');
    await page.evaluate(() => {
      localStorage.removeItem('hermes-token');
      localStorage.removeItem('hermes-role');
    });

    await page.goto('/');
    await page.waitForURL('**/login', { timeout: 10_000 });
  });

  test('unauthenticated visit to protected route redirects to /login', async ({ page }) => {
    await page.goto('/login');
    await page.evaluate(() => {
      localStorage.removeItem('hermes-token');
      localStorage.removeItem('hermes-role');
    });

    await page.goto('/settings');
    await page.waitForURL('**/login', { timeout: 10_000 });
  });

  test('successful login redirects to home and renders app', async ({ page }) => {
    await loginViaUi(page);
    expect(page.url()).toMatch(/\/$/);
    await waitForWasm(page, '.mode-selector');
    await expect(page.locator('.mode-selector')).toBeVisible();
  });

  test('wrong credentials show user-friendly error message', async ({ page }) => {
    await page.goto('/login');
    await page.waitForSelector('html[data-wasm-ready="true"]', { timeout: 30_000 });
    await page.fill('#l-email', 'nobody@example.com');
    await page.fill('#l-pass', 'wrongpassword');
    await page.locator('.login-btn').click();

    const error = page.locator('.login-error');
    await expect(error).toBeVisible({ timeout: 10_000 });
    await expect(error).toContainText('invalid email or password');
    // Must not expose internal Rust/framework error details
    await expect(error).not.toContainText('ServerFnError');
    await expect(error).not.toContainText('error running server function');
  });

  test('wrong password for existing email shows same error (no user enumeration)', async ({
    page,
  }) => {
    await page.goto('/login');
    await page.waitForSelector('html[data-wasm-ready="true"]', { timeout: 30_000 });
    await page.fill('#l-email', E2E_ADMIN_EMAIL);
    await page.fill('#l-pass', 'definitely-wrong');
    await page.locator('.login-btn').click();

    const error = page.locator('.login-error');
    await expect(error).toBeVisible({ timeout: 10_000 });
    await expect(error).toContainText('invalid email or password');
  });

  test('empty fields show validation error without making a server call', async ({
    page,
  }) => {
    await page.goto('/login');
    await page.waitForSelector('html[data-wasm-ready="true"]', { timeout: 30_000 });
    // Submit without filling anything
    await page.locator('.login-btn').click();

    const error = page.locator('.login-error');
    await expect(error).toBeVisible({ timeout: 5_000 });
    await expect(error).toContainText('required');
  });

  // ── Redirect when already logged in ────────────────────────────────────────

  test('/login redirects to / when already authenticated', async ({ page }) => {
    await loginViaUi(page);
    await page.goto('/login');
    await page.waitForURL(/\/$/, { timeout: 10_000 });
  });

  // ── Logout ──────────────────────────────────────────────────────────────────

  test('logout clears token and redirects to /login', async ({ page }) => {
    await loginViaUi(page);
    await waitForWasm(page, '.mode-selector');

    await logout(page);

    const token = await page.evaluate(() => localStorage.getItem('hermes-token'));
    const role = await page.evaluate(() => localStorage.getItem('hermes-role'));
    expect(token).toBeNull();
    expect(role).toBeNull();
  });

  test('after logout, navigating to / redirects to /login', async ({ page }) => {
    await loginViaUi(page);
    await waitForWasm(page, '.mode-selector');
    await logout(page);

    await page.goto('/');
    await page.waitForURL('**/login', { timeout: 10_000 });
  });

  // ── Admin role ──────────────────────────────────────────────────────────────

  test('admin user sees settings gear icon in navbar', async ({ page }) => {
    await loginViaUi(page, E2E_ADMIN_EMAIL, E2E_ADMIN_PASSWORD);
    await waitForWasm(page, '.mode-selector');
    await expect(page.locator('[title="Settings"]')).toBeVisible();
  });

  test('admin can navigate to /settings', async ({ page }) => {
    await loginViaUi(page, E2E_ADMIN_EMAIL, E2E_ADMIN_PASSWORD);
    await waitForWasm(page, '.mode-selector');
    await page.locator('[title="Settings"]').click();
    await page.waitForURL('**/settings', { timeout: 10_000 });
  });
});
