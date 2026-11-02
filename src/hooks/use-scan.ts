import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api } from "@/api";
import type { ScanCandidate, ScanSelection } from "@/api";
import { invalidateFamilies, keys } from "@/hooks/query-keys";

export function useScanRoots() {
  return useQuery({
    queryKey: keys.scanRootsList,
    queryFn: () => api.listScanRoots(),
  });
}

/** Native folder dialog, then registers the pick; null when cancelled. */
export function useAddScanRoot() {
  const client = useQueryClient();
  return useMutation({
    mutationFn: async () => {
      const path = await api.pickFolder();
      return path === null ? null : api.addScanRoot(path);
    },
    onSuccess: () => invalidateFamilies(client, "scanRoots"),
  });
}

export function useRemoveScanRoot() {
  const client = useQueryClient();
  return useMutation({
    mutationFn: (rootId: number) => api.removeScanRoot(rootId),
    onSuccess: () => invalidateFamilies(client, "scanRoots"),
  });
}

/** Runs only when mutate is called; the result is kept under ["scan", "candidates"]. */
export function useScanFolders() {
  const client = useQueryClient();
  return useMutation({
    mutationFn: (rootIds: number[] | null) => api.scanFolders(rootIds),
    onSuccess: (candidates) =>
      client.setQueryData<ScanCandidate[]>(keys.scanCandidates, candidates),
  });
}

export function useAddScannedGames() {
  const client = useQueryClient();
  return useMutation({
    mutationFn: (selection: ScanSelection[]) => api.addScannedGames(selection),
    onSuccess: () => invalidateFamilies(client, "games", "stats", "import"),
  });
}
