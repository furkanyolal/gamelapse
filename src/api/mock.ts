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
  DiscoveredGame,
  ImportResult,
  ImportSource,
  ImportSourceStatus,
  Limit,
  LimitInput,
  LimitStatus,
  Session,
  SessionWithGame,
  Settings,
  SteamApiSettings,
  Tag,
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
  importOnStartup: true,
  notificationsEnabled: true,
  reminderIntervalSecs: 0,
};

/** The fake active session goes AFK for the last 20 s of every 90 s cycle. */
const AFK_CYCLE_SECS = 90;
const AFK_SECS = 20;
const REPEAT_NOTICE_SECS = 15 * 60;
const MIN_LIMIT_SECS = 60;
const MAX_LIMIT_SECS = 86_400;
const MAX_TAG_LEN = 40;
const MOCK_COVER_URL =
  "https://cdn.cloudflare.steamstatic.com/steam/apps/1091500/header.jpg";

type StoredLimit = Omit<Limit, "gameName">;

const STEAM_ROOT = "c:\\program files (x86)\\steam";
const steamCover = (appId: string) =>
  `https://cdn.cloudflare.steamstatic.com/steam/apps/${appId}/header.jpg`;

type CatalogEntry = Omit<DiscoveredGame, "alreadyImported"> & {
  /** Counts as imported from the start, as if an earlier run had added it. */
  preImported?: boolean;
};

function steamEntry(
  appId: string,
  name: string,
  dir: string,
  playtimeMinutes: number | null,
  preImported = false,
): CatalogEntry {
  const installDir = `${STEAM_ROOT}\\steamapps\\common\\${dir.toLowerCase()}`;
  return {
    source: "steam",
    sourceId: appId,
    name,
    installDir,
    executables: [
      `${installDir}\\${dir.toLowerCase().replaceAll(" ", "")}.exe`,
    ],
    playtimeMinutes,
    preImported,
  };
}

const IMPORT_CATALOG: Record<ImportSource, CatalogEntry[]> = {
  steam: [
    steamEntry("620", "Portal 2", "Portal 2", 1260, true),
    steamEntry("550", "Left 4 Dead 2", "Left 4 Dead 2", 3480, true),
    steamEntry("367520", "Hollow Knight", "Hollow Knight", 2410),
    steamEntry("632470", "Disco Elysium", "Disco Elysium", 1875),
    steamEntry("105600", "Terraria", "Terraria", null),
  ],
  epic: [],
  gog: [
    {
      source: "gog",
      sourceId: "1423049311",
      name: "Cyberpunk 2077",
      installDir: "c:\\gog games\\cyberpunk 2077",
      executables: [
        "c:\\gog games\\cyberpunk 2077\\bin\\x64\\cyberpunk2077.exe",
      ],
      playtimeMinutes: 5400,
    },
  ],
};

const IMPORT_AVAILABILITY: Record<
  ImportSource,
  { available: boolean; detail: string }
