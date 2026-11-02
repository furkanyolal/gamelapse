// Hand-mirrored from docs/contract.md until the generated bindings replace it.

export type GameSource = "manual" | "steam" | "epic" | "gog";
export type SessionSource = "auto" | "manual" | "imported";
export type MatchMode = "exact_path" | "install_dir_prefix" | "exe_name";
export type Theme = "dark" | "light" | "system";

export interface AppInfo {
  version: string;
  dataDir: string;
  dbPath: string;
  logDir: string;
}

export interface Game {
  id: number;
  name: string;
  sortName: string;
  source: GameSource;
  sourceId: string | null;
  installDir: string | null;
  iconPath: string | null;
  coverPath: string | null;
  hidden: boolean;
  afkDetectionEnabled: boolean;
  createdAt: number;
}

export interface GameExecutable {
  id: number;
  gameId: number;
  exePath: string;
  matchMode: MatchMode;
}

export interface Session {
  id: number;
  gameId: number;
  startedAt: number;
  /** null while the session is active */
  endedAt: number | null;
  durationSecs: number;
  afkSecs: number;
  source: SessionSource;
  note: string | null;
}

export interface SessionWithGame extends Session {
  gameName: string;
}

export interface ActiveSession {
  sessionId: number;
  gameId: number;
  gameName: string;
  startedAt: number;
  elapsedSecs: number;
  afkSecs: number;
  isAfk: boolean;
  source: "auto" | "manual";
}

export interface TrackerStatus {
  trackingEnabled: boolean;
  active: ActiveSession[];
  lastPollAt: number | null;
}

export interface GameSummary {
  game: Game;
  totalSecs: number;
  todaySecs: number;
  weekSecs: number;
  sessionCount: number;
  lastPlayedAt: number | null;
  importedMinutes: number;
}

export interface GameTotal {
  gameId: number;
  gameName: string;
  totalSecs: number;
}

export interface DashboardStats {
  todaySecs: number;
  weekSecs: number;
  allTimeSecs: number;
  gamesPlayedCount: number;
  topGames: GameTotal[];
  recentSessions: SessionWithGame[];
}

export interface DailyTotal {
  /** Local date, "YYYY-MM-DD" */
  date: string;
  totalSecs: number;
}

export interface HourlyTotal {
  hour: number;
  totalSecs: number;
}

export interface Settings {
  pollIntervalSecs: number;
  afkThresholdSecs: number;
  sessionMergeGapSecs: number;
  requireForeground: boolean;
  trackingEnabled: boolean;
  theme: Theme;
  importOnStartup: boolean;
}

export type ImportSource = "steam" | "epic" | "gog";

export interface ImportSourceStatus {
  source: ImportSource;
  /** The launcher was found on this machine */
  available: boolean;
  /** Install path when available, otherwise why not */
  detail: string | null;
  lastScanAt: number | null;
}

export interface DiscoveredGame {
  source: ImportSource;
  sourceId: string;
  name: string;
  installDir: string | null;
  /** Normalised exact executable paths known from the launcher; may be empty */
  executables: string[];
  /** Launcher-reported playtime, when the launcher tracks it */
  playtimeMinutes: number | null;
  /** A game with this source and sourceId already exists */
  alreadyImported: boolean;
}

export interface ImportResult {
  source: ImportSource;
  discovered: number;
  added: number;
  updated: number;
  skipped: number;
  errors: string[];
}

export interface SteamApiSettings {
  apiKey: string | null;
  steamId64: string | null;
}

export interface SettingsUpdate {
  pollIntervalSecs: number | null;
  afkThresholdSecs: number | null;
  sessionMergeGapSecs: number | null;
  requireForeground: boolean | null;
  trackingEnabled: boolean | null;
  theme: Theme | null;
  importOnStartup: boolean | null;
}

export interface GameUpdate {
  name: string | null;
  hidden: boolean | null;
  afkDetectionEnabled: boolean | null;
}

export interface SessionUpdate {
  startedAt: number | null;
  endedAt: number | null;
  /** null keeps, "" clears */
  note: string | null;
}

export interface SessionQuery {
  gameId: number | null;
  from: number | null;
  to: number | null;
  limit: number;
  offset: number;
}

export type ApiEventMap = {
  "session-started": ActiveSession;
  "session-updated": ActiveSession;
  "session-ended": Session;
  "tracker-status-changed": TrackerStatus;
  "library-changed": null;
  "sessions-changed": null;
};

export type ApiEventName = keyof ApiEventMap;
