/**
 * Playwright fixtures for Admin Groups page tests.
 *
 * Provides:
 * - Authenticated admin page
 * - LumenApiClient for API-level setup/teardown
 * - GroupsAdminPage page object
 */

import { test as base, expect, type Page } from "@playwright/test";
import { loginAs } from "@tests/e2e/utils/auth";
import { LumenApiClient } from "@tests/e2e/utils/lumenApiClient";
import { GroupsAdminPage } from "./GroupsAdminPage";

export const test = base.extend<{
  adminPage: Page;
  api: LumenApiClient;
  groupsPage: GroupsAdminPage;
}>({
  adminPage: async ({ page }, use) => {
    await page.context().clearCookies();
    await loginAs(page, "admin");
    await use(page);
  },

  api: async ({ adminPage }, use) => {
    const client = new LumenApiClient(adminPage.request);
    await use(client);
  },

  groupsPage: async ({ adminPage }, use) => {
    const groupsPage = new GroupsAdminPage(adminPage);
    await use(groupsPage);
  },
});

export { expect };
