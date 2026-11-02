import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api } from "@/api";
import { invalidateFamilies, keys } from "@/hooks/query-keys";

export function useTags() {
  return useQuery({
    queryKey: keys.tagsList,
    queryFn: () => api.listTags(),
  });
}

function useTagMutation<TVars, TData>(fn: (vars: TVars) => Promise<TData>) {
  const client = useQueryClient();
  return useMutation({
    mutationFn: fn,
    onSuccess: () => invalidateFamilies(client, "tags", "games"),
  });
}

export function useCreateTag() {
  return useTagMutation((name: string) => api.createTag(name));
}

export function useRenameTag() {
  return useTagMutation((vars: { tagId: number; name: string }) =>
    api.renameTag(vars.tagId, vars.name),
  );
}

export function useDeleteTag() {
  return useTagMutation((tagId: number) => api.deleteTag(tagId));
}

export function useSetGameTags(gameId: number) {
  return useTagMutation((tagIds: number[]) => api.setGameTags(gameId, tagIds));
}
