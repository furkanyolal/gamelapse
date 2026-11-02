import { useEffect, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api } from "@/api";
import type { UpdateInfo, UpdateProgress } from "@/api";
import { keys } from "@/hooks/query-keys";

/** Whether Gamelapse starts with Windows. */
export function useAutostart() {
  return useQuery({
    queryKey: keys.autostart,
    queryFn: () => api.getAutostart(),
  });
}

export function useSetAutostart() {
  const client = useQueryClient();
  return useMutation({
    mutationFn: (enabled: boolean) => api.setAutostart(enabled),
    onSuccess: () => client.invalidateQueries({ queryKey: keys.autostart }),
  });
}

/** Last explicit or silent update check; undefined until one has run. */
export function useUpdateCheckResult() {
  return useQuery<UpdateInfo | null>({
    queryKey: keys.updateCheck,
    queryFn: () => Promise.reject(new Error("No update check has run")),
    enabled: false,
    staleTime: Infinity,
    retry: false,
  });
}

export function useCheckForUpdate() {
  const client = useQueryClient();
  return useMutation({
    mutationFn: () => api.checkForUpdate(),
    onSuccess: (info) => client.setQueryData(keys.updateCheck, info),
  });
}

export function useInstallUpdate() {
  return useMutation({ mutationFn: () => api.installUpdate() });
}

/** Latest `update-progress` payload while `active`; reset when it turns off. */
export function useUpdateProgress(active: boolean): UpdateProgress | null {
  const [progress, setProgress] = useState<UpdateProgress | null>(null);
  useEffect(() => {
    if (!active) return;
    const off = api.on("update-progress", setProgress);
    return () => {
      off();
      setProgress(null);
    };
  }, [active]);
  return progress;
}
