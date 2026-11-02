import { useEffect } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { api } from "@/api";
import type { TrackerStatus } from "@/api";
import { invalidateFamilies, keys } from "@/hooks/query-keys";

/** Subscribes to every backend event; mount once in the app shell. */
export function useApiEvents() {
  const client = useQueryClient();

  useEffect(() => {
    const unsubscribers = [
      api.on("session-started", () =>
        invalidateFamilies(client, "tracker", "games", "sessions", "stats"),
      ),
      api.on("session-ended", () =>
        invalidateFamilies(client, "tracker", "games", "sessions", "stats"),
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
        invalidateFamilies(client, "games", "sessions", "stats"),
      ),
      api.on("sessions-changed", () =>
        invalidateFamilies(client, "sessions", "games", "stats"),
      ),
    ];
    return () => unsubscribers.forEach((off) => off());
  }, [client]);
}
