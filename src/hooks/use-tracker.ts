import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api } from "@/api";
import { invalidateFamilies, keys } from "@/hooks/query-keys";

export function useTrackerStatus() {
  return useQuery({
    queryKey: keys.trackerStatus,
    queryFn: () => api.getTrackerStatus(),
  });
}

export function useSetTrackingEnabled() {
  const client = useQueryClient();
  return useMutation({
    mutationFn: (enabled: boolean) => api.setTrackingEnabled(enabled),
    onSuccess: () => invalidateFamilies(client, "tracker", "settings"),
  });
}

export function useStartManualSession() {
  const client = useQueryClient();
  return useMutation({
    mutationFn: (gameId: number) => api.startManualSession(gameId),
    onSuccess: () =>
      invalidateFamilies(client, "tracker", "games", "sessions", "stats"),
  });
}

export function useStopSession() {
  const client = useQueryClient();
  return useMutation({
    mutationFn: (sessionId: number) => api.stopSession(sessionId),
    onSuccess: () =>
      invalidateFamilies(client, "tracker", "games", "sessions", "stats"),
  });
}
