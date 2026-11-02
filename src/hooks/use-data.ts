import { useMutation, useQueryClient } from "@tanstack/react-query";
import { api } from "@/api";
import type { ExportOptions } from "@/api";

export function useExportData() {
  return useMutation({
    mutationFn: (options: ExportOptions) => api.exportData(options),
  });
}

export function useBackupDatabase() {
  return useMutation({ mutationFn: () => api.backupDatabase() });
}

/** After a restore every cached query is stale. */
export function useRestoreDatabase() {
  const client = useQueryClient();
  return useMutation({
    mutationFn: () => api.restoreDatabase(),
    onSuccess: () => client.invalidateQueries(),
  });
}
