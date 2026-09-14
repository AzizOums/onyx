/**
 * Lumen Chat Widget - Entry Point
 * Exports the main web component
 */

import { LumenChatWidget } from "./widget";

// Define the custom element
if (
  typeof customElements !== "undefined" &&
  !customElements.get("lumen-chat-widget")
) {
  customElements.define("lumen-chat-widget", LumenChatWidget);
}

// Export for use in other modules
export { LumenChatWidget };
export * from "./types/api-types";
export * from "./types/widget-types";
