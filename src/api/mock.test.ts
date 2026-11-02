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

  it("never overlaps sessions or exceeds 14h in a local day", async () => {
    const now = 1_800_000_000;
    const api = createMockApi({ now: () => now });
    const all = await api.listSessions(noSessionFilter);
    expect(all.length).toBeGreaterThanOrEqual(100);
    const spans = all
      .map((s) => ({ from: s.startedAt, to: s.endedAt ?? now }))
      .sort((a, b) => a.from - b.from);
    for (let i = 1; i < spans.length; i++) {
      expect(spans[i].from).toBeGreaterThanOrEqual(spans[i - 1].to);
    }
    for (const s of all) {
      if (s.endedAt === null) continue;
      expect(s.endedAt - s.startedAt).toBeGreaterThanOrEqual(600);
      expect(s.endedAt - s.startedAt).toBeLessThanOrEqual(5 * 3600);
    }
    const daily = await api.getDailyTotals(60, null);
    expect(Math.max(...daily.map((d) => d.totalSecs))).toBeLessThanOrEqual(
      14 * 3600,
    );
    expect(daily.some((d) => d.totalSecs === 0)).toBe(true);
  });

  it("keeps 14-day totals consistent with sessions and the 30-day view", async () => {
    const now = 1_800_000_000;
    const api = createMockApi({ now: () => now });
    const d14 = await api.getDailyTotals(14, null);
    const d30 = await api.getDailyTotals(30, null);
    expect(d14).toHaveLength(14);
    expect(d14).toEqual(d30.slice(-14));
    const start = new Date(now * 1000);
    const from =
      new Date(
        start.getFullYear(),
        start.getMonth(),
        start.getDate() - 13,
      ).getTime() / 1000;
    const all = await api.listSessions(noSessionFilter);
    const expected = all
      .filter((s) => s.startedAt >= from)
      .reduce((n, s) => n + s.durationSecs, 0);
    const actual = d14.reduce((n, d) => n + d.totalSecs, 0);
    expect(Math.abs(actual - expected)).toBeLessThanOrEqual(14);
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

describe("mock api: launcher imports", () => {
  it("lists all three sources with Epic unavailable", async () => {
    const api = createMockApi({ empty: true });
    const sources = await api.listImportSources();
    expect(sources.map((s) => s.source)).toEqual(["steam", "epic", "gog"]);
    expect(sources.map((s) => s.available)).toEqual([true, false, true]);
    expect(sources[1].detail).toBeTruthy();
    await expect(api.scanImportSource("epic")).rejects.toThrow();
  });

  it("scans, imports, reports counts and marks games imported", async () => {
    const api = createMockApi({ empty: true });
    const onLibrary = vi.fn();
    api.on("library-changed", onLibrary);

    const first = await api.scanImportSource("steam");
    expect(first).toHaveLength(5);
    expect(first.filter((g) => g.alreadyImported)).toHaveLength(2);
    expect(await api.listGames(false)).toHaveLength(0);

    const result = await api.importGames("steam");
    expect(result).toMatchObject({
      source: "steam",
      discovered: 5,
      added: 3,
      updated: 0,
      skipped: 2,
      errors: [],
    });
    expect(onLibrary).toHaveBeenCalled();

    const games = await api.listGames(false);
    expect(games).toHaveLength(3);
    const hollow = games.find((g) => g.game.name === "Hollow Knight")!;
    expect(hollow.game.source).toBe("steam");
    expect(hollow.game.sourceId).toBe("367520");
    expect(hollow.game.installDir).toContain("hollow knight");
    expect(hollow.importedMinutes).toBe(2410);
    const exes = await api.listGameExecutables(hollow.game.id);
    expect(exes.map((e) => e.matchMode).sort()).toEqual([
      "exact_path",
      "install_dir_prefix",
    ]);

    const second = await api.scanImportSource("steam");
    expect(second.every((g) => g.alreadyImported)).toBe(true);
    const again = await api.importGames("steam");
    expect(again).toMatchObject({ added: 0, updated: 3, skipped: 2 });

    const gog = await api.importGames("gog");
    expect(gog).toMatchObject({ discovered: 1, added: 1 });
  });

  it("fetchSteamPlaytime needs a key and SteamID64", async () => {
    const api = createMockApi({ empty: true });
    await expect(api.fetchSteamPlaytime()).rejects.toThrow(/api key/i);
    await api.setSteamApiSettings({ apiKey: "k", steamId64: null });
    await expect(api.fetchSteamPlaytime()).rejects.toThrow(/steamid64/i);

    await api.importGames("steam");
    await api.setSteamApiSettings({ apiKey: "k", steamId64: "7656119" });
    expect(await api.getSteamApiSettings()).toEqual({
      apiKey: "k",
      steamId64: "7656119",
    });
    const before = (await api.listGames(false)).map((g) => g.importedMinutes);
    const result = await api.fetchSteamPlaytime();
    expect(result).toMatchObject({ source: "steam", updated: 3, errors: [] });
    const after = (await api.listGames(false)).map((g) => g.importedMinutes);
    after.forEach((m, i) => expect(m).toBeGreaterThan(before[i]));
  });

  it("gives two demo games an https cover", async () => {
    const games = await createMockApi().listGames(false);
    const covers = games.map((g) => g.game.coverPath).filter(Boolean);
    expect(covers).toHaveLength(2);
    covers.forEach((c) => expect(c).toMatch(/^https:\/\//));
  });
});
