import { format, formatDistanceToNow } from "date-fns";

/** "2h 15m", "45m", "30s"; "0m" for zero. */
export function formatDuration(secs: number): string {
  const total = Math.max(0, Math.floor(secs));
  if (total === 0) return "0m";
  if (total < 60) return `${total}s`;
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  if (h === 0) return `${m}m`;
  return m === 0 ? `${h}h` : `${h}h ${m}m`;
}

/** Clock-style duration for live timers: "1:05:09". */
export function formatClock(secs: number): string {
  const total = Math.max(0, Math.floor(secs));
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${h}:${pad(m)}:${pad(s)}`;
}

/** Unix seconds to e.g. "Nov 2, 2026 14:05". */
export function formatDateTime(ts: number): string {
  return format(new Date(ts * 1000), "MMM d, yyyy HH:mm");
}

/** Unix seconds to e.g. "3 hours ago"; "Never" for null. */
export function formatRelative(ts: number | null): string {
  if (ts === null) return "Never";
  return formatDistanceToNow(new Date(ts * 1000), { addSuffix: true });
}

/** Compact relative time: "just now", "25m ago", "7h ago", "4d ago", "3w ago", "5mo ago", "2y ago". */
export function formatRelativeShort(
  ts: number | null,
  nowSecs: number = Math.floor(Date.now() / 1000),
): string {
  if (ts === null) return "Never";
  const secs = Math.max(0, nowSecs - ts);
  const mins = Math.floor(secs / 60);
  if (mins < 1) return "just now";
  if (mins < 60) return `${mins}m ago`;
  const hours = Math.floor(mins / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.floor(hours / 24);
  if (days < 7) return `${days}d ago`;
  if (days < 30) return `${Math.floor(days / 7)}w ago`;
  if (days < 365) return `${Math.floor(days / 30)}mo ago`;
  return `${Math.floor(days / 365)}y ago`;
}

/** Unix seconds to the `YYYY-MM-DDTHH:mm` value of a datetime-local input. */
export function toDateTimeLocal(ts: number): string {
  return format(new Date(ts * 1000), "yyyy-MM-dd'T'HH:mm");
}

/** Value of a datetime-local input to unix seconds; null if empty or invalid. */
export function fromDateTimeLocal(value: string): number | null {
  if (!value) return null;
  const ms = new Date(value).getTime();
  return Number.isNaN(ms) ? null : Math.floor(ms / 1000);
}

/** Start of the local day, `dayOffset` days away from the day of `ts`. */
export function startOfLocalDay(ts: number, dayOffset = 0): number {
  const d = new Date(ts * 1000);
  return (
    new Date(d.getFullYear(), d.getMonth(), d.getDate() + dayOffset).getTime() /
    1000
  );
}

/** "YYYY-MM-DD" to a short axis label such as "Nov 2". */
export function formatDayLabel(date: string): string {
  const [y, m, d] = date.split("-").map(Number);
  return format(new Date(y, m - 1, d), "MMM d");
}

/** Seconds to fractional hours for chart axes. */
export function toHours(secs: number): number {
  return Math.round((secs / 3600) * 100) / 100;
}
