/**
 * Auth setup — runs once before all authenticated tests.
 * Logs in via the UI and saves localStorage state (token + role) to
 * STORAGE_STATE so that subsequent tests can skip the login step.
 */
import { test as setup } from '@playwright/test';
import { loginViaUi, STORAGE_STATE } from '../fixtures/helpers';

setup('authenticate as admin', async ({ page }) => {
  await loginViaUi(page);
  await page.context().storageState({ path: STORAGE_STATE });
});
