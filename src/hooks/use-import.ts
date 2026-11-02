import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api } from "@/api";
import type { GameSource, ImportSource, SteamApiSettings } from "@/api";
import { invalidateFamilies, keys } from "@/hooks/query-keys";

export function useImportSources() {
  return useQuery({
    queryKey: keys.importSources,
    queryFn: () => api.listImportSources(),
  });
}

/** Preview of a source; fetched on demand, never cached as fresh. */
export function useScanImportSource(source: ImportSource | null) {
  return useQuery({
    queryKey: keys.importScan(source ?? "steam"),
    queryFn: () => api.scanImportSource(source!),
    enabled: false,
    gcTime: 0,
  });
}

export function useImportGames() {
  const client = useQueryClient();
  return useMutation({
    mutationFn: (source: ImportSource) => api.importGames(source),
    onSuccess: () => invalidateFamilies(client, "games", "stats", "import"),
  });
}

export function useIgnoredImports() {
  return useQuery({
    queryKey: keys.importIgnored,
    queryFn: () => api.listIgnoredImports(),
  });
}

/** Un-ignores a game, then imports its source so it comes back. */
export function useRestoreIgnoredImport() {
  const client = useQueryClient();
  return useMutation({
    mutationFn: async (v: {
      source: GameSource;
      sourceId: string;
      importSource: ImportSource;
    }) => {
      await api.restoreIgnoredImport(v.source, v.sourceId);
      return api.importGames(v.importSource);
    },
    onSettled: () => invalidateFamilies(client, "import", "games", "stats"),
  });
}

export function useSteamApiSettings() {
  return useQuery({
    queryKey: keys.steamApiSettings,
    queryFn: () => api.getSteamApiSettings(),
  });
}

export function useSetSteamApiSettings() {
  const client = useQueryClient();
  return useMutation({
    mutationFn: (settings: SteamApiSettings) =>
      api.setSteamApiSettings(settings),
    onSuccess: () => invalidateFamilies(client, "steamApi"),
  });
}

export function useFetchSteamPlaytime() {
  const client = useQueryClient();
  return useMutation({
    mutationFn: () => api.fetchSteamPlaytime(),
    onSuccess: () => invalidateFamilies(client, "games", "stats", "import"),
  });
}
