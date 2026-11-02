import { describe, expect, it, vi } from "vitest";
import { createMockApi } from "@/api/mock";

const noSessionFilter = {
  gameId: null,
  from: null,
  to: null,
  limit: 1000,
  offset: 0,
};

describe("mock api", () => {
  it("lists eight games sorted by sortName", async () => {
    const api = createMockApi();
    const games = await api.listGames(false);
    expect(games).toHaveLength(8);
    const sortNames = games.map((g) => g.game.sortName);
    expect(sortNames).toEqual(
      [...sortNames].sort((a, b) => a.localeCompare(b)),
    );
    // "The Witcher 3" sorts under W.
    expect(sortNames).toContain("witcher 3");
  });

  it("is deterministic for a given clock and seed", async () => {
    const now = () => 1_800_000_000;
    const a = await createMockApi({ now }).listSessions(noSessionFilter);
    const b = await createMockApi({ now }).listSessions(noSessionFilter);
    expect(a).toEqual(b);
    expect(a.length).toBeGreaterThanOrEqual(100);
  });

  it("starts and stops a manual session", async () => {
    let t = 1_800_000_000;
    const api = createMockApi({ now: () => t });
    const [first] = await api.listGames(false);
    const started = await api.startManualSession(first.game.id);
    expect(started.source).toBe("manual");
    expect(
      (await api.getTrackerStatus()).active.map((a) => a.sessionId),
    ).toContain(started.sessionId);
    await expect(api.startManualSession(first.game.id)).rejects.toThrow();

    t += 600;
    const ended = await api.stopSession(started.sessionId);
    expect(ended.durationSecs).toBe(600);
    expect(ended.endedAt).toBe(t);
    expect(
      (await api.getTrackerStatus()).active.map((a) => a.sessionId),
    ).not.toContain(started.sessionId);
  });

  it("adds a game from an executable and rejects duplicates", async () => {
    const api = createMockApi({ empty: true });
    const game = await api.addGameFromExe("C:\\Games\\Foo\\foo.exe", null);
    expect(game.name).toBe("foo");
    expect(game.installDir).toBe("c:\\games\\foo");
    expect(await api.listGameExecutables(game.id)).toHaveLength(1);
    await expect(
      api.addGameFromExe("c:/games/foo/FOO.exe", "Again"),
    ).rejects.toThrow();
    expect(await api.listGames(false)).toHaveLength(1);
  });

  it("hides hidden games unless asked", async () => {
    const api = createMockApi();
    const [first] = await api.listGames(false);
    await api.updateGame(first.game.id, {
      name: null,
      hidden: true,
      afkDetectionEnabled: null,
    });
    expect(await api.listGames(false)).toHaveLength(7);
    expect(await api.listGames(true)).toHaveLength(8);
  });

  it("seeds demo data into an empty library only once", async () => {
    const api = createMockApi({ empty: true });
    await api.seedDemoData();
    await api.seedDemoData();
    expect(await api.listGames(false)).toHaveLength(8);
  });

  it("creates, edits and deletes sessions", async () => {
    const api = createMockApi();
    const [first] = await api.listGames(false);
    await expect(
      api.createSession(first.game.id, 200, 100, null),
    ).rejects.toThrow();
    const s = await api.createSession(first.game.id, 1000, 4600, "note");
    expect(s.durationSecs).toBe(3600);
    const edited = await api.updateSession(s.id, {
      startedAt: null,
      endedAt: 2800,
      note: "",
    });
    expect(edited.durationSecs).toBe(1800);
    expect(edited.note).toBeNull();
    await api.deleteSession(s.id);
    const all = await api.listSessions(noSessionFilter);
    expect(all.find((x) => x.id === s.id)).toBeUndefined();
  });

  it("returns zero-filled daily totals and 24 hourly buckets", async () => {
    const api = createMockApi();
    const daily = await api.getDailyTotals(30, null);
    expect(daily).toHaveLength(30);
    expect(daily.some((d) => d.totalSecs > 0)).toBe(true);
    expect(await api.getHourlyTotals(30)).toHaveLength(24);
  });

  it("emits session-updated every second while subscribed", async () => {
    vi.useFakeTimers();
    try {
      const api = createMockApi();
      const handler = vi.fn();
      const off = api.on("session-updated", handler);
      vi.advanceTimersByTime(3000);
      expect(handler).toHaveBeenCalledTimes(3);
      off();
      vi.advanceTimersByTime(3000);
      expect(handler).toHaveBeenCalledTimes(3);
    } finally {
      vi.useRealTimers();
    }
  });
});
