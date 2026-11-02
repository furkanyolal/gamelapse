import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api } from "@/api";
import type { MetadataMatch } from "@/api";
import { invalidateFamilies, keys } from "@/hooks/query-keys";
import { bumpCoverVersion } from "@/lib/assets";

/** Never fetches by itself; call refetch from a user action. */
export function useLookupGameMetadata(name: string) {
  return useQuery({
    queryKey: keys.metadata(name),
    queryFn: () => api.lookupGameMetadata(name),
    enabled: false,
    retry: false,
    staleTime: 5 * 60_000,
  });
}

export function useApplyGameMetadata(gameId: number) {
  const client = useQueryClient();
  return useMutation({
    mutationFn: (vars: { match: MetadataMatch; renameGame: boolean }) =>
      api.applyGameMetadata(gameId, vars.match, vars.renameGame),
    onSuccess: () => {
      bumpCoverVersion();
      return invalidateFamilies(client, "games", "stats", "import");
    },
  });
}
