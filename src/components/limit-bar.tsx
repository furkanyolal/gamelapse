import type { LimitStatus } from "@/api";
import { cn } from "@/lib/utils";
import { formatDuration } from "@/lib/format";
import { limitTitle } from "@/lib/limits";

/** Usage bar with the remaining time; red once the limit is reached. */
export function LimitBar({ status }: { status: LimitStatus }) {
  const { limit, usedTodaySecs, remainingSecs, reached } = status;
  const percent = Math.min(
    100,
    Math.round((usedTodaySecs / Math.max(1, limit.dailySecs)) * 100),
  );
  return (
    <div className="space-y-1">
      <div
        role="progressbar"
        aria-label={`${limitTitle(status)} usage`}
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={percent}
        className="bg-muted h-2 overflow-hidden rounded-full"
      >
        <div
          className={cn(
            "h-full rounded-full",
            reached ? "bg-destructive" : "bg-primary",
          )}
          style={{ width: `${percent}%` }}
        />
      </div>
      <p
        className={cn(
          "text-xs tabular-nums",
          reached ? "text-destructive" : "text-muted-foreground",
        )}
      >
        {formatDuration(usedTodaySecs)} of {formatDuration(limit.dailySecs)}
        {" · "}
        {reached ? "Limit reached" : `${formatDuration(remainingSecs)} left`}
      </p>
    </div>
  );
}
