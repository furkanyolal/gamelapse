import { format } from "date-fns";
import type { Api } from "./api";
import type {
  ActiveSession,
  ApiEventMap,
  ApiEventName,
  DailyTotal,
  Game,
  GameExecutable,
  GameSummary,
  HourlyTotal,
  Session,
  SessionWithGame,
  Settings,
  TrackerStatus,
} from "./types";

export interface MockOptions {
  /** Current time in unix seconds. Defaults to the wall clock. */
  now?: () => number;
  /** Start with an empty library; `seedDemoData` then fills it. */
  empty?: boolean;
  /** Seed of the deterministic generator. */
  seed?: number;
}

const DAY = 86_400;
const DEMO_GAMES = [
  { name: "Elden Ring", weight: 8 },
  { name: "Hades II", weight: 6 },
  { name: "Stardew Valley", weight: 5 },
  { name: "Counter-Strike 2", weight: 5 },
  { name: "Baldur's Gate 3", weight: 4 },
  { name: "Celeste", weight: 2 },
  { name: "Factorio", weight: 3 },
  { name: "The Witcher 3", weight: 2 },
];
const DEFAULT_SETTINGS: Settings = {
  pollIntervalSecs: 5,
  afkThresholdSecs: 600,
  sessionMergeGapSecs: 120,
  requireForeground: false,
  trackingEnabled: true,
  theme: "dark",
};

