import type { QueryClient } from "@tanstack/react-query";
import type { ImportSource, SessionQuery } from "@/api/types";

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
  autostart: ["autostart"] as const,
  updateCheck: ["update", "check"] as const,
  import: ["import"] as const,
  importSources: ["import", "sources"] as const,
  importScan: (source: ImportSource) => ["import", "scan", source] as const,
  steamApi: ["steamApi"] as const,
  steamApiSettings: ["steamApi", "settings"] as const,
  limits: ["limits"] as const,
  limitsList: ["limits", "list"] as const,
  tags: ["tags"] as const,
  tagsList: ["tags", "list"] as const,
};

type Family =
  | "tracker"
  | "games"
  | "sessions"
  | "stats"
  | "settings"
  | "import"
  | "steamApi"
  | "limits"
  | "tags";

export function invalidateFamilies(client: QueryClient, ...families: Family[]) {
  return Promise.all(
    families.map((f) => client.invalidateQueries({ queryKey: [f] })),
  );
}