> = {
  steam: { available: true, detail: "C:\\Program Files (x86)\\Steam" },
  epic: {
    available: false,
    detail:
      "Epic Games Launcher not found (no manifests in C:\\ProgramData\\Epic\\EpicGamesLauncher\\Data\\Manifests)",
  },
  gog: { available: true, detail: "C:\\Program Files (x86)\\GOG Galaxy" },
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
  const importedMinutes = new Map<number, number>();
  let tags: Tag[] = [];
  let nextTagId = 1;
  const gameTags = new Map<number, Set<number>>();
  const lastScanAt: Record<ImportSource, number | null> = {
    steam: null,
    epic: null,
    gog: null,
  };
  let steamApi: SteamApiSettings = { apiKey: null, steamId64: null };
  let limits: StoredLimit[] = [];
  let nextLimitId = 1;
  let tickCount = 0;
  let fakeAfkSessionId: number | null = null;
  const afkSessions = new Set<number>();
  /** Limits already reported as reached, so each fires once. */
  const reachedLimits = new Set<number>();
  const lastLimitNoticeAt = new Map<number, number>();
  /** Last reminder multiple seen per session; the first sight is silent. */
  const reminderBuckets = new Map<number, number>();

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
      isAfk: afkSessions.has(s.id),
      source: s.source === "manual" ? "manual" : "auto",
    };
  }

  function limitView(l: StoredLimit): Limit {
    return {
      ...l,
      gameName: l.gameId === null ? null : gameName(l.gameId),
    };
  }

  function statusOfLimit(l: StoredLimit): LimitStatus {
    const own =
      l.gameId === null
        ? sessions
        : sessions.filter((s) => s.gameId === l.gameId);
    const used = sumWithin(own, localMidnight(now()), Infinity);
    return {
      limit: limitView(l),
      usedTodaySecs: used,
      remainingSecs: Math.max(0, l.dailySecs - used),
      reached: used >= l.dailySecs,
    };
  }

  function limitStatuses(): LimitStatus[] {
    return [...limits]
      .sort((a, b) => {
        if (a.scope !== b.scope) return a.scope === "global" ? -1 : 1;
        return gameName(a.gameId ?? 0).localeCompare(gameName(b.gameId ?? 0));
      })
      .map(statusOfLimit);
  }

  function checkedLimitInput(input: LimitInput, selfId: number | null) {
    if (
      !Number.isInteger(input.dailySecs) ||
      input.dailySecs < MIN_LIMIT_SECS ||
      input.dailySecs > MAX_LIMIT_SECS
    ) {
      throw new Error("A limit must be between 1 minute and 24 hours");
    }
    let gameId: number | null = null;
    if (input.scope === "game") {
      if (input.gameId === null) throw new Error("Choose a game for the limit");
      gameId = requireGame(input.gameId).id;
    }
    if (limits.some((l) => l.id !== selfId && l.gameId === gameId)) {
      throw new Error(
        gameId === null
          ? "A global limit already exists"
          : `${gameName(gameId)} already has a limit`,
      );
    }
    return {
      scope: input.scope,
      gameId,
      dailySecs: input.dailySecs,
      action: input.action,
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

  function sortedTags(list: Tag[]): Tag[] {
    return [...list]
      .sort((a, b) => a.name.localeCompare(b.name))
      .map((t) => ({ ...t }));
  }

  function tagsOf(gameId: number): Tag[] {
    const ids = gameTags.get(gameId);
    return sortedTags(tags.filter((t) => ids?.has(t.id)));
  }

  function checkedTagName(raw: string, selfId: number | null): string {
    const name = raw.trim();
    if (!name) throw new Error("Tag name cannot be empty");
    if (name.length > MAX_TAG_LEN) {
      throw new Error(`Tag name must be at most ${MAX_TAG_LEN} characters`);
    }
    const lower = name.toLowerCase();
    if (tags.some((t) => t.id !== selfId && t.name.toLowerCase() === lower)) {
      throw new Error(`A tag named "${name}" already exists`);
    }
    return name;
  }

  function summaryOf(game: Game): GameSummary {
    const own = sessions.filter((s) => s.gameId === game.id);
    const today = localMidnight(now());
    return {
      game: { ...game },
      tags: tagsOf(game.id),
      totalSecs: own.reduce((n, s) => n + durationOf(s), 0),
      todaySecs: sumWithin(own, today, Infinity),
      weekSecs: sumWithin(own, localMidnight(now(), -6), Infinity),
      sessionCount: own.length,
      lastPlayedAt: own.length
        ? Math.max(...own.map((s) => s.startedAt))
        : null,
      importedMinutes: importedMinutes.get(game.id) ?? 0,
    };
  }

  function findImported(source: ImportSource, sourceId: string) {
    return games.find((g) => g.source === source && g.sourceId === sourceId);
  }

  function requireAvailable(source: ImportSource) {
    const { available, detail } = IMPORT_AVAILABILITY[source];
    if (!available) throw new Error(detail);
  }

  function discover(source: ImportSource): DiscoveredGame[] {
    requireAvailable(source);
    return IMPORT_CATALOG[source].map(
      ({ preImported, ...entry }): DiscoveredGame => ({
        ...entry,
        executables: [...entry.executables],
        alreadyImported:
          findImported(source, entry.sourceId) !== undefined ||
          preImported === true,
      }),
    );
  }

  function importOf(source: ImportSource): ImportResult {
    const found = discover(source);
    const result: ImportResult = {
      source,
      discovered: found.length,
      added: 0,
      updated: 0,
      skipped: 0,
      errors: [],
    };
    for (const d of found) {
      const existing = findImported(source, d.sourceId);
      if (existing) {
        existing.installDir = d.installDir;
        if (d.playtimeMinutes !== null) {
          importedMinutes.set(existing.id, d.playtimeMinutes);
        }
        result.updated += 1;
        continue;
      }
      if (d.alreadyImported) {
        result.skipped += 1;
        continue;
      }
      const game = insertGame(d.name, {
        source,
        sourceId: d.sourceId,
        installDir: d.installDir,
        coverPath: source === "steam" ? steamCover(d.sourceId) : null,
      });
      if (d.installDir) {
        executables.push({
          id: nextExeId++,
          gameId: game.id,
          exePath: d.installDir,
          matchMode: "install_dir_prefix",
        });
      }
      for (const exe of d.executables) {
        executables.push({
          id: nextExeId++,
          gameId: game.id,
          exePath: exe,
          matchMode: "exact_path",
        });
      }
      if (d.playtimeMinutes !== null) {
        importedMinutes.set(game.id, d.playtimeMinutes);
      }
      result.added += 1;
    }
    lastScanAt[source] = now();
    return result;
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
        coverPath:
          i === 0
            ? steamCover("1245620")
            : i === 4
              ? steamCover("1086940")
              : null,
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
    if (tags.length === 0) {
      const [rpg, coop, backlog] = ["RPG", "Co-op", "Backlog"].map((name) => {
        const tag: Tag = { id: nextTagId++, name };
        tags.push(tag);
        return tag;
      });
      const assign: [number, Tag[]][] = [
        [0, [rpg]],
        [1, [rpg]],
        [3, [coop]],
        [4, [rpg, coop]],
        [5, [backlog]],
        [7, [rpg, backlog]],
      ];
      for (const [i, list] of assign) {
        gameTags.set(demo[i].id, new Set(list.map((t) => t.id)));
      }
    }
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
      const active = insertSession({
        gameId: demo[6].id,
        startedAt: now() - 25 * 60,
        endedAt: null,
        durationSecs: 0,
        afkSecs: 0,
        source: "auto",
        note: null,
      });
      fakeAfkSessionId = active.id;
      limits.push(
        {
          id: nextLimitId++,
          scope: "global",
          gameId: null,
          dailySecs: 4 * 3600,
          action: "notify_repeat",
        },
        {
          id: nextLimitId++,
          scope: "game",
          gameId: demo[6].id,
          dailySecs: 2 * 3600,
          action: "notify",
        },
      );
      limitStatuses().forEach((st) => {
        if (st.reached) reachedLimits.add(st.limit.id);
      });
    }
  }

  function checkLimits() {
    const activeGames = new Set(
      activeSessions()
        .filter((s) => !afkSessions.has(s.id))
        .map((s) => s.gameId),
    );
    for (const st of limitStatuses()) {
      const id = st.limit.id;
      if (!st.reached) {
        reachedLimits.delete(id);
        continue;
      }
      const counted =
        st.limit.gameId === null
          ? activeGames.size > 0
          : activeGames.has(st.limit.gameId);
      const first = !reachedLimits.has(id);
      const repeat =
        !first &&
        counted &&
        st.limit.action === "notify_repeat" &&
        now() - (lastLimitNoticeAt.get(id) ?? 0) >= REPEAT_NOTICE_SECS;
      if (first || repeat) {
        reachedLimits.add(id);
        lastLimitNoticeAt.set(id, now());
        emit("limit-reached", st);
      }
    }
  }

  function checkReminders() {
    const interval = settings.reminderIntervalSecs;
    if (interval <= 0) {
      reminderBuckets.clear();
      return;
    }
    for (const s of activeSessions()) {
      if (afkSessions.has(s.id)) continue;
      const elapsed = durationOf(s);
      const bucket = Math.floor(elapsed / interval);
      const prev = reminderBuckets.get(s.id);
      reminderBuckets.set(s.id, bucket);
      if (prev !== undefined && bucket > prev) {
        emit("reminder", {
          sessionId: s.id,
          gameId: s.gameId,
          gameName: gameName(s.gameId),
          elapsedSecs: elapsed,
        });
      }
    }
  }

  function tick() {
    lastPollAt = now();
    tickCount += 1;
    const fake = sessions.find(
      (s) => s.id === fakeAfkSessionId && s.endedAt === null,
    );
    if (fake) {
      if (tickCount % AFK_CYCLE_SECS >= AFK_CYCLE_SECS - AFK_SECS) {
        afkSessions.add(fake.id);
        fake.afkSecs += 1;
      } else {
        afkSessions.delete(fake.id);
      }
    }
    activeSessions().forEach((s) => emit("session-updated", toActive(s)));
    checkLimits();
    checkReminders();
  }

  function ensureTicker() {
    if (timer !== null || listenerCount() === 0) return;
    timer = setInterval(tick, 1000);
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
    afkSessions.delete(s.id);
    reminderBuckets.delete(s.id);
    return { ...s };
  }

  const api: Api = {
    async getAppInfo() {
      return {
        version: "0.1.0-mock",
        dataDir: "C:\\Users\\mock\\AppData\\Roaming\\Gamelapse",
        dbPath: "C:\\Users\\mock\\AppData\\Roaming\\Gamelapse\\gamelapse.db",
        logDir: "C:\\Users\\mock\\AppData\\Roaming\\Gamelapse\\logs",
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
      limits = limits.filter((l) => l.gameId !== gameId);
      importedMinutes.delete(gameId);
      gameTags.delete(gameId);
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
      if (patch.importOnStartup !== null) {
        next.importOnStartup = patch.importOnStartup;
      }
      if (patch.notificationsEnabled !== null) {
        next.notificationsEnabled = patch.notificationsEnabled;
      }
      if (patch.reminderIntervalSecs !== null) {
        const v = patch.reminderIntervalSecs;
        if (v !== 0 && (v < 300 || v > 14_400)) {
          throw new Error(
            "Reminder interval must be off or 5 minutes to 4 hours",
          );
        }
        if (v !== next.reminderIntervalSecs) reminderBuckets.clear();
        next.reminderIntervalSecs = v;
      }
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
    async listImportSources() {
      return (["steam", "epic", "gog"] as const).map<ImportSourceStatus>(
        (source) => ({
          source,
          available: IMPORT_AVAILABILITY[source].available,
          detail: IMPORT_AVAILABILITY[source].detail,
          lastScanAt: lastScanAt[source],
        }),
      );
    },
    async scanImportSource(source) {
      const found = discover(source);
      lastScanAt[source] = now();
      return found;
    },
    async importGames(source) {
      const result = importOf(source);
      emit("library-changed", null);
      return result;
    },
    async getSteamApiSettings() {
      return { ...steamApi };
    },
    async setSteamApiSettings(next) {
      steamApi = {
        apiKey: next.apiKey?.trim() || null,
        steamId64: next.steamId64?.trim() || null,
      };
      return null;
    },
    async fetchSteamPlaytime() {
      if (!steamApi.apiKey) {
        throw new Error("Set a Steam Web API key before fetching playtime.");
      }
      if (!steamApi.steamId64) {
        throw new Error("Set your SteamID64 before fetching playtime.");
      }
      const owned = games.filter((g) => g.source === "steam");
      for (const g of owned) {
        const base = IMPORT_CATALOG.steam.find(
          (e) => e.sourceId === g.sourceId,
        );
        importedMinutes.set(
          g.id,
          Math.max(importedMinutes.get(g.id) ?? 0, base?.playtimeMinutes ?? 0) +
            60,
        );
      }
      emit("library-changed", null);
      return {
        source: "steam",
        discovered: owned.length,
        added: 0,
        updated: owned.length,
        skipped: 0,
        errors: [],
      };
    },
    async listLimits() {
      return limitStatuses();
    },
    async createLimit(input) {
      const limit: StoredLimit = {
        id: nextLimitId,
        ...checkedLimitInput(input, null),
      };
      nextLimitId += 1;
      limits.push(limit);
      return limitView(limit);
    },
    async updateLimit(limitId, input) {
      const limit = limits.find((l) => l.id === limitId);
      if (!limit) throw new Error(`Limit ${limitId} not found`);
      Object.assign(limit, checkedLimitInput(input, limitId));
      return limitView(limit);
    },
    async deleteLimit(limitId) {
      if (!limits.some((l) => l.id === limitId)) {
        throw new Error(`Limit ${limitId} not found`);
      }
      limits = limits.filter((l) => l.id !== limitId);
      reachedLimits.delete(limitId);
      lastLimitNoticeAt.delete(limitId);
      return null;
    },
    async listTags() {
      return sortedTags(tags);
    },
    async createTag(name) {
      const tag: Tag = { id: nextTagId++, name: checkedTagName(name, null) };
      tags.push(tag);
      emit("library-changed", null);
      return { ...tag };
    },
    async renameTag(tagId, name) {
      const tag = tags.find((t) => t.id === tagId);
      if (!tag) throw new Error(`Tag ${tagId} not found`);
      tag.name = checkedTagName(name, tagId);
      emit("library-changed", null);
      return { ...tag };
    },
    async deleteTag(tagId) {
      if (!tags.some((t) => t.id === tagId)) {
        throw new Error(`Tag ${tagId} not found`);
      }
      tags = tags.filter((t) => t.id !== tagId);
      gameTags.forEach((ids) => ids.delete(tagId));
      emit("library-changed", null);
      return null;
    },
    async setGameTags(gameId, tagIds) {
      requireGame(gameId);
      const unknown = tagIds.find((id) => !tags.some((t) => t.id === id));
      if (unknown !== undefined) throw new Error(`Tag ${unknown} not found`);
      gameTags.set(gameId, new Set(tagIds));
      emit("library-changed", null);
      return tagsOf(gameId);
    },
    async mergeGames(survivorId, mergedIds) {
      requireGame(survivorId);
      if (mergedIds.includes(survivorId)) {
        throw new Error("The game to keep cannot also be merged into itself");
      }
      const ids = [...new Set(mergedIds)];
      ids.forEach(requireGame);
      for (const id of ids) {
        if (activeSessions().some((s) => s.gameId === id)) {
          throw new Error(`Stop the active session of ${gameName(id)} first`);
        }
      }
      let mergedSessions = 0;
      let mergedExecutables = 0;
      for (const id of ids) {
        for (const s of sessions) {
          if (s.gameId === id) {
            s.gameId = survivorId;
            mergedSessions += 1;
          }
        }
        for (const e of executables.filter((x) => x.gameId === id)) {
          const dup = executables.some(
            (x) =>
              x.gameId === survivorId &&
              x.exePath === e.exePath &&
              x.matchMode === e.matchMode,
          );
          if (!dup) {
            e.gameId = survivorId;
            mergedExecutables += 1;
          }
        }
        const own = gameTags.get(survivorId) ?? new Set<number>();
        gameTags.get(id)?.forEach((t) => own.add(t));
        gameTags.set(survivorId, own);
        const minutes = importedMinutes.get(id);
        if (minutes !== undefined) {
          importedMinutes.set(
            survivorId,
            Math.max(importedMinutes.get(survivorId) ?? 0, minutes),
          );
        }
      }
      games = games.filter((g) => !ids.includes(g.id));
      executables = executables.filter((e) => !ids.includes(e.gameId));
      limits = limits.filter(
        (l) => l.gameId === null || !ids.includes(l.gameId),
      );
      ids.forEach((id) => {
        gameTags.delete(id);
        importedMinutes.delete(id);
      });
      emit("library-changed", null);
      emit("sessions-changed", null);
      return { survivorId, mergedSessions, mergedExecutables };
    },
    async pickImage() {
      return MOCK_COVER_URL;
    },
    async setGameCover(gameId, sourcePath) {
      const game = requireGame(gameId);
      game.coverPath = sourcePath;
      emit("library-changed", null);
      return { ...game };
    },
    async exportData(options) {
      const picked = sessions.filter(
        (s) =>
          (options.gameId === null || s.gameId === options.gameId) &&
          (options.from === null || s.startedAt >= options.from) &&
          (options.to === null || s.startedAt < options.to),
      );
      const stamp = format(new Date(now() * 1000), "yyyyMMdd");
      return {
        path: `C:\\Users\\mock\\Documents\\gamelapse-sessions-${stamp}.${options.format}`,
        sessions: picked.length,
        games: new Set(picked.map((s) => s.gameId)).size,
      };
    },
    async backupDatabase() {
      const stamp = format(new Date(now() * 1000), "yyyyMMdd-HHmm");
      return {
        path: `C:\\Users\\mock\\Documents\\gamelapse-backup-${stamp}.db`,
        createdAt: now(),
        sizeBytes: 262_144 + sessions.length * 96,
      };
    },
    async restoreDatabase() {
      if (activeSessions().length > 0) {
        throw new Error("Stop the active sessions before restoring a backup");
      }
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
