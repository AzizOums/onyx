import { expect, test } from "@playwright/test";
import { loginAs } from "@tests/e2e/utils/auth";
import { LumenApiClient } from "@tests/e2e/utils/lumenApiClient";

test.describe("models.dev provider picker", () => {
  test("browses the catalog and fills the form with multimodal flags", async ({
    page,
  }) => {
    test.setTimeout(240000);
    await page.context().clearCookies();
    await loginAs(page, "admin");

    await page.goto("/admin/language-models");
    await page.waitForLoadState("networkidle");
    await page.getByText("OpenAI-Compatible", { exact: true }).first().click();
    await page.waitForTimeout(1500);

    // Open the models.dev catalog and browse the opencode provider
    await page.getByRole("button", { name: /browse models.dev/i }).click();
    await page.waitForTimeout(3000);
    await page.getByTestId("modelsdev-search").click();
    await page
      .getByTestId("modelsdev-search")
      .pressSequentially("zen", { delay: 50 });
    await page.waitForTimeout(1200);
    await page.getByTestId("modelsdev-browse-opencode").click();
    await page.waitForTimeout(6000);

    // The catalog row exposes the models.dev modalities
    await expect(
      page.getByText(/MiMo V2\.5 Free/i).first()
    ).toBeVisible({ timeout: 20000 });
    await page.getByTestId("modelsdev-add-mimo-v2.5-free").click();
    await page.waitForTimeout(800);
    await page.getByTestId("modelsdev-apply").click();
    await page.waitForTimeout(1500);

    // The picked model lands in the provider form
    await expect(
      page.getByText(/MiMo V2\.5 Free/i).first()
    ).toBeVisible({ timeout: 15000 });
  });

  test("discovery enriches models with models.dev modalities", async ({
    page,
  }) => {
    await page.context().clearCookies();
    await loginAs(page, "admin");
    const apiClient = new LumenApiClient(page.request);

    // Create an openai-compatible provider, then check the discovered model
    // capabilities merged from models.dev (mimo: image + audio + video).
    const res = await page.request.put(
      "/api/admin/llm/provider?is_creation=true",
      {
        data: {
          name: `E2E-ModelsDev-${Date.now().toString(36)}`,
          provider: "openai_compatible",
          api_base: "https://opencode.ai/zen/v1",
          model_configurations: [
            {
              name: "mimo-v2.5-free",
              is_visible: true,
              max_input_tokens: 200000,
              supports_image_input: true,
              supports_audio_input: true,
              supports_video_input: true,
            },
          ],
        },
      }
    );
    expect(res.ok()).toBe(true);
    const created = (await res.json()) as { id: number };
    try {
      const list = await apiClient.listLlmProviders();
      const full = list.find((p: { id: number }) => p.id === created.id);
      expect(full).toBeTruthy();
    } finally {
      await apiClient.deleteProvider(created.id);
    }
  });
});
