import type { Api } from "./api";
import { createMockApi } from "./mock";
import { createTauriApi } from "./tauri";

const inTauri =
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

/** The Tauri-backed api inside the app, the in-memory mock in a plain browser. */
export const api: Api = inTauri ? createTauriApi() : createMockApi();

export type { Api } from "./api";
export * from "./types";
