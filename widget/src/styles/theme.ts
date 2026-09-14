import { css } from "lit";
import { colors } from "./colors";

/**
 * Lumen Design System - Theme
 * Typography, spacing, and layout tokens from Figma
 */
export const theme = css`
  ${colors}

  :host {
    /* Typography - Hanken Grotesk */
    --lumen-font-family:
      "Hanken Grotesk", -apple-system, BlinkMacSystemFont, "Segoe UI",
      sans-serif;
    --lumen-font-family-mono: "DM Mono", "Monaco", "Menlo", monospace;

    /* Font Sizes */
    --lumen-font-size-small: 10px;
    --lumen-font-size-secondary: 12px;
    --lumen-font-size-sm: 13px;
    --lumen-font-size-main: 14px;
    --lumen-font-size-label: 16px;

    /* Line Heights */
    --lumen-line-height-small: 12px;
    --lumen-line-height-secondary: 16px;
    --lumen-line-height-main: 20px;
    --lumen-line-height-label: 24px;
    --lumen-line-height-section: 28px;
    --lumen-line-height-headline: 36px;

    /* Font Weights */
    --lumen-weight-regular: 400;
    --lumen-weight-medium: 500;
    --lumen-weight-semibold: 600;

    /* Content Heights */
    --lumen-height-content-secondary: 12px;
    --lumen-height-content-main: 16px;
    --lumen-height-content-label: 18px;
    --lumen-height-content-section: 24px;

    /* Border Radius - from Figma */
    --lumen-radius-04: 4px;
    --lumen-radius-08: 8px;
    --lumen-radius-12: 12px;
    --lumen-radius-16: 16px;
    --lumen-radius-round: 1000px;

    /* Spacing - Block */
    --lumen-space-block-1x: 4px;
    --lumen-space-block-2x: 8px;
    --lumen-space-block-3x: 12px;
    --lumen-space-block-4x: 16px;
    --lumen-space-block-6x: 24px;

    /* Spacing - Inline */
    --lumen-space-inline-0: 0px;
    --lumen-space-inline-0_5x: 2px;
    --lumen-space-inline-1x: 4px;

    /* Legacy spacing aliases (for compatibility) */
    --lumen-space-2xs: var(--lumen-space-block-1x);
    --lumen-space-xs: var(--lumen-space-block-2x);
    --lumen-space-sm: var(--lumen-space-block-3x);
    --lumen-space-md: var(--lumen-space-block-4x);
    --lumen-space-lg: var(--lumen-space-block-6x);

    /* Padding */
    --lumen-padding-icon-0: 0px;
    --lumen-padding-icon-0_5x: 2px;
    --lumen-padding-text-0_5x: 2px;
    --lumen-padding-text-1x: 4px;

    /* Icon Weights (stroke-width) */
    --lumen-icon-weight-secondary: 1px;
    --lumen-icon-weight-main: 1.5px;
    --lumen-icon-weight-section: 2px;

    /* Z-index */
    --lumen-z-launcher: 9999;
    --lumen-z-widget: 10000;

    /* Transitions */
    --lumen-transition-fast: 150ms cubic-bezier(0.4, 0, 0.2, 1);
    --lumen-transition-base: 200ms cubic-bezier(0.4, 0, 0.2, 1);
  }

  * {
    box-sizing: border-box;
  }
`;
