import type { QueryClient } from "@tanstack/react-query";
import type { SessionQuery } from "@/api/types";

/** Query keys from docs/contract.md; arguments go last. */
export const keys = {
  tracker: ["tracker"] as const,
  trackerStatus: ["tracker", "status"] as const,
  games: ["games"] as const,
  gamesList: (includeHidden: boolean) =>
    ["games", "list", { includeHidden }] as const,
  gameDetail: (gameId: number) => ["games", "detail", gameId] as const,
  gameExecutables: (gameId: number) =>
    ["games", "executables", gameId] as const,
  sessions: ["sessions"] as const,
  sessionsList: (query: SessionQuery) => ["sessions", "list", query] as const,
  stats: ["stats"] as const,
  statsDashboard: ["stats", "dashboard"] as const,
  statsDaily: (days: number, gameId: number | null) =>
    ["stats", "daily", days, gameId] as const,
  statsGames: (from: number | null, to: number | null, limit: number) =>
    ["stats", "games", from, to, limit] as const,
  statsHourly: (days: number) => ["stats", "hourly", days] as const,
  settings: ["settings"] as const,
  appInfo: ["app", "info"] as const,
};

type Family = "tracker" | "games" | "sessions" | "stats" | "settings";

export function invalidateFamilies(client: QueryClient, ...families: Family[]) {
  return Promise.all(
    families.map((f) => client.invalidateQueries({ queryKey: [f] })),
  );
}
