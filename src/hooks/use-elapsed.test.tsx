import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ActiveSession } from "@/api";
import { liveElapsed, useElapsed } from "@/hooks/use-elapsed";

const base: ActiveSession = {
  sessionId: 1,
  gameId: 1,
  gameName: "Game",
  startedAt: 0,
  elapsedSecs: 100,
  afkSecs: 0,
  isAfk: false,
  source: "auto",
  hidden: false,
};

describe("liveElapsed", () => {
  it("adds local ticks only while not AFK", () => {
    expect(liveElapsed(base, 1000, 1007)).toBe(107);
    expect(liveElapsed({ ...base, isAfk: true }, 1000, 1007)).toBe(100);
  });
});

describe("useElapsed", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it("ticks, freezes while AFK and resumes from the next payload", () => {
    const { result, rerender } = renderHook(({ s }) => useElapsed(s), {
      initialProps: { s: base },
    });
    expect(result.current).toBe(100);

    act(() => void vi.advanceTimersByTime(5000));
    expect(result.current).toBe(105);

    rerender({ s: { ...base, elapsedSecs: 105, isAfk: true } });
    act(() => void vi.advanceTimersByTime(10_000));
    expect(result.current).toBe(105);

    rerender({ s: { ...base, elapsedSecs: 105, afkSecs: 10 } });
    act(() => void vi.advanceTimersByTime(3000));
    expect(result.current).toBe(108);
  });
});
