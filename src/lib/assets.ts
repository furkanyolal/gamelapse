import { convertFileSrc } from "@tauri-apps/api/core";

const inTauri = () =>
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

/**
 * A replaced custom cover keeps its file path, so the webview would keep showing the cached
 * image. Every cover URL carries `?v=<nonce>-<counter>`: the nonce differs per launch and the
 * counter is bumped whenever a cover changes. The asset protocol ignores the query string.
 */
const nonce = Date.now().toString(36);
let version = 0;
const listeners = new Set<() => void>();

/** Call after a cover was set, replaced or cleared; mounted covers reload. */
export function bumpCoverVersion() {
  version += 1;
  listeners.forEach((l) => l());
}

export function subscribeCoverVersion(listener: () => void) {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

export function getCoverVersion() {
  return version;
}

/** Image source for a game's cover: URLs pass through, local files go through the asset protocol. */
export function coverSrc(coverPath: string | null): string | undefined {
  if (!coverPath) return undefined;
  if (/^(https?|data|blob):/i.test(coverPath)) return coverPath;
  return inTauri()
    ? `${convertFileSrc(coverPath)}?v=${nonce}-${version}`
    : undefined;
}
