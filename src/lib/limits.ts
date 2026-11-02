import type { LimitStatus } from "@/api";

/** Display name of what a limit applies to. */
export function limitTitle(status: LimitStatus): string {
  return status.limit.scope === "global"
    ? "All games"
    : (status.limit.gameName ?? "Unknown game");
}
