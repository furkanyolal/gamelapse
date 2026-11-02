/**
 * Compile-time check that the hand-written contract types in `./types` match the
 * generated `src/bindings.ts`. Nothing here runs; `tsc` fails when the shapes drift.
 * Outputs must match both ways. Inputs only need our stricter shape to be accepted.
 */
import type * as B from "@/bindings";
import type * as T from "./types";

type Mutual<X, Y> = [X] extends [Y] ? ([Y] extends [X] ? true : false) : false;
type Accepts<Input, Expected> = [Input] extends [Expected] ? true : false;
type Assert<C extends true> = C;

export type OutputChecks = [
  Assert<Mutual<B.AppInfo, T.AppInfo>>,
  Assert<Mutual<B.ActiveSession, T.ActiveSession>>,
  Assert<Mutual<B.TrackerStatus_Serialize, T.TrackerStatus>>,
  Assert<Mutual<B.Game_Serialize, T.Game>>,
  Assert<Mutual<B.GameExecutable, T.GameExecutable>>,
  Assert<Mutual<B.Session_Serialize, T.Session>>,
  Assert<Mutual<B.SessionWithGame_Serialize, T.SessionWithGame>>,
  Assert<Mutual<B.GameSummary_Serialize, T.GameSummary>>,
  Assert<Mutual<B.GameTotal, T.GameTotal>>,
  Assert<Mutual<B.DailyTotal, T.DailyTotal>>,
  Assert<Mutual<B.HourlyTotal, T.HourlyTotal>>,
  Assert<Mutual<B.DashboardStats_Serialize, T.DashboardStats>>,
  Assert<Mutual<B.Settings, T.Settings>>,
  Assert<Mutual<B.GameSource, T.GameSource>>,
  Assert<Mutual<B.SessionSource, T.SessionSource>>,
  Assert<Mutual<B.MatchMode, T.MatchMode>>,
  Assert<Mutual<B.Theme, T.Theme>>,
  Assert<Mutual<B.ImportSource, T.ImportSource>>,
  Assert<Mutual<B.ImportSourceStatus_Serialize, T.ImportSourceStatus>>,
  Assert<Mutual<B.DiscoveredGame_Serialize, T.DiscoveredGame>>,
  Assert<Mutual<B.ImportResult, T.ImportResult>>,
  Assert<Mutual<B.SteamApiSettings_Serialize, T.SteamApiSettings>>,
];

export type InputChecks = [
  Assert<Accepts<T.SettingsUpdate, B.SettingsUpdate_Deserialize>>,
  Assert<Accepts<T.GameUpdate, B.GameUpdate_Deserialize>>,
  Assert<Accepts<T.SessionUpdate, B.SessionUpdate_Deserialize>>,
  Assert<Accepts<T.SessionQuery, B.SessionQuery_Deserialize>>,
  Assert<Accepts<T.SteamApiSettings, B.SteamApiSettings_Deserialize>>,
];
