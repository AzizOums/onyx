import { expect, test } from "@playwright/test";
import { loginAsWorkerUser } from "@tests/e2e/utils/auth";
import {
  startNewChat,
  verifyDefaultAgentIsChosen,
} from "@tests/e2e/utils/chatActions";

test.describe("FLOSS build — real LLM chat via Ollama", () => {
  test.beforeEach(async ({ page }, testInfo) => {
    await page.context().clearCookies();
    await loginAsWorkerUser(page, testInfo.workerIndex);
  });

  test("sends a message in the UI and receives an LLM answer", async ({
    page,
  }) => {
    test.setTimeout(180_000);
    await page.goto("/app");
    await page.waitForLoadState("networkidle");

    // Fresh chat with the default agent
    await startNewChat(page);
    await verifyDefaultAgentIsChosen(page);

    // Send a real message — answered by Ollama (llama3.1:8b) locally.
    // Local 8B models can take >60s on first load, so no fixed short waits.
    await page.locator("#lumen-chat-input-textbox").fill("Reply with exactly: FLOSS-OK");
    await page.locator("#lumen-chat-input-send-button").click();

    const assistantAnswer = page
      .locator('[data-testid="lumen-ai-message"]')
      .last();
    await expect(assistantAnswer).toBeVisible({ timeout: 150_000 });
    await expect(assistantAnswer).toContainText("FLOSS-OK", {
      timeout: 150_000,
    });

    // Screenshot as visual proof
    await page.screenshot({
      path: "output/playwright/floss-ollama-chat.png",
      fullPage: true,
    });
  });
});
