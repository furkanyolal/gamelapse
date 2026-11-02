import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api } from "@/api";
import type { LimitInput } from "@/api";
import { invalidateFamilies, keys } from "@/hooks/query-keys";

export function useLimits() {
  return useQuery({
    queryKey: keys.limitsList,
    queryFn: () => api.listLimits(),
  });
}

function useLimitMutation<TVars, TData>(fn: (vars: TVars) => Promise<TData>) {
  const client = useQueryClient();
  return useMutation({
    mutationFn: fn,
    onSuccess: () => invalidateFamilies(client, "limits"),
  });
}

export function useCreateLimit() {
  return useLimitMutation((input: LimitInput) => api.createLimit(input));
}

export function useUpdateLimit() {
  return useLimitMutation((vars: { limitId: number; input: LimitInput }) =>
    api.updateLimit(vars.limitId, vars.input),
  );
}

export function useDeleteLimit() {
  return useLimitMutation((limitId: number) => api.deleteLimit(limitId));
}
