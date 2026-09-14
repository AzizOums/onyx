/**
 * Always true: this build ships no billing, so no feature sits behind a
 * subscription. Kept as a hook so call sites stay unchanged.
 */
export function useCloudSubscription(): boolean {
  return true;
}
