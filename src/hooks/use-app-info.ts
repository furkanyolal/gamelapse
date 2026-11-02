import { useQuery } from "@tanstack/react-query";
import { commands, type AppInfo } from "@/bindings";

export const appInfoQueryKey = ["app-info"] as const;

async function fetchAppInfo(): Promise<AppInfo> {
  const result = await commands.getAppInfo();
  if (result.status === "error") {
    throw new Error(result.error);
  }
  return result.data;
}

/** App version and on-disk locations, from the `get_app_info` command. */
export function useAppInfo() {
  return useQuery({
    queryKey: appInfoQueryKey,
    queryFn: fetchAppInfo,
    staleTime: Infinity,
  });
}
