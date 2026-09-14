export const THEMES = {
  LIGHT: "light",
  DARK: "dark",
};

export const DEFAULT_LUMEN_DOMAIN = "http://localhost:3000";

export const SIDE_PANEL_PATH = "/nrf/side-panel";

export const ACTIONS = {
  GET_SELECTED_TEXT: "getSelectedText",
  GET_CURRENT_LUMEN_DOMAIN: "getCurrentLumenDomain",
  UPDATE_PAGE_URL: "updatePageUrl",
  SEND_TO_LUMEN: "sendToLumen",
  OPEN_SIDE_PANEL: "openSidePanel",
  TOGGLE_NEW_TAB_OVERRIDE: "toggleNewTabOverride",
  OPEN_SIDE_PANEL_WITH_INPUT: "openSidePanelWithInput",
  OPEN_LUMEN_WITH_INPUT: "openLumenWithInput",
  CLOSE_SIDE_PANEL: "closeSidePanel",
  TAB_URL_UPDATED: "tabUrlUpdated",
  TAB_READING_ENABLED: "tabReadingEnabled",
  TAB_READING_DISABLED: "tabReadingDisabled",
};

export const CHROME_SPECIFIC_STORAGE_KEYS = {
  LUMEN_DOMAIN: "lumenExtensionDomain",
  USE_LUMEN_AS_DEFAULT_NEW_TAB: "lumenExtensionDefaultNewTab",
  THEME: "lumenExtensionTheme",
  BACKGROUND_IMAGE: "lumenExtensionBackgroundImage",
  DARK_BG_URL: "lumenExtensionDarkBgUrl",
  LIGHT_BG_URL: "lumenExtensionLightBgUrl",
  ONBOARDING_COMPLETE: "lumenExtensionOnboardingComplete",
};

export const CHROME_MESSAGE = {
  PREFERENCES_UPDATED: "PREFERENCES_UPDATED",
  LUMEN_APP_LOADED: "LUMEN_APP_LOADED",
  SET_DEFAULT_NEW_TAB: "SET_DEFAULT_NEW_TAB",
  LOAD_NEW_CHAT_PAGE: "LOAD_NEW_CHAT_PAGE",
  LOAD_NEW_PAGE: "LOAD_NEW_PAGE",
  AUTH_REQUIRED: "AUTH_REQUIRED",
  TAB_READING_ENABLED: "TAB_READING_ENABLED",
  TAB_READING_DISABLED: "TAB_READING_DISABLED",
  TAB_URL_UPDATED: "TAB_URL_UPDATED",
};

export const WEB_MESSAGE = {
  PAGE_CHANGE: "PAGE_CHANGE",
};
