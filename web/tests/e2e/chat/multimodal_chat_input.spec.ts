import { expect, test } from "@playwright/test";
import { loginAsWorkerUser } from "@tests/e2e/utils/auth";

async function openChat(page: import("@playwright/test").Page) {
  await page.goto("/app");
  await page.waitForLoadState("networkidle");
  await expect(
    page.locator('input[type="file"]').first()
  ).toBeAttached({ timeout: 15000 });
}

async function switchToModel(page: import("@playwright/test").Page, group: string, model: string) {
  await page.getByTestId("model-selector").locator("button").last().click();
  await page.waitForTimeout(1500);
  const groupRow = page
    .locator('[role="dialog"]')
    .last()
    .getByText(group, { exact: false })
    .first();
  if ((await groupRow.count()) > 0) {
    await groupRow.click();
    await page.waitForTimeout(1200);
  }
  await page
    .locator('[role="dialog"]')
    .last()
    .getByRole("button")
    .filter({ hasText: model })
    .first()
    .click();
  await page.waitForTimeout(1500);
}

test.describe("Multimodal chat input adapts to model capabilities", () => {
  test.beforeEach(async ({ page }, testInfo) => {
    await page.context().clearCookies();
    await loginAsWorkerUser(page, testInfo.workerIndex);
  });

  test("file picker accept string follows the selected model", async ({
    page,
  }) => {
    await openChat(page);
    const input = page.locator('input[type="file"]').first();

    // Default model (multimodal, from models.dev): audio + video accepted
    const multimodal = await input.getAttribute("accept");
    expect(multimodal).toContain("audio/*");
    expect(multimodal).toContain("video/*");
    expect(multimodal).toContain("image/*");

    // Switch to a text-only model: audio/video dropped, docs kept
    await switchToModel(page, "Ollama local/Meta", "Llama3.1 8B");
    const textOnly = await page
      .locator('input[type="file"]')
      .first()
      .getAttribute("accept");
    expect(textOnly).not.toContain("audio/*");
    expect(textOnly).not.toContain("video/*");
    expect(textOnly).toContain("application/pdf");
  });

  test("audio upload is rejected with a toast on a text-only model", async ({
    page,
  }) => {
    await openChat(page);
    await switchToModel(page, "Ollama local/Meta", "Llama3.1 8B");
    await page
      .locator('input[type="file"]')
      .first()
      .setInputFiles("/tmp/test_audio.wav");
    await expect(
      page.getByText(/does not support audio input/i).first()
    ).toBeVisible({ timeout: 15000 });
  });
});
