import { convertFileSrc } from "@tauri-apps/api/core";

const inTauri = () =>
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

/** Image source for a game's cover: URLs pass through, local files go through the asset protocol. */
export function coverSrc(coverPath: string | null): string | undefined {
  if (!coverPath) return undefined;
  if (/^(https?|data|blob):/i.test(coverPath)) return coverPath;
  return inTauri() ? convertFileSrc(coverPath) : undefined;
}
