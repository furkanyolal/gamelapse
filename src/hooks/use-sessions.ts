import {
  keepPreviousData,
  useMutation,
  useQuery,
  useQueryClient,
} from "@tanstack/react-query";
import { api } from "@/api";
import type { SessionQuery, SessionUpdate } from "@/api";
import { invalidateFamilies, keys } from "@/hooks/query-keys";

export function useSessions(query: SessionQuery) {
  return useQuery({
    queryKey: keys.sessionsList(query),
    queryFn: () => api.listSessions(query),
    placeholderData: keepPreviousData,
  });
}

function useSessionMutation<TVars, TData>(fn: (vars: TVars) => Promise<TData>) {
  const client = useQueryClient();
  return useMutation({
    mutationFn: fn,
    onSuccess: () => invalidateFamilies(client, "sessions", "games", "stats"),
  });
}

export function useCreateSession() {
  return useSessionMutation(
    (v: {
      gameId: number;
      startedAt: number;
      endedAt: number;
      note: string | null;
    }) => api.createSession(v.gameId, v.startedAt, v.endedAt, v.note),
  );
}

export function useUpdateSession() {
  return useSessionMutation((v: { sessionId: number; patch: SessionUpdate }) =>
    api.updateSession(v.sessionId, v.patch),
  );
}

export function useDeleteSession() {
  return useSessionMutation((sessionId: number) =>
    api.deleteSession(sessionId),
  );
}
