import { Link } from "react-router";
import { DailyChart, GameBars } from "@/components/charts";
import { NowPlayingStrip } from "@/components/now-playing";
import { PageHeader } from "@/components/page-header";
import { EmptyState, ErrorState, RowsSkeleton } from "@/components/query-state";
import { StatCard } from "@/components/stat-card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
import { useGames } from "@/hooks/use-games";
import { useDailyTotals, useDashboardStats } from "@/hooks/use-stats";
import { useTrackerStatus } from "@/hooks/use-tracker";
import { formatDateTime, formatDuration } from "@/lib/format";

export default function Dashboard() {
  const stats = useDashboardStats();
  const daily = useDailyTotals(14);
  const status = useTrackerStatus();
  const games = useGames(true);

  const noGames = games.data !== undefined && games.data.length === 0;

  return (
    <div className="space-y-6">
      <PageHeader
        title="Dashboard"
        description="Today's playtime and recent activity."
      />

      {noGames && (
        <EmptyState title="No games yet">
          <p>Add a game to start tracking your playtime.</p>
          <Button asChild>
            <Link to="/games">Add game</Link>
          </Button>
        </EmptyState>
      )}

      {status.data?.active.map((s) => (
        <NowPlayingStrip key={s.sessionId} session={s} />
      ))}

      {stats.error && (
        <ErrorState error={stats.error} onRetry={() => stats.refetch()} />
      )}

      <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
        <StatCard
          label="Today"
          value={stats.data && formatDuration(stats.data.todaySecs)}
          loading={stats.isPending}
        />
        <StatCard
          label="This week"
          value={stats.data && formatDuration(stats.data.weekSecs)}
          loading={stats.isPending}
        />
        <StatCard
          label="All time"
          value={stats.data && formatDuration(stats.data.allTimeSecs)}
          loading={stats.isPending}
        />
        <StatCard
          label="Games played"
          value={stats.data && String(stats.data.gamesPlayedCount)}
          loading={stats.isPending}
        />
      </div>

      <Card>
        <CardHeader>
          <CardTitle>Last 14 days</CardTitle>
          <CardDescription>Hours played per day.</CardDescription>
        </CardHeader>
        <CardContent>
          {daily.error ? (
            <ErrorState error={daily.error} />
          ) : daily.data ? (
            <DailyChart data={daily.data} />
          ) : (
            <Skeleton className="h-[220px] w-full" />
          )}
        </CardContent>
      </Card>

      <div className="grid gap-4 lg:grid-cols-2">
        <Card>
          <CardHeader>
            <CardTitle>Top games</CardTitle>
            <CardDescription>All-time playtime.</CardDescription>
          </CardHeader>
          <CardContent>
            {stats.isPending ? (
              <RowsSkeleton />
            ) : stats.data && stats.data.topGames.length > 0 ? (
              <GameBars games={stats.data.topGames} />
            ) : (
              <p className="text-muted-foreground text-sm">
                Nothing played yet.
              </p>
            )}
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>Recent sessions</CardTitle>
            <CardDescription>
              <Link to="/sessions" className="hover:underline">
                View all sessions
              </Link>
            </CardDescription>
          </CardHeader>
          <CardContent>
            {stats.isPending ? (
              <RowsSkeleton />
            ) : stats.data && stats.data.recentSessions.length > 0 ? (
              <ul className="divide-y">
                {stats.data.recentSessions.map((s) => (
                  <li
                    key={s.id}
                    className="flex items-center justify-between gap-3 py-2 text-sm"
                  >
                    <div className="min-w-0">
                      <Link
                        to={`/games/${s.gameId}`}
                        className="block truncate font-medium hover:underline"
                      >
                        {s.gameName}
                      </Link>
                      <span className="text-muted-foreground text-xs">
                        {formatDateTime(s.startedAt)}
                      </span>
                    </div>
                    {s.endedAt === null ? (
                      <Badge>In progress</Badge>
                    ) : (
                      <span className="text-muted-foreground tabular-nums">
                        {formatDuration(s.durationSecs)}
                      </span>
                    )}
                  </li>
                ))}
              </ul>
            ) : (
              <p className="text-muted-foreground text-sm">
                No sessions recorded.
              </p>
            )}
          </CardContent>
        </Card>
      </div>
    </div>
  );
}
