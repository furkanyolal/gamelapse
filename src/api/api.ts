import type {
  ActiveSession,
  BackupInfo,
  ExportOptions,
  ExportResult,
  MergeResult,
  Tag,
  ApiEventMap,
  ApiEventName,
  AppInfo,
  DailyTotal,
  DashboardStats,
  DiscoveredGame,
  Game,
  GameExecutable,
  GameSummary,
  GameTotal,
  GameUpdate,
  HourlyTotal,
  ImportResult,
  ImportSource,
  ImportSourceStatus,
  Limit,
  LimitInput,
  LimitStatus,
  MatchMode,
  Session,
  SessionQuery,
  SessionUpdate,
  SessionWithGame,
  Settings,
  SettingsUpdate,
  SteamApiSettings,
  TrackerStatus,
  UpdateInfo,
} from "./types";

/** One camelCase method per IPC command in docs/contract.md. */
export interface Api {
  getAppInfo(): Promise<AppInfo>;
  ping(): Promise<string>;
  getTrackerStatus(): Promise<TrackerStatus>;
  setTrackingEnabled(enabled: boolean): Promise<TrackerStatus>;
  startManualSession(gameId: number): Promise<ActiveSession>;
  stopSession(sessionId: number): Promise<Session>;
  listGames(includeHidden: boolean): Promise<GameSummary[]>;
  getGame(gameId: number): Promise<GameSummary>;
  addGameFromExe(exePath: string, name: string | null): Promise<Game>;
  updateGame(gameId: number, patch: GameUpdate): Promise<Game>;
  deleteGame(gameId: number): Promise<null>;
  listGameExecutables(gameId: number): Promise<GameExecutable[]>;
  addGameExecutable(
    gameId: number,
    exePath: string,
    matchMode: MatchMode,
  ): Promise<GameExecutable>;
  removeGameExecutable(executableId: number): Promise<null>;
  pickExecutable(): Promise<string | null>;
  listSessions(query: SessionQuery): Promise<SessionWithGame[]>;
  createSession(
    gameId: number,
    startedAt: number,
    endedAt: number,
    note: string | null,
  ): Promise<Session>;
  updateSession(sessionId: number, patch: SessionUpdate): Promise<Session>;
  deleteSession(sessionId: number): Promise<null>;
  getDashboardStats(): Promise<DashboardStats>;
  getDailyTotals(days: number, gameId: number | null): Promise<DailyTotal[]>;
  getGameTotals(
    from: number | null,
    to: number | null,
    limit: number,
  ): Promise<GameTotal[]>;
  getHourlyTotals(days: number): Promise<HourlyTotal[]>;
  getSettings(): Promise<Settings>;
  updateSettings(patch: SettingsUpdate): Promise<Settings>;
  seedDemoData(): Promise<null>;
  listImportSources(): Promise<ImportSourceStatus[]>;
  scanImportSource(source: ImportSource): Promise<DiscoveredGame[]>;
  importGames(source: ImportSource): Promise<ImportResult>;
  getSteamApiSettings(): Promise<SteamApiSettings>;
  setSteamApiSettings(settings: SteamApiSettings): Promise<null>;
  fetchSteamPlaytime(): Promise<ImportResult>;
  listLimits(): Promise<LimitStatus[]>;
  createLimit(input: LimitInput): Promise<Limit>;
  updateLimit(limitId: number, input: LimitInput): Promise<Limit>;
  deleteLimit(limitId: number): Promise<null>;
  listTags(): Promise<Tag[]>;
  createTag(name: string): Promise<Tag>;
  renameTag(tagId: number, name: string): Promise<Tag>;
  deleteTag(tagId: number): Promise<null>;
  setGameTags(gameId: number, tagIds: number[]): Promise<Tag[]>;
  mergeGames(survivorId: number, mergedIds: number[]): Promise<MergeResult>;
  pickImage(): Promise<string | null>;
  setGameCover(gameId: number, sourcePath: string | null): Promise<Game>;
  exportData(options: ExportOptions): Promise<ExportResult | null>;
  backupDatabase(): Promise<BackupInfo | null>;
  restoreDatabase(): Promise<null>;
  getAutostart(): Promise<boolean>;
  setAutostart(enabled: boolean): Promise<boolean>;
  checkForUpdate(): Promise<UpdateInfo | null>;
  installUpdate(): Promise<null>;
  /** Subscribe to a backend event; returns the unsubscribe function. */
  on<E extends ApiEventName>(
    event: E,
    handler: (payload: ApiEventMap[E]) => void,
  ): () => void;
}
