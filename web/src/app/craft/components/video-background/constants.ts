// Empty by default: this build ships no CDN of its own. Set
// NEXT_PUBLIC_VIDEO_BACKGROUND_SRC to a reachable mp4 to enable the Craft
// animated background; the component renders nothing while it is empty.
export const VIDEO_BACKGROUND_SRC =
  process.env.NEXT_PUBLIC_VIDEO_BACKGROUND_SRC || "";
export const VIDEO_BACKGROUND_STORAGE_KEY = "craft-video-background";
export const VIDEO_BACKGROUND_CLICK_COUNT = 7;
export const VIDEO_BACKGROUND_CLICK_RESET_MS = 1500;
