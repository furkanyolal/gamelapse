import { useState } from "react";
import type { ActiveSession } from "@/api";
import { useNow } from "@/hooks/use-now";

/**
 * Elapsed play time at `now`: the payload's `elapsedSecs` plus the seconds
 * since it arrived, frozen while the session is AFK.
 */
export function liveElapsed(
  session: Pick<ActiveSession, "elapsedSecs" | "isAfk">,
  receivedAt: number,
  now: number,
): number {
  if (session.isAfk) return session.elapsedSecs;
  return session.elapsedSecs + Math.max(0, now - receivedAt);
}

/** Elapsed play time of a session, ticking locally every second unless AFK. */
export function useElapsed(session: ActiveSession): number {
  const now = useNow();
  const marker = `${session.sessionId}:${session.elapsedSecs}:${session.isAfk}`;
  const [seen, setSeen] = useState({ marker, at: now });
  let receivedAt = seen.at;
  if (seen.marker !== marker) {
    // A fresh payload: restart local ticking from it.
    setSeen({ marker, at: now });
    receivedAt = now;
  }
  return liveElapsed(session, receivedAt, now);
}
