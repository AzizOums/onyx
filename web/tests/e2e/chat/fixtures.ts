/**
 * Playwright fixtures for chat page tests.
 *
 * Provides:
 * - Authenticated worker-user page
 * - LumenApiClient for API-level setup/teardown
 * - ChatPage page object (with inputBar sub-object)
 */

import { test as base, expect } from "@playwright/test";
import { loginAsWorkerUser } from "@tests/e2e/utils/auth";
import { LumenApiClient } from "@tests/e2e/utils/lumenApiClient";
import { ChatPage } from "@tests/e2e/chat/ChatPage";

export const test = base.extend<{
  api: LumenApiClient;
  chatPage: ChatPage;
}>({
  chatPage: async ({ page }, use, testInfo) => {
    await page.context().clearCookies();
    await loginAsWorkerUser(page, testInfo.workerIndex);
    const chatPage = new ChatPage(page);
    await use(chatPage);
  },

  api: async ({ chatPage }, use) => {
    const client = new LumenApiClient(chatPage.page.request);
    await use(client);
  },
});

export { expect };
