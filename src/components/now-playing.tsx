import { Link } from "react-router";
import { Square } from "lucide-react";
import type { ActiveSession } from "@/api";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { useElapsed } from "@/hooks/use-elapsed";
import { useStopSession } from "@/hooks/use-tracker";
import { formatClock } from "@/lib/format";

export function Elapsed({ session }: { session: ActiveSession }) {
  return (
    <span className="tabular-nums">{formatClock(useElapsed(session))}</span>
  );
}

export function NowPlayingStrip({ session }: { session: ActiveSession }) {
  const stop = useStopSession();
  return (
    <div className="bg-card flex items-center gap-4 rounded-lg border p-4">
      <span className="relative flex size-2.5">
        <span className="absolute inline-flex size-full animate-ping rounded-full bg-emerald-500 opacity-60" />
        <span className="relative inline-flex size-2.5 rounded-full bg-emerald-500" />
      </span>
      <div className="min-w-0 flex-1">
        <p className="text-muted-foreground text-xs">Now playing</p>
        <Link
          to={`/games/${session.gameId}`}
          className="block truncate font-medium hover:underline"
        >
          {session.gameName}
        </Link>
      </div>
      {session.isAfk && <Badge variant="secondary">AFK</Badge>}
      <Badge variant="outline">{session.source}</Badge>
      <div className="text-xl font-semibold">
        <Elapsed session={session} />
      </div>
      <Button
        variant="outline"
        size="sm"
        disabled={stop.isPending}
        onClick={() => stop.mutate(session.sessionId)}
      >
        <Square /> Stop
      </Button>
    </div>
  );
}
