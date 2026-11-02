import { useQuery } from "@tanstack/react-query";
import { api } from "@/api";
import { keys } from "@/hooks/query-keys";

/** App version and on-disk locations, from the `get_app_info` command. */
export function useAppInfo() {
  return useQuery({
    queryKey: keys.appInfo,
    queryFn: () => api.getAppInfo(),
    staleTime: Infinity,
  });
}
