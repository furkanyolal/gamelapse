import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api } from "@/api";
import type { GameUpdate, MatchMode } from "@/api";
import { invalidateFamilies, keys } from "@/hooks/query-keys";
import { bumpCoverVersion } from "@/lib/assets";

export function useGames(includeHidden: boolean) {
  return useQuery({
    queryKey: keys.gamesList(includeHidden),
    queryFn: () => api.listGames(includeHidden),
  });
}

export function useGame(gameId: number) {
  return useQuery({
    queryKey: keys.gameDetail(gameId),
    queryFn: () => api.getGame(gameId),
    enabled: Number.isFinite(gameId),
  });
}

export function useGameExecutables(gameId: number) {
  return useQuery({
    queryKey: keys.gameExecutables(gameId),
    queryFn: () => api.listGameExecutables(gameId),
    enabled: Number.isFinite(gameId),
  });
}

function useLibraryMutation<TVars, TData>(fn: (vars: TVars) => Promise<TData>) {
  const client = useQueryClient();
  return useMutation({
    mutationFn: fn,
    onSuccess: () => invalidateFamilies(client, "games", "sessions", "stats"),
  });
}

function useManagementMutation<TVars, TData>(
  fn: (vars: TVars) => Promise<TData>,
) {
  const client = useQueryClient();
  return useMutation({
    mutationFn: fn,
    onSuccess: () =>
      invalidateFamilies(client, "games", "sessions", "stats", "limits"),
  });
}

export function useMergeGames(survivorId: number) {
  return useManagementMutation((mergedIds: number[]) =>
    api.mergeGames(survivorId, mergedIds),
  );
}

/** Picks an image with the native dialog, then sets it as the cover. */
export function useChooseCover(gameId: number) {
  return useManagementMutation(async () => {
    const path = await api.pickImage();
    if (path === null) return null;
    const game = await api.setGameCover(gameId, path);
    bumpCoverVersion();
    return game;
  });
}

export function useClearCover(gameId: number) {
  return useManagementMutation(async () => {
    const game = await api.setGameCover(gameId, null);
    bumpCoverVersion();
    return game;
  });
}

/** Picks an executable with the native dialog, then creates the game. */
export function useAddGameFromExe() {
  return useLibraryMutation(async (name: string | null) => {
    const path = await api.pickExecutable();
    return path === null ? null : api.addGameFromExe(path, name);
  });
}

export function useUpdateGame(gameId: number) {
  return useLibraryMutation((patch: Partial<GameUpdate>) =>
    api.updateGame(gameId, {
      name: null,
      hidden: null,
      afkDetectionEnabled: null,
      ...patch,
    }),
  );
}

export function useDeleteGame() {
  return useLibraryMutation((gameId: number) => api.deleteGame(gameId));
}

/** Picks an executable with the native dialog, then attaches it. */
export function useAddGameExecutable(gameId: number) {
  return useLibraryMutation(async (matchMode: MatchMode) => {
    const path = await api.pickExecutable();
    return path === null
      ? null
      : api.addGameExecutable(gameId, path, matchMode);
  });
}

export function useRemoveGameExecutable() {
  return useLibraryMutation((executableId: number) =>
    api.removeGameExecutable(executableId),
  );
}
