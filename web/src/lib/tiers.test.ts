import { Tier } from "@/lib/settings/types";
import { LLM_GATEWAY_AVAILABLE, tierAtLeast } from "@/lib/tiers";

describe("tiers", () => {
  it("grants every feature on every tier", () => {
    for (const current of [Tier.COMMUNITY, Tier.BUSINESS, Tier.ENTERPRISE]) {
      for (const required of [Tier.COMMUNITY, Tier.BUSINESS, Tier.ENTERPRISE]) {
        expect(tierAtLeast(current, required)).toBe(true);
      }
    }
    expect(tierAtLeast(undefined, Tier.ENTERPRISE)).toBe(true);
  });

  it("keeps the LLM gateway out of this build", () => {
    expect(LLM_GATEWAY_AVAILABLE).toBe(false);
  });
});
