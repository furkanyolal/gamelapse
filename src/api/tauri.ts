import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { Api } from "./api";

async function call<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (e) {
    // Commands reject with the error message as a plain string.
    throw e instanceof Error ? e : new Error(String(e));
  }
}

export function createTauriApi(): Api {
  return {
    getAppInfo: () => call("get_app_info"),
    ping: () => call("ping"),
    getTrackerStatus: () => call("get_tracker_status"),
    setTrackingEnabled: (enabled) => call("set_tracking_enabled", { enabled }),
    startManualSession: (gameId) => call("start_manual_session", { gameId }),
    stopSession: (sessionId) => call("stop_session", { sessionId }),
    listGames: (includeHidden) => call("list_games", { includeHidden }),
    getGame: (gameId) => call("get_game", { gameId }),
    addGameFromExe: (exePath, name) =>
      call("add_game_from_exe", { exePath, name }),
    updateGame: (gameId, patch) => call("update_game", { gameId, patch }),
    deleteGame: (gameId) => call("delete_game", { gameId }),
    listGameExecutables: (gameId) => call("list_game_executables", { gameId }),
    addGameExecutable: (gameId, exePath, matchMode) =>
      call("add_game_executable", { gameId, exePath, matchMode }),
    removeGameExecutable: (executableId) =>
      call("remove_game_executable", { executableId }),
    pickExecutable: () => call("pick_executable"),
    listSessions: (query) => call("list_sessions", { query }),
    createSession: (gameId, startedAt, endedAt, note) =>
      call("create_session", { gameId, startedAt, endedAt, note }),
    updateSession: (sessionId, patch) =>
      call("update_session", { sessionId, patch }),
    deleteSession: (sessionId) => call("delete_session", { sessionId }),
    getDashboardStats: () => call("get_dashboard_stats"),
    getDailyTotals: (days, gameId) =>
      call("get_daily_totals", { days, gameId }),
    getGameTotals: (from, to, limit) =>
      call("get_game_totals", { from, to, limit }),
    getHourlyTotals: (days) => call("get_hourly_totals", { days }),
    getSettings: () => call("get_settings"),
    updateSettings: (patch) => call("update_settings", { patch }),
    seedDemoData: () => call("seed_demo_data"),
    listImportSources: () => call("list_import_sources"),
    scanImportSource: (source) => call("scan_import_source", { source }),
    importGames: (source) => call("import_games", { source }),
    listIgnoredImports: () => call("list_ignored_imports"),
    restoreIgnoredImport: (source, sourceId) =>
      call("restore_ignored_import", { source, sourceId }),
    getSteamApiSettings: () => call("get_steam_api_settings"),
    setSteamApiSettings: (settings) =>
      call("set_steam_api_settings", { settings }),
    fetchSteamPlaytime: () => call("fetch_steam_playtime"),
    listLimits: () => call("list_limits"),
    createLimit: (input) => call("create_limit", { input }),
    updateLimit: (limitId, input) => call("update_limit", { limitId, input }),
    deleteLimit: (limitId) => call("delete_limit", { limitId }),
    listTags: () => call("list_tags"),
    createTag: (name) => call("create_tag", { name }),
    renameTag: (tagId, name) => call("rename_tag", { tagId, name }),
    deleteTag: (tagId) => call("delete_tag", { tagId }),
    setGameTags: (gameId, tagIds) => call("set_game_tags", { gameId, tagIds }),
    mergeGames: (survivorId, mergedIds) =>
      call("merge_games", { survivorId, mergedIds }),
    pickImage: () => call("pick_image"),
    setGameCover: (gameId, sourcePath) =>
      call("set_game_cover", { gameId, sourcePath }),
    exportData: (options) => call("export_data", { options }),
    backupDatabase: () => call("backup_database"),
    restoreDatabase: () => call("restore_database"),
    getAutostart: () => call("get_autostart"),
    setAutostart: (enabled) => call("set_autostart", { enabled }),
    checkForUpdate: () => call("check_for_update"),
    installUpdate: () => call("install_update"),
    pickFolder: () => call("pick_folder"),
    listScanRoots: () => call("list_scan_roots"),
    addScanRoot: (path) => call("add_scan_root", { path }),
    removeScanRoot: (rootId) => call("remove_scan_root", { rootId }),
    scanFolders: (rootIds) => call("scan_folders", { rootIds }),
    addScannedGames: (selection) => call("add_scanned_games", { selection }),
    lookupGameMetadata: (name) => call("lookup_game_metadata", { name }),
    applyGameMetadata: (gameId, match, renameGame) =>
      call("apply_game_metadata", { gameId, match, renameGame }),
    on: (event, handler) => {
      // listen() is async; unsubscribe even if it is called before it resolves.
      let cancelled = false;
      let unlisten: (() => void) | undefined;
      void listen(event, (e) => handler(e.payload as never)).then((fn) => {
        if (cancelled) fn();
        else unlisten = fn;
      });
      return () => {
        cancelled = true;
        unlisten?.();
      };
    },
  };
}
