import { useEffect } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import { api } from "@/api";
import type { TrackerStatus } from "@/api";
import { invalidateFamilies, keys } from "@/hooks/query-keys";
import { formatDuration } from "@/lib/format";

/** Subscribes to every backend event; mount once in the app shell. */
export function useApiEvents() {
  const client = useQueryClient();

  useEffect(() => {
    const unsubscribers = [
      api.on("session-started", () =>
        invalidateFamilies(
          client,
          "tracker",
          "games",
          "sessions",
          "stats",
          "limits",
        ),
      ),
      api.on("session-ended", () =>
        invalidateFamilies(
          client,
          "tracker",
          "games",
          "sessions",
          "stats",
          "limits",
        ),
      ),
      api.on("limit-reached", (status) => {
        invalidateFamilies(client, "limits");
        const { limit } = status;
        toast.warning(`${limit.gameName ?? "Daily"} limit reached`, {
          description: `${formatDuration(status.usedTodaySecs)} played today (limit ${formatDuration(limit.dailySecs)}).`,
        });
      }),
      api.on("reminder", (reminder) =>
        toast(`Still playing ${reminder.gameName}`, {
          description: `${formatDuration(reminder.elapsedSecs)} this session.`,
        }),
      ),
      api.on("session-updated", (active) => {
        client.setQueryData<TrackerStatus>(keys.trackerStatus, (old) => {
          if (!old) return old;
          const known = old.active.some(
            (a) => a.sessionId === active.sessionId,
          );
          return {
            ...old,
            active: known
              ? old.active.map((a) =>
                  a.sessionId === active.sessionId ? active : a,
                )
              : [...old.active, active],
          };
        });
      }),
      api.on("tracker-status-changed", () =>
        invalidateFamilies(client, "tracker", "settings"),
      ),
      api.on("library-changed", () =>
        invalidateFamilies(client, "games", "sessions", "stats", "import"),
      ),
      api.on("sessions-changed", () =>
        invalidateFamilies(client, "sessions", "games", "stats"),
      ),
    ];
    return () => unsubscribers.forEach((off) => off());
  }, [client]);
}
