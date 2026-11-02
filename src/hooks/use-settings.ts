import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api } from "@/api";
import type { SettingsUpdate } from "@/api";
import { invalidateFamilies, keys } from "@/hooks/query-keys";

export function useSettings() {
  return useQuery({
    queryKey: keys.settings,
    queryFn: () => api.getSettings(),
  });
}

export function useUpdateSettings() {
  const client = useQueryClient();
  return useMutation({
    mutationFn: (patch: Partial<SettingsUpdate>) =>
      api.updateSettings({
        pollIntervalSecs: null,
        afkThresholdSecs: null,
        sessionMergeGapSecs: null,
        requireForeground: null,
        trackingEnabled: null,
        theme: null,
        importOnStartup: null,
        ...patch,
      }),
    onSuccess: (settings) => {
      client.setQueryData(keys.settings, settings);
      return invalidateFamilies(client, "tracker");
    },
  });
}

export function useSeedDemoData() {
  const client = useQueryClient();
  return useMutation({
    mutationFn: () => api.seedDemoData(),
    onSuccess: () => invalidateFamilies(client, "games", "sessions", "stats"),
  });
}

export function usePing() {
  return useMutation({ mutationFn: () => api.ping() });
}
