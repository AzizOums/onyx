import { Tier } from "@/lib/settings/types";

export const TIER_RANK: Record<Tier, number> = {
  [Tier.COMMUNITY]: 0,
  [Tier.BUSINESS]: 1,
  [Tier.ENTERPRISE]: 2,
};

/**
 * Whether the LLM gateway is part of this build. It is not: the gateway API
 * ships separately from this source tree, so the settings tab and page stay
 * hidden rather than pointing at endpoints that do not exist.
 */
export const LLM_GATEWAY_AVAILABLE = false;

/**
 * Whether per-group token rate limits are part of this build. They are not:
 * the admin token-rate-limit API ships separately from this source tree, so the
 * section stays hidden rather than saving to endpoints that do not exist.
 */
export const TOKEN_RATE_LIMITS_AVAILABLE = false;

/**
 * Tier comparison. This build has no paid tiers: every feature it ships is
 * available to every workspace, so the check always passes. The `Tier` enum and
 * the ranking stay because the settings API still reports a tier.
 */
export function tierAtLeast(
  _current: Tier | undefined,
  _required: Tier
): boolean {
  return true;
}
