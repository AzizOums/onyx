import {
  DEFAULT_LUMEN_DOMAIN,
  CHROME_SPECIFIC_STORAGE_KEYS,
} from "./constants.js";

export async function getLumenDomain() {
  const result = await chrome.storage.local.get({
    [CHROME_SPECIFIC_STORAGE_KEYS.LUMEN_DOMAIN]: DEFAULT_LUMEN_DOMAIN,
  });
  return result[CHROME_SPECIFIC_STORAGE_KEYS.LUMEN_DOMAIN];
}

export function setLumenDomain(domain, callback) {
  chrome.storage.local.set(
    { [CHROME_SPECIFIC_STORAGE_KEYS.LUMEN_DOMAIN]: domain },
    callback
  );
}

export function getLumenDomainSync() {
  return new Promise((resolve) => {
    getLumenDomain(resolve);
  });
}