/** Small deterministic PRNG. */
function mulberry32(seed: number): () => number {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

function sortNameOf(name: string): string {
  const lower = name.trim().toLowerCase();
  return lower.startsWith("the ") ? lower.slice(4) : lower;
}

function normalisePath(path: string): string {
  return path.trim().replaceAll("/", "\\").toLowerCase();
}

function localMidnight(ts: number, dayOffset = 0): number {
  const d = new Date(ts * 1000);
  return (
    new Date(d.getFullYear(), d.getMonth(), d.getDate() + dayOffset).getTime() /
    1000
  );
}

function nextMidnight(ts: number): number {
  return localMidnight(ts, 1);
}

function nextHour(ts: number): number {
  const d = new Date(ts * 1000);
  d.setMinutes(0, 0, 0);
  return d.getTime() / 1000 + 3600;
}

function overlap(a0: number, a1: number, b0: number, b1: number): number {
  return Math.max(0, Math.min(a1, b1) - Math.max(a0, b0));
}

export function createMockApi(options: MockOptions = {}): Api {
  const now = options.now ?? (() => Math.floor(Date.now() / 1000));
  const rng = mulberry32(options.seed ?? 42);

  let games: Game[] = [];
  let executables: GameExecutable[] = [];
  let sessions: Session[] = [];
  let settings: Settings = { ...DEFAULT_SETTINGS };
  let lastPollAt: number | null = null;
  let nextGameId = 1;
  let nextExeId = 1;
  let nextSessionId = 1;
  let pickCounter = 0;

  const listeners = new Map<ApiEventName, Set<(payload: never) => void>>();
  let timer: ReturnType<typeof setInterval> | null = null;

  function emit<E extends ApiEventName>(event: E, payload: ApiEventMap[E]) {
    listeners.get(event)?.forEach((h) => h(payload as never));
  }

  function listenerCount() {
    let n = 0;
    listeners.forEach((set) => (n += set.size));
    return n;
  }

  function durationOf(s: Session): number {
    return s.endedAt === null
      ? Math.max(0, now() - s.startedAt - s.afkSecs)
      : s.durationSecs;
  }

  function view(s: Session): Session {
    return { ...s, durationSecs: durationOf(s) };
  }

  function gameName(gameId: number): string {
    return games.find((g) => g.id === gameId)?.name ?? "Unknown";
  }

  function toActive(s: Session): ActiveSession {
    return {
      sessionId: s.id,
      gameId: s.gameId,
      gameName: gameName(s.gameId),
      startedAt: s.startedAt,
      elapsedSecs: durationOf(s),
      afkSecs: s.afkSecs,
      isAfk: false,
      source: s.source === "manual" ? "manual" : "auto",
    };
  }

  function activeSessions(): Session[] {
    return sessions.filter((s) => s.endedAt === null);
  }

  function status(): TrackerStatus {
    return {
      trackingEnabled: settings.trackingEnabled,
      active: activeSessions().map(toActive),
      lastPollAt,
    };
  }

  /** Seconds of a session's counted time that fall inside [from, to). */
  function secsWithin(s: Session, from: number, to: number): number {
    const end = s.endedAt ?? now();
    const span = end - s.startedAt;
    if (span <= 0) return 0;
    return (overlap(s.startedAt, end, from, to) * durationOf(s)) / span;
  }

  function sumWithin(list: Session[], from: number, to: number): number {
    return Math.round(list.reduce((n, s) => n + secsWithin(s, from, to), 0));
  }

  function withGame(s: Session): SessionWithGame {
    return { ...view(s), gameName: gameName(s.gameId) };
  }

  function summaryOf(game: Game): GameSummary {
    const own = sessions.filter((s) => s.gameId === game.id);
    const today = localMidnight(now());
    return {
      game: { ...game },
      totalSecs: own.reduce((n, s) => n + durationOf(s), 0),
      todaySecs: sumWithin(own, today, Infinity),
      weekSecs: sumWithin(own, localMidnight(now(), -6), Infinity),
      sessionCount: own.length,
      lastPlayedAt: own.length
        ? Math.max(...own.map((s) => s.startedAt))
        : null,
      importedMinutes: 0,
    };
  }

  function requireGame(gameId: number): Game {
    const g = games.find((x) => x.id === gameId);
    if (!g) throw new Error(`Game ${gameId} not found`);
    return g;
  }

  function requireSession(sessionId: number): Session {
    const s = sessions.find((x) => x.id === sessionId);
    if (!s) throw new Error(`Session ${sessionId} not found`);
    return s;
  }

  function insertGame(name: string, init: Partial<Game> = {}): Game {
    const game: Game = {
      id: nextGameId++,
      name,
      sortName: sortNameOf(name),
      source: "manual",
      sourceId: null,
      installDir: null,
      iconPath: null,
      coverPath: null,
      hidden: false,
      afkDetectionEnabled: true,
      createdAt: now(),
      ...init,
    };
    games.push(game);
    return game;
  }

  function insertSession(init: Omit<Session, "id">): Session {
    const s: Session = { ...init, id: nextSessionId++ };
    sessions.push(s);
    return s;
  }

  function generateDemo(withActive: boolean) {
    const demo = DEMO_GAMES.map((g, i) => {
      const dir = `c:\\games\\${g.name.toLowerCase().replace(/[^a-z0-9]+/g, "-")}`;
      const game = insertGame(g.name, {
        sourceId: `demo-${i + 1}`,
        installDir: dir,
        createdAt: now() - 60 * DAY,
      });
      executables.push({
        id: nextExeId++,
        gameId: game.id,
        exePath: `${dir}\\game.exe`,
        matchMode: "exact_path",
      });
      return game;
    });
    const totalWeight = DEMO_GAMES.reduce((n, g) => n + g.weight, 0);
    const pickGame = () => {
      let pick = rng() * totalWeight;
      let idx = 0;
      while (idx < DEMO_GAMES.length - 1 && pick >= DEMO_GAMES[idx].weight) {
        pick -= DEMO_GAMES[idx].weight;
        idx++;
      }
      return idx;
    };
    // Walk forward day by day, playing one game at a time so sessions never
    // overlap, stay inside their own day and finish before the fake active one.
    const generated: Omit<Session, "id">[] = [];
    const lastEnd = now() - 1800;
    for (let daysAgo = 59; daysAgo >= 0; daysAgo--) {
      if (rng() > 0.8) continue;
      const dayStart = localMidnight(now(), -daysAgo);
      const dayEnd = Math.min(
        localMidnight(now(), -daysAgo + 1) - 300,
        lastEnd,
      );
      let cursor = dayStart + (9 + Math.floor(rng() * 11)) * 3600;
      let budget = 12 * 3600;
      const count = 1 + Math.floor(rng() * 4);
      for (let n = 0; n < count; n++) {
        const wanted = (10 + Math.floor(Math.pow(rng(), 1.6) * 290)) * 60;
        const length = Math.min(wanted, budget, dayEnd - cursor);
        if (length < 600) break;
        const afkSecs = rng() < 0.1 ? Math.floor(length * 0.1) : 0;
        generated.push({
          gameId: demo[pickGame()].id,
          startedAt: cursor,
          endedAt: cursor + length,
          durationSecs: length - afkSecs,
          afkSecs,
          source: "auto",
          note: null,
        });
        budget -= length;
        cursor += length + (10 + Math.floor(rng() * 110)) * 60;
      }
    }
    generated.forEach((s) => insertSession(s));
    if (withActive) {
      insertSession({
        gameId: demo[6].id,
        startedAt: now() - 25 * 60,
        endedAt: null,
        durationSecs: 0,
        afkSecs: 0,
        source: "auto",
        note: null,
      });
    }
  }

  function ensureTicker() {
    if (timer !== null || listenerCount() === 0) return;
    timer = setInterval(() => {
      lastPollAt = now();
      activeSessions().forEach((s) => emit("session-updated", toActive(s)));
    }, 1000);
  }

  function maybeStopTicker() {
    if (timer !== null && listenerCount() === 0) {
      clearInterval(timer);
      timer = null;
    }
  }

  if (!options.empty) generateDemo(true);

  function endSession(s: Session): Session {
    const end = Math.max(now(), s.startedAt);
    s.endedAt = end;
    s.durationSecs = Math.max(0, end - s.startedAt - s.afkSecs);
    return { ...s };
  }

  const api: Api = {
    async getAppInfo() {
      return {
        version: "0.1.0-mock",
        dataDir: "C:\\Users\\mock\\AppData\\Roaming\\GameTimeTracker",
        dbPath:
          "C:\\Users\\mock\\AppData\\Roaming\\GameTimeTracker\\gametimetracker.db",
        logDir: "C:\\Users\\mock\\AppData\\Roaming\\GameTimeTracker\\logs",
      };
    },
    async ping() {
      return "pong (mock)";
    },
    async getTrackerStatus() {
      return status();
    },
    async setTrackingEnabled(enabled) {
      settings = { ...settings, trackingEnabled: enabled };
      if (!enabled) {
        for (const s of activeSessions().filter((x) => x.source !== "manual")) {
          emit("session-ended", endSession(s));
        }
      }
      const st = status();
      emit("tracker-status-changed", st);
      return st;
    },
    async startManualSession(gameId) {
      const game = requireGame(gameId);
      if (activeSessions().some((s) => s.gameId === game.id)) {
        throw new Error(`${game.name} already has an active session`);
      }
      const s = insertSession({
        gameId,
        startedAt: now(),
        endedAt: null,
        durationSecs: 0,
        afkSecs: 0,
        source: "manual",
        note: null,
      });
      const active = toActive(s);
      emit("session-started", active);
      emit("tracker-status-changed", status());
      return active;
    },
    async stopSession(sessionId) {
      const s = requireSession(sessionId);
      if (s.endedAt !== null) throw new Error("Session is not active");
      const ended = endSession(s);
      emit("session-ended", ended);
      emit("tracker-status-changed", status());
      return ended;
    },
    async listGames(includeHidden) {
      return games
        .filter((g) => includeHidden || !g.hidden)
        .sort((a, b) => a.sortName.localeCompare(b.sortName))
        .map(summaryOf);
    },
    async getGame(gameId) {
      return summaryOf(requireGame(gameId));
    },
    async addGameFromExe(exePath, name) {
      const path = normalisePath(exePath);
      if (executables.some((e) => e.exePath === path)) {
        throw new Error("That executable is already attached to a game");
      }
      const file = exePath.split(/[\\/]/).pop() ?? exePath;
      const stem = file.replace(/\.[^.]*$/, "");
      const dir =
        exePath.includes("\\") || exePath.includes("/")
          ? normalisePath(exePath.replace(/[\\/][^\\/]*$/, ""))
          : null;
      const game = insertGame(name?.trim() || stem, { installDir: dir });
      executables.push({
        id: nextExeId++,
        gameId: game.id,
        exePath: path,
        matchMode: "exact_path",
      });
      emit("library-changed", null);
      return { ...game };
    },
    async updateGame(gameId, patch) {
      const game = requireGame(gameId);
      if (patch.name !== null) {
        const name = patch.name.trim();
        if (!name) throw new Error("Name cannot be empty");
        game.name = name;
        game.sortName = sortNameOf(name);
      }
      if (patch.hidden !== null) game.hidden = patch.hidden;
      if (patch.afkDetectionEnabled !== null) {
        game.afkDetectionEnabled = patch.afkDetectionEnabled;
      }
      emit("library-changed", null);
      return { ...game };
    },
    async deleteGame(gameId) {
      requireGame(gameId);
      if (activeSessions().some((s) => s.gameId === gameId)) {
        throw new Error("Stop the active session before deleting this game");
      }
      games = games.filter((g) => g.id !== gameId);
      executables = executables.filter((e) => e.gameId !== gameId);
      sessions = sessions.filter((s) => s.gameId !== gameId);
      emit("library-changed", null);
      return null;
    },
    async listGameExecutables(gameId) {
      return executables
        .filter((e) => e.gameId === gameId)
        .map((e) => ({ ...e }));
    },
    async addGameExecutable(gameId, exePath, matchMode) {
      requireGame(gameId);
      const exe: GameExecutable = {
        id: nextExeId++,
        gameId,
        exePath: normalisePath(exePath),
        matchMode,
      };
      executables.push(exe);
      emit("library-changed", null);
      return { ...exe };
    },
    async removeGameExecutable(executableId) {
      executables = executables.filter((e) => e.id !== executableId);
      emit("library-changed", null);
      return null;
    },
    async pickExecutable() {
      pickCounter += 1;
      return `C:\\Games\\Mock Game ${pickCounter}\\mockgame${pickCounter}.exe`;
    },
    async listSessions(query) {
      return sessions
        .filter(
          (s) =>
            (query.gameId === null || s.gameId === query.gameId) &&
            (query.from === null || s.startedAt >= query.from) &&
            (query.to === null || s.startedAt < query.to),
        )
        .sort((a, b) => b.startedAt - a.startedAt || b.id - a.id)
        .slice(query.offset, query.offset + query.limit)
        .map(withGame);
    },
    async createSession(gameId, startedAt, endedAt, note) {
      requireGame(gameId);
      if (endedAt <= startedAt) throw new Error("End must be after start");
      const s = insertSession({
        gameId,
        startedAt,
        endedAt,
        durationSecs: endedAt - startedAt,
        afkSecs: 0,
        source: "manual",
        note: note?.trim() ? note.trim() : null,
      });
      emit("sessions-changed", null);
      return { ...s };
    },
    async updateSession(sessionId, patch) {
      const s = requireSession(sessionId);
      if (s.endedAt === null) throw new Error("Cannot edit an active session");
      const startedAt = patch.startedAt ?? s.startedAt;
      const endedAt = patch.endedAt ?? s.endedAt;
      if (endedAt <= startedAt) throw new Error("End must be after start");
      s.startedAt = startedAt;
      s.endedAt = endedAt;
      s.durationSecs = Math.max(0, endedAt - startedAt - s.afkSecs);
      if (patch.note !== null) s.note = patch.note === "" ? null : patch.note;
      emit("sessions-changed", null);
      return { ...s };
    },
    async deleteSession(sessionId) {
      const s = requireSession(sessionId);
      if (s.endedAt === null)
        throw new Error("Cannot delete an active session");
      sessions = sessions.filter((x) => x.id !== sessionId);
      emit("sessions-changed", null);
      return null;
    },
    async getDashboardStats() {
      const topMap = new Map<number, number>();
      sessions.forEach((s) =>
        topMap.set(s.gameId, (topMap.get(s.gameId) ?? 0) + durationOf(s)),
      );
      return {
        todaySecs: sumWithin(sessions, localMidnight(now()), Infinity),
        weekSecs: sumWithin(sessions, localMidnight(now(), -6), Infinity),
        allTimeSecs: sessions.reduce((n, s) => n + durationOf(s), 0),
        gamesPlayedCount: topMap.size,
        topGames: [...topMap.entries()]
          .map(([gameId, totalSecs]) => ({
            gameId,
            gameName: gameName(gameId),
            totalSecs,
          }))
          .sort((a, b) => b.totalSecs - a.totalSecs)
          .slice(0, 5),
        recentSessions: [...sessions]
          .sort((a, b) => b.startedAt - a.startedAt || b.id - a.id)
          .slice(0, 10)
          .map(withGame),
      };
    },
    async getDailyTotals(days, gameId) {
      const list =
        gameId === null
          ? sessions
          : sessions.filter((s) => s.gameId === gameId);
      const out: DailyTotal[] = [];
      for (let i = days - 1; i >= 0; i--) {
        const from = localMidnight(now(), -i);
        out.push({
          date: format(new Date(from * 1000), "yyyy-MM-dd"),
          totalSecs: sumWithin(list, from, nextMidnight(from)),
        });
      }
      return out;
    },
    async getGameTotals(from, to, limit) {
      const map = new Map<number, number>();
      sessions
        .filter(
          (s) =>
            (from === null || s.startedAt >= from) &&
            (to === null || s.startedAt < to),
        )
        .forEach((s) =>
          map.set(s.gameId, (map.get(s.gameId) ?? 0) + durationOf(s)),
        );
      return [...map.entries()]
        .map(([gameId, totalSecs]) => ({
          gameId,
          gameName: gameName(gameId),
          totalSecs,
        }))
        .sort((a, b) => b.totalSecs - a.totalSecs)
        .slice(0, limit);
    },
    async getHourlyTotals(days) {
      const rangeStart = localMidnight(now(), -(days - 1));
      const buckets = new Array<number>(24).fill(0);
      for (const s of sessions) {
        const end = s.endedAt ?? now();
        const span = end - s.startedAt;
        if (span <= 0) continue;
        const ratio = durationOf(s) / span;
        let t = Math.max(s.startedAt, rangeStart);
        while (t < end) {
          const b = Math.min(nextHour(t), end);
          buckets[new Date(t * 1000).getHours()] += (b - t) * ratio;
          t = b;
        }
      }
      return buckets.map<HourlyTotal>((secs, hour) => ({
        hour,
        totalSecs: Math.round(secs),
      }));
    },
    async getSettings() {
      return { ...settings };
    },
    async updateSettings(patch) {
      const next = { ...settings };
      if (patch.pollIntervalSecs !== null) {
        if (patch.pollIntervalSecs < 1 || patch.pollIntervalSecs > 60) {
          throw new Error("Poll interval must be between 1 and 60 seconds");
        }
        next.pollIntervalSecs = patch.pollIntervalSecs;
      }
      if (patch.afkThresholdSecs !== null) {
        if (patch.afkThresholdSecs < 60) {
          throw new Error("AFK threshold must be at least 1 minute");
        }
        next.afkThresholdSecs = patch.afkThresholdSecs;
      }
      if (patch.sessionMergeGapSecs !== null) {
        if (patch.sessionMergeGapSecs < 0) {
          throw new Error("Merge gap cannot be negative");
        }
        next.sessionMergeGapSecs = patch.sessionMergeGapSecs;
      }
      if (patch.requireForeground !== null) {
        next.requireForeground = patch.requireForeground;
      }
      if (patch.trackingEnabled !== null) {
        next.trackingEnabled = patch.trackingEnabled;
      }
      if (patch.theme !== null) next.theme = patch.theme;
      settings = next;
      return { ...settings };
    },
    async seedDemoData() {
      if (games.some((g) => g.sourceId?.startsWith("demo-"))) return null;
      generateDemo(false);
      emit("library-changed", null);
      emit("sessions-changed", null);
      return null;
    },
    on(event, handler) {
      let set = listeners.get(event);
      if (!set) {
        set = new Set();
        listeners.set(event, set);
      }
      const h = handler as (payload: never) => void;
      set.add(h);
      ensureTicker();
      return () => {
        set.delete(h);
        maybeStopTicker();
      };
    },
  };
  return api;
}
