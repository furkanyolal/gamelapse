import { useMemo } from "react";
import { Link } from "react-router";
import { Bar, BarChart, CartesianGrid, XAxis, YAxis } from "recharts";
import type { DailyTotal, GameTotal, HourlyTotal } from "@/api";
import {
  ChartContainer,
  ChartTooltip,
  ChartTooltipContent,
  type ChartConfig,
} from "@/components/ui/chart";
import { formatDayLabel, formatDuration, toHours } from "@/lib/format";

const config = {
  hours: { label: "Playtime", color: "var(--chart-1)" },
} satisfies ChartConfig;

const hoursTick = (v: number) => `${v}h`;

function hoursTooltip() {
  return (
    <ChartTooltipContent
      hideIndicator
      formatter={(_value, _name, item) => (
        <span className="font-medium tabular-nums">
          {formatDuration((item.payload as { secs: number }).secs)}
        </span>
      )}
    />
  );
}

export function DailyChart({
  data,
  height = 220,
}: {
  data: DailyTotal[];
  height?: number;
}) {
  const rows = useMemo(
    () =>
      data.map((d) => ({
        label: formatDayLabel(d.date),
        hours: toHours(d.totalSecs),
        secs: d.totalSecs,
      })),
    [data],
  );
  return (
    <ChartContainer config={config} className="w-full" style={{ height }}>
      <BarChart data={rows} accessibilityLayer>
        <CartesianGrid vertical={false} />
        <XAxis
          dataKey="label"
          tickLine={false}
          axisLine={false}
          tickMargin={8}
          interval="preserveStartEnd"
          minTickGap={16}
        />
        <YAxis
          tickLine={false}
          axisLine={false}
          width={36}
          tickFormatter={hoursTick}
        />
        <ChartTooltip cursor={false} content={hoursTooltip()} />
        <Bar
          dataKey="hours"
          fill="var(--color-hours)"
          radius={3}
          isAnimationActive={false}
        />
      </BarChart>
    </ChartContainer>
  );
}

export function HourlyChart({
  data,
  height = 220,
}: {
  data: HourlyTotal[];
  height?: number;
}) {
  const rows = useMemo(
    () =>
      data.map((d) => ({
        label: `${String(d.hour).padStart(2, "0")}:00`,
        hours: toHours(d.totalSecs),
        secs: d.totalSecs,
      })),
    [data],
  );
  return (
    <ChartContainer config={config} className="w-full" style={{ height }}>
      <BarChart data={rows} accessibilityLayer>
        <CartesianGrid vertical={false} />
        <XAxis
          dataKey="label"
          tickLine={false}
          axisLine={false}
          tickMargin={8}
          interval={2}
        />
        <YAxis
          tickLine={false}
          axisLine={false}
          width={36}
          tickFormatter={hoursTick}
        />
        <ChartTooltip cursor={false} content={hoursTooltip()} />
        <Bar
          dataKey="hours"
          fill="var(--color-hours)"
          radius={3}
          isAnimationActive={false}
        />
      </BarChart>
    </ChartContainer>
  );
}

/** Ranked list of games with bars proportional to the leader. */
export function GameBars({ games }: { games: GameTotal[] }) {
  const max = Math.max(1, ...games.map((g) => g.totalSecs));
  return (
    <ol className="space-y-3">
      {games.map((g, i) => (
        <li key={g.gameId} className="space-y-1">
          <div className="flex items-baseline justify-between gap-3 text-sm">
            <Link
              to={`/games/${g.gameId}`}
              className="truncate hover:underline"
            >
              <span className="text-muted-foreground mr-2 tabular-nums">
                {i + 1}
              </span>
              {g.gameName}
            </Link>
            <span className="text-muted-foreground shrink-0 tabular-nums">
              {formatDuration(g.totalSecs)}
            </span>
          </div>
          <div className="bg-muted h-2 overflow-hidden rounded-full">
            <div
              className="bg-chart-1 h-full rounded-full"
              style={{ width: `${(g.totalSecs / max) * 100}%` }}
            />
          </div>
        </li>
      ))}
    </ol>
  );
}
