import { describe, expect, it } from "vitest";
import {
  formatClock,
  formatDateTime,
  formatDuration,
  formatRelative,
  formatRelativeShort,
  fromDateTimeLocal,
  splitHoursMinutes,
  toDateTimeLocal,
} from "@/lib/format";

describe("splitHoursMinutes", () => {
  it("splits seconds into hours and minutes", () => {
    expect(splitHoursMinutes(8100)).toEqual({ hours: 2, minutes: 15 });
    expect(splitHoursMinutes(86_400)).toEqual({ hours: 24, minutes: 0 });
    expect(splitHoursMinutes(60)).toEqual({ hours: 0, minutes: 1 });
    expect(splitHoursMinutes(-5)).toEqual({ hours: 0, minutes: 0 });
  });
});

describe("formatDuration", () => {
  it("formats hours and minutes", () => {
    expect(formatDuration(8100)).toBe("2h 15m");
    expect(formatDuration(3600)).toBe("1h");
  });
  it("does not wrap at 24 hours", () => {
    expect(formatDuration(86_400)).toBe("24h");
    expect(formatDuration(90_000)).toBe("25h");
    expect(formatDuration(100 * 3600 + 5 * 60)).toBe("100h 5m");
  });
  it("formats minutes, seconds and zero", () => {
    expect(formatDuration(2700)).toBe("45m");
    expect(formatDuration(30)).toBe("30s");
    expect(formatDuration(0)).toBe("0m");
    expect(formatDuration(-5)).toBe("0m");
  });
});

describe("formatClock", () => {
  it("pads minutes and seconds", () => {
    expect(formatClock(3909)).toBe("1:05:09");
    expect(formatClock(0)).toBe("0:00:00");
  });
});

describe("date helpers", () => {
  it("formats a timestamp in local time", () => {
    const ts = new Date(2026, 10, 2, 14, 5).getTime() / 1000;
    expect(formatDateTime(ts)).toBe("Nov 2, 2026 14:05");
  });

  it("formats relative time", () => {
    const ts = Math.floor(Date.now() / 1000) - 3 * 3600;
    expect(formatRelative(ts)).toBe("about 3 hours ago");
    expect(formatRelative(null)).toBe("Never");
  });

  it("formats compact relative time", () => {
    const now = 1_800_000_000;
    const ago = (s: number) => formatRelativeShort(now - s, now);
    expect(ago(10)).toBe("just now");
    expect(ago(25 * 60)).toBe("25m ago");
    expect(ago(7 * 3600)).toBe("7h ago");
    expect(ago(4 * 86400)).toBe("4d ago");
    expect(ago(25 * 86400)).toBe("3w ago");
    expect(ago(150 * 86400)).toBe("5mo ago");
    expect(ago(800 * 86400)).toBe("2y ago");
    expect(formatRelativeShort(null)).toBe("Never");
  });

  it("round-trips datetime-local values", () => {
    const ts = new Date(2026, 10, 2, 9, 30).getTime() / 1000;
    expect(toDateTimeLocal(ts)).toBe("2026-11-02T09:30");
    expect(fromDateTimeLocal("2026-11-02T09:30")).toBe(ts);
    expect(fromDateTimeLocal("")).toBeNull();
  });
});
