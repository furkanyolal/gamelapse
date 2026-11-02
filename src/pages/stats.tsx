import { useState } from "react";
import { DailyChart, GameBars, HourlyChart } from "@/components/charts";
import { PageHeader } from "@/components/page-header";
import { ErrorState, RowsSkeleton } from "@/components/query-state";
import { StatCard } from "@/components/stat-card";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { useNow } from "@/hooks/use-now";
import {
  useDailyTotals,
  useGameTotals,
  useHourlyTotals,
} from "@/hooks/use-stats";
import { formatDuration, startOfLocalDay } from "@/lib/format";

const PERIODS = [7, 30, 90] as const;

export default function Stats() {
  const [days, setDays] = useState<number>(30);
  const now = useNow(60_000);
  const from = startOfLocalDay(now, -(days - 1));
  const daily = useDailyTotals(days);
  const games = useGameTotals(from, null, 10);
  const hourly = useHourlyTotals(days);

  const total = daily.data?.reduce((n, d) => n + d.totalSecs, 0);
  const activeDays = daily.data?.filter((d) => d.totalSecs > 0).length;
  const best = daily.data?.reduce(
    (a, d) => (d.totalSecs > a.totalSecs ? d : a),
    { date: "", totalSecs: 0 },
  );

  return (
    <div className="space-y-6">
      <div className="flex items-start justify-between gap-4">
        <PageHeader title="Stats" description="Trends in your play time." />
        <Tabs value={String(days)} onValueChange={(v) => setDays(Number(v))}>
          <TabsList>
            {PERIODS.map((p) => (
              <TabsTrigger key={p} value={String(p)}>
                {p} days
              </TabsTrigger>
            ))}
          </TabsList>
        </Tabs>
      </div>

      <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
        <StatCard
          label="Total"
          value={total === undefined ? undefined : formatDuration(total)}
        />
        <StatCard
          label="Daily average"
          value={total === undefined ? undefined : formatDuration(total / days)}
        />
        <StatCard
          label="Days played"
          value={activeDays === undefined ? undefined : `${activeDays}/${days}`}
        />
        <StatCard
          label="Best day"
          value={best ? formatDuration(best.totalSecs) : undefined}
        />
      </div>

      <Card>
        <CardHeader>
          <CardTitle>Daily playtime</CardTitle>
          <CardDescription>Hours played per day.</CardDescription>
        </CardHeader>
        <CardContent>
          {daily.error ? (
            <ErrorState error={daily.error} />
          ) : daily.data ? (
            <DailyChart data={daily.data} height={260} />
          ) : (
            <Skeleton className="h-[260px] w-full" />
          )}
        </CardContent>
      </Card>

      <div className="grid gap-4 lg:grid-cols-2">
        <Card>
          <CardHeader>
            <CardTitle>Top games</CardTitle>
            <CardDescription>In the last {days} days.</CardDescription>
          </CardHeader>
          <CardContent>
            {games.error ? (
              <ErrorState error={games.error} />
            ) : games.data ? (
              games.data.length > 0 ? (
                <GameBars games={games.data} />
              ) : (
                <p className="text-muted-foreground text-sm">
                  Nothing played in this period.
                </p>
              )
            ) : (
              <RowsSkeleton />
            )}
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>Time of day</CardTitle>
            <CardDescription>Hours played per hour of the day.</CardDescription>
          </CardHeader>
          <CardContent>
            {hourly.error ? (
              <ErrorState error={hourly.error} />
            ) : hourly.data ? (
              <HourlyChart data={hourly.data} />
            ) : (
              <Skeleton className="h-[220px] w-full" />
            )}
          </CardContent>
        </Card>
      </div>
    </div>
  );
}
