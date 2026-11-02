import { useEffect, useState } from "react";
import { useLocation } from "react-router";
import { openPath } from "@tauri-apps/plugin-opener";
import { toast } from "sonner";
import { Pencil, Plus, Trash2 } from "lucide-react";
import type {
  ExportFormat,
  ImportSource,
  Limit,
  Settings as AppSettings,
  Theme,
} from "@/api";
import { ConfirmDialog } from "@/components/confirm-dialog";
import { ImportDialog } from "@/components/import-dialog";
import { LimitBar } from "@/components/limit-bar";
import { LimitDialog } from "@/components/limit-dialog";
import { PageHeader } from "@/components/page-header";
import { ScanDialog } from "@/components/scan-dialog";
import { ErrorState, RowsSkeleton } from "@/components/query-state";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardAction,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { useAppInfo } from "@/hooks/use-app-info";
import {
  useBackupDatabase,
  useExportData,
  useRestoreDatabase,
} from "@/hooks/use-data";
import { useGames } from "@/hooks/use-games";
import { useDeleteLimit, useLimits } from "@/hooks/use-limits";
import {
  useFetchSteamPlaytime,
  useImportSources,
  useSetSteamApiSettings,
  useSteamApiSettings,
} from "@/hooks/use-import";
import {
  usePing,
  useSeedDemoData,
  useSettings,
  useUpdateSettings,
} from "@/hooks/use-settings";
import {
  useAutostart,
  useCheckForUpdate,
  useInstallUpdate,
  useSetAutostart,
  useUpdateCheckResult,
  useUpdateProgress,
} from "@/hooks/use-system";
import {
  useAddScanRoot,
  useRemoveScanRoot,
  useScanRoots,
} from "@/hooks/use-scan";
import { useSetTrackingEnabled } from "@/hooks/use-tracker";
import { formatDuration, formatRelativeShort } from "@/lib/format";
import { limitTitle } from "@/lib/limits";
import { SOURCE_LABELS } from "@/lib/import-sources";

function InfoRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="grid grid-cols-[8rem_1fr] gap-2 text-sm">
      <dt className="text-muted-foreground">{label}</dt>
      <dd className="font-mono break-all">{value}</dd>
    </div>
  );
}

function SwitchRow({
  id,
  label,
  description,
  checked,
  onCheckedChange,
}: {
  id: string;
  label: string;
  description?: string;
  checked: boolean;
  onCheckedChange: (checked: boolean) => void;
}) {
  return (
    <div className="flex items-center justify-between gap-4">
      <div className="space-y-0.5">
        <Label htmlFor={id}>{label}</Label>
        {description && (
          <p className="text-muted-foreground text-xs">{description}</p>
        )}
      </div>
      <Switch id={id} checked={checked} onCheckedChange={onCheckedChange} />
    </div>
  );
}

function NumberField({
  id,
  label,
  suffix,
  value,
  min,
  max,
  onChange,
}: {
  id: string;
  label: string;
  suffix: string;
  value: string;
  min: number;
  max?: number;
  onChange: (value: string) => void;
}) {
  return (
    <div className="space-y-2">
      <Label htmlFor={id}>{label}</Label>
      <div className="flex items-center gap-2">
        <Input
          id={id}
          type="number"
          min={min}
          max={max}
          className="w-28"
          value={value}
          onChange={(e) => onChange(e.target.value)}
        />
        <span className="text-muted-foreground text-sm">{suffix}</span>
      </div>
    </div>
  );
}

function GeneralCard() {
  const autostart = useAutostart();
  const setAutostart = useSetAutostart();

  return (
    <Card>
      <CardHeader>
        <CardTitle>General</CardTitle>
        <CardDescription>How Gamelapse runs on your PC.</CardDescription>
      </CardHeader>
      <CardContent>
        <div className="flex items-center justify-between gap-4">
          <div className="space-y-0.5">
            <Label htmlFor="autostart">Start with Windows</Label>
            <p className="text-muted-foreground text-xs">
              Launch minimized to the tray at sign-in so no session is missed.
            </p>
          </div>
          <Switch
            id="autostart"
            checked={autostart.data ?? false}
            disabled={autostart.isPending || setAutostart.isPending}
            onCheckedChange={(enabled) =>
              setAutostart.mutate(enabled, {
                onError: (e) => toast.error(e.message),
              })
            }
          />
        </div>
      </CardContent>
    </Card>
  );
}

function UpdatesCard() {
  const info = useAppInfo();
  const last = useUpdateCheckResult();
  const check = useCheckForUpdate();
  const install = useInstallUpdate();
  const progress = useUpdateProgress(install.isPending);
  const update = last.data ?? null;

  const percent =
    progress && progress.total
      ? Math.min(100, Math.round((progress.downloaded / progress.total) * 100))
      : null;

  return (
    <Card>
      <CardHeader>
        <CardTitle>Updates</CardTitle>
        <CardDescription>
          Current version {info.data?.version ?? "…"}
        </CardDescription>
        <CardAction>
          <Button
            variant="outline"
            size="sm"
            disabled={check.isPending || install.isPending}
            onClick={() =>
              check.mutate(undefined, {
                onError: (e) =>
                  toast.error(`Update check failed: ${e.message}`),
              })
            }
          >
            {check.isPending ? "Checking…" : "Check for updates"}
          </Button>
        </CardAction>
      </CardHeader>
      <CardContent className="space-y-4">
        {last.data === undefined ? (
          <p className="text-muted-foreground text-sm">
            No update check yet this session.
          </p>
        ) : update === null ? (
          <p className="text-sm">Up to date</p>
        ) : (
          <div className="space-y-3">
            <p className="text-sm font-medium">
              Version {update.version} available
            </p>
            {update.date && (
              <p className="text-muted-foreground text-xs">
                Released {new Date(update.date).toLocaleDateString()}
              </p>
            )}
            {update.notes && (
              <p className="text-muted-foreground text-sm whitespace-pre-wrap">
                {update.notes}
              </p>
            )}
            {install.isPending && (
              <div
                role="progressbar"
                aria-label="Update download"
                aria-valuemin={0}
                aria-valuemax={100}
                aria-valuenow={percent ?? undefined}
                className="bg-muted h-2 w-full overflow-hidden rounded-full"
              >
                <div
                  className={
                    percent === null
                      ? "bg-primary h-full w-1/3 animate-pulse"
                      : "bg-primary h-full transition-all"
                  }
                  style={
                    percent === null ? undefined : { width: `${percent}%` }
                  }
                />
              </div>
            )}
            <Button
              disabled={install.isPending}
              onClick={() =>
                install.mutate(undefined, {
                  onError: (e) => toast.error(`Update failed: ${e.message}`),
                })
              }
            >
              {install.isPending ? "Downloading…" : "Install and restart"}
            </Button>
          </div>
        )}
      </CardContent>
    </Card>
  );
}

function TrackingCard({ settings }: { settings: AppSettings }) {
  const update = useUpdateSettings();
  const setTracking = useSetTrackingEnabled();
  const [poll, setPoll] = useState(String(settings.pollIntervalSecs));
  const [afk, setAfk] = useState(String(settings.afkThresholdSecs / 60));
  const [gap, setGap] = useState(String(settings.sessionMergeGapSecs / 60));

  const pollN = Number(poll);
  const afkN = Number(afk);
  const gapN = Number(gap);
  const valid =
    Number.isInteger(pollN) &&
    pollN >= 1 &&
    pollN <= 60 &&
    afk !== "" &&
    afkN >= 1 &&
    gap !== "" &&
    gapN >= 0;
  const dirty =
    pollN !== settings.pollIntervalSecs ||
    Math.round(afkN * 60) !== settings.afkThresholdSecs ||
    Math.round(gapN * 60) !== settings.sessionMergeGapSecs;

  return (
    <Card>
      <CardHeader>
        <CardTitle>Tracking</CardTitle>
        <CardDescription>
          How games are detected. The tracker reads changes on its next poll.
        </CardDescription>
      </CardHeader>
      <CardContent className="space-y-5">
        <SwitchRow
          id="tracking-enabled"
          label="Tracking enabled"
          description="Turning this off ends automatic sessions. Manual sessions keep running."
          checked={settings.trackingEnabled}
          onCheckedChange={(enabled) => setTracking.mutate(enabled)}
        />
        <SwitchRow
          id="require-foreground"
          label="Require foreground"
          description="Only count a game while its window is in front."
          checked={settings.requireForeground}
          onCheckedChange={(requireForeground) =>
            update.mutate({ requireForeground })
          }
        />
        <div className="flex items-center justify-between gap-4">
          <Label htmlFor="theme">Theme</Label>
          <Select
            value={settings.theme}
            onValueChange={(theme) => update.mutate({ theme: theme as Theme })}
          >
            <SelectTrigger id="theme" className="w-36">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="dark">Dark</SelectItem>
              <SelectItem value="light">Light</SelectItem>
              <SelectItem value="system">System</SelectItem>
            </SelectContent>
          </Select>
        </div>
        <div className="flex flex-wrap gap-6">
          <NumberField
            id="poll-interval"
            label="Poll interval"
            suffix="seconds (1-60)"
            value={poll}
            min={1}
            max={60}
            onChange={setPoll}
          />
          <NumberField
            id="afk-threshold"
            label="AFK threshold"
            suffix="minutes"
            value={afk}
            min={1}
            onChange={setAfk}
          />
          <NumberField
            id="merge-gap"
            label="Merge gap"
            suffix="minutes"
            value={gap}
            min={0}
            onChange={setGap}
          />
        </div>
        <Button
          disabled={!valid || !dirty || update.isPending}
          onClick={() =>
            update.mutate(
              {
                pollIntervalSecs: pollN,
                afkThresholdSecs: Math.round(afkN * 60),
                sessionMergeGapSecs: Math.round(gapN * 60),
              },
              { onSuccess: () => toast.success("Settings saved") },
            )
          }
        >
          Save
        </Button>
      </CardContent>
    </Card>
  );
}

const REMINDER_CHOICES = [
  { secs: 0, label: "Off" },
  { secs: 1800, label: "30 min" },
  { secs: 3600, label: "1 h" },
  { secs: 7200, label: "2 h" },
  { secs: 10800, label: "3 h" },
];

function LimitsCard({ settings }: { settings: AppSettings }) {
  const limits = useLimits();
  const games = useGames(true);
  const remove = useDeleteLimit();
  const update = useUpdateSettings();
  const [editing, setEditing] = useState<Limit | null>(null);
  const [adding, setAdding] = useState(false);
  const [deleting, setDeleting] = useState<Limit | null>(null);

  const current = settings.reminderIntervalSecs;
  const choices = REMINDER_CHOICES.some((c) => c.secs === current)
    ? REMINDER_CHOICES
    : [...REMINDER_CHOICES, { secs: current, label: formatDuration(current) }];

  return (
    <Card>
      <CardHeader>
        <CardTitle>Limits</CardTitle>
        <CardDescription>
          Daily playtime caps and reminders. AFK time does not count.
        </CardDescription>
        <CardAction>
          <Button size="sm" onClick={() => setAdding(true)}>
            <Plus /> Add limit
          </Button>
        </CardAction>
      </CardHeader>
      <CardContent className="space-y-5">
        {limits.isPending && <RowsSkeleton rows={2} />}
        {limits.error && (
          <ErrorState error={limits.error} onRetry={() => limits.refetch()} />
        )}
        {limits.data?.length === 0 && (
          <p className="text-muted-foreground text-sm">No limits set.</p>
        )}
        <ul className="divide-y">
          {limits.data?.map((status) => (
            <li key={status.limit.id} className="flex items-center gap-3 py-3">
              <div className="min-w-0 flex-1 space-y-2">
                <div className="flex flex-wrap items-center gap-2">
                  <span className="truncate font-medium">
                    {limitTitle(status)}
                  </span>
                  <Badge variant="secondary">
                    {formatDuration(status.limit.dailySecs)} per day
                  </Badge>
                  {status.limit.action === "notify_repeat" && (
                    <Badge variant="outline">Repeats</Badge>
                  )}
                </div>
                <LimitBar status={status} />
              </div>
              <Button
                variant="ghost"
                size="icon"
                aria-label={`Edit limit for ${limitTitle(status)}`}
                onClick={() => setEditing(status.limit)}
              >
                <Pencil />
              </Button>
              <Button
                variant="ghost"
                size="icon"
                aria-label={`Delete limit for ${limitTitle(status)}`}
                onClick={() => setDeleting(status.limit)}
              >
                <Trash2 />
              </Button>
            </li>
          ))}
        </ul>
        <SwitchRow
          id="notifications-enabled"
          label="Notifications"
          description="Show a system notification when a limit or reminder fires."
          checked={settings.notificationsEnabled}
          onCheckedChange={(notificationsEnabled) =>
            update.mutate({ notificationsEnabled })
          }
        />
        <div className="flex items-center justify-between gap-4">
          <div className="space-y-0.5">
            <Label htmlFor="reminder-interval">Play reminder</Label>
            <p className="text-muted-foreground text-xs">
              Remind me every so often while a game is running.
            </p>
          </div>
          <Select
            value={String(current)}
            onValueChange={(v) =>
              update.mutate({ reminderIntervalSecs: Number(v) })
            }
          >
            <SelectTrigger id="reminder-interval" className="w-32">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {choices.map((c) => (
                <SelectItem key={c.secs} value={String(c.secs)}>
                  {c.label}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
        {update.error && (
          <p role="alert" className="text-destructive text-sm">
            {update.error.message}
          </p>
        )}
      </CardContent>
      {adding && (
        <LimitDialog
          open
          onOpenChange={(open) => !open && setAdding(false)}
          games={games.data ?? []}
        />
      )}
      {editing && (
        <LimitDialog
          open
          onOpenChange={(open) => !open && setEditing(null)}
          games={games.data ?? []}
          limit={editing}
        />
      )}
      <ConfirmDialog
        open={deleting !== null}
        onOpenChange={(open) => !open && setDeleting(null)}
        title="Delete this limit?"
        description="Playtime is no longer checked against it."
        onConfirm={() => deleting && remove.mutate(deleting.id)}
      />
    </Card>
  );
}

function GamesFoldersCard() {
  const roots = useScanRoots();
  const addRoot = useAddScanRoot();
  const removeRoot = useRemoveScanRoot();
  const [scanOpen, setScanOpen] = useState(false);
  const { hash } = useLocation();

  useEffect(() => {
    if (hash === "#games-folders" && roots.data) {
      document
        .getElementById("games-folders")
        ?.scrollIntoView({ behavior: "smooth" });
    }
  }, [hash, roots.data]);

  const error = addRoot.error ?? removeRoot.error;

  return (
    <Card id="games-folders">
      <CardHeader>
        <CardTitle>Games folders</CardTitle>
        <CardDescription>
          Gamelapse scans only when you click Scan and adds only what you
          confirm.
        </CardDescription>
        <CardAction>
          <Button
            variant="outline"
            size="sm"
            disabled={addRoot.isPending}
            onClick={() => addRoot.mutate()}
          >
            <Plus /> Add folder
          </Button>
        </CardAction>
      </CardHeader>
      <CardContent className="space-y-4">
        {roots.isPending && <RowsSkeleton rows={2} />}
        {roots.error && (
          <ErrorState error={roots.error} onRetry={() => roots.refetch()} />
        )}
        {error && (
          <p role="alert" className="text-destructive text-sm">
            {error.message}
          </p>
        )}
        {roots.data?.length === 0 && (
          <p className="text-muted-foreground text-sm">
            No folders yet. Add the folder where you keep your games.
          </p>
        )}
        <ul className="divide-y">
          {roots.data?.map((r) => (
            <li key={r.id} className="flex items-center gap-3 py-2">
              <span className="min-w-0 flex-1 font-mono text-sm break-all">
                {r.path}
              </span>
              <Button
                variant="ghost"
                size="sm"
                aria-label={`Remove ${r.path}`}
                disabled={removeRoot.isPending}
                onClick={() => removeRoot.mutate(r.id)}
              >
                <Trash2 /> Remove
              </Button>
            </li>
          ))}
        </ul>
        <Button
          variant="outline"
          disabled={!roots.data?.length}
          onClick={() => setScanOpen(true)}
        >
          Scan folders
        </Button>
      </CardContent>
      {scanOpen && <ScanDialog open onOpenChange={setScanOpen} />}
    </Card>
  );
}

function ImportCard({ settings }: { settings: AppSettings }) {
  const sources = useImportSources();
  const update = useUpdateSettings();
  const [scanSource, setScanSource] = useState<ImportSource | null>(null);

  return (
    <Card>
      <CardHeader>
        <CardTitle>Import</CardTitle>
        <CardDescription>
          Add games from the launchers installed on this PC.
        </CardDescription>
      </CardHeader>
      <CardContent className="space-y-5">
        {sources.isPending && <RowsSkeleton rows={3} />}
        {sources.error && (
          <ErrorState error={sources.error} onRetry={() => sources.refetch()} />
        )}
        <ul className="divide-y">
          {sources.data?.map((s) => (
            <li key={s.source} className="flex items-center gap-3 py-3">
              <div className="min-w-0 flex-1 space-y-0.5">
                <div className="flex items-center gap-2">
                  <span className="font-medium">{SOURCE_LABELS[s.source]}</span>
                  <Badge variant={s.available ? "secondary" : "outline"}>
                    {s.available ? "Available" : "Not found"}
                  </Badge>
                </div>
                {s.detail && (
                  <p className="text-muted-foreground text-xs break-all">
                    {s.detail}
                  </p>
                )}
                <p className="text-muted-foreground text-xs">
                  Last scan: {formatRelativeShort(s.lastScanAt)}
                </p>
              </div>
              <Button
                variant="outline"
                size="sm"
                disabled={!s.available}
                aria-label={`Scan ${SOURCE_LABELS[s.source]}`}
                onClick={() => setScanSource(s.source)}
              >
                Scan
              </Button>
            </li>
          ))}
        </ul>
        <SwitchRow
          id="import-on-startup"
          label="Import on startup"
          description="Re-scan every available launcher when the app starts."
          checked={settings.importOnStartup}
          onCheckedChange={(importOnStartup) =>
            update.mutate({ importOnStartup })
          }
        />
      </CardContent>
      {scanSource && (
        <ImportDialog
          open
          onOpenChange={(open) => {
            if (!open) setScanSource(null);
          }}
          initialSource={scanSource}
        />
      )}
    </Card>
  );
}

type PeriodPreset = "all" | "30d" | "90d" | "year";

const PERIODS: { value: PeriodPreset; label: string }[] = [
  { value: "all", label: "All time" },
  { value: "30d", label: "Last 30 days" },
  { value: "90d", label: "Last 90 days" },
  { value: "year", label: "This year" },
];

function periodStart(preset: PeriodPreset): number | null {
  const nowSecs = Math.floor(Date.now() / 1000);
  switch (preset) {
    case "30d":
      return nowSecs - 30 * 86_400;
    case "90d":
      return nowSecs - 90 * 86_400;
    case "year":
      return Math.floor(
        new Date(new Date().getFullYear(), 0, 1).getTime() / 1000,
      );
    default:
      return null;
  }
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function DataCard() {
  const games = useGames(true);
  const exportData = useExportData();
  const backup = useBackupDatabase();
  const restore = useRestoreDatabase();
  const [format, setFormat] = useState<ExportFormat>("csv");
  const [period, setPeriod] = useState<PeriodPreset>("all");
  const [gameId, setGameId] = useState("all");
  const [restoreOpen, setRestoreOpen] = useState(false);

  const error = exportData.error ?? backup.error ?? restore.error;

  return (
    <Card>
      <CardHeader>
        <CardTitle>Data</CardTitle>
        <CardDescription>
          Export your sessions, back up the database or restore a backup.
        </CardDescription>
      </CardHeader>
      <CardContent className="space-y-6">
        <div className="space-y-3">
          <h3 className="text-sm font-medium">Export sessions</h3>
          <div className="flex flex-wrap items-end gap-4">
            <div className="space-y-2">
              <Label htmlFor="export-format">Format</Label>
              <Select
                value={format}
                onValueChange={(v) => setFormat(v as ExportFormat)}
              >
                <SelectTrigger id="export-format" className="w-28">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="csv">CSV</SelectItem>
                  <SelectItem value="json">JSON</SelectItem>
                </SelectContent>
              </Select>
            </div>
            <div className="space-y-2">
              <Label htmlFor="export-period">Period</Label>
              <Select
                value={period}
                onValueChange={(v) => setPeriod(v as PeriodPreset)}
              >
                <SelectTrigger id="export-period" className="w-40">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {PERIODS.map((p) => (
                    <SelectItem key={p.value} value={p.value}>
                      {p.label}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
            <div className="space-y-2">
              <Label htmlFor="export-game">Game</Label>
              <Select value={gameId} onValueChange={setGameId}>
                <SelectTrigger id="export-game" className="w-48">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="all">All games</SelectItem>
                  {games.data?.map((g) => (
                    <SelectItem key={g.game.id} value={String(g.game.id)}>
                      {g.game.name}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
            <Button
              disabled={exportData.isPending}
              onClick={() =>
                exportData.mutate(
                  {
                    format,
                    from: periodStart(period),
                    to: null,
                    gameId: gameId === "all" ? null : Number(gameId),
                  },
                  {
                    onSuccess: (r) => {
                      if (r) {
                        toast.success(`Exported ${r.sessions} sessions`, {
                          description: r.path,
                        });
                      }
                    },
                  },
                )
              }
            >
              Export
            </Button>
          </div>
        </div>
        <div className="flex flex-wrap items-center gap-3">
          <Button
            variant="outline"
            disabled={backup.isPending}
            onClick={() =>
              backup.mutate(undefined, {
                onSuccess: (b) => {
                  if (b) {
                    toast.success("Backup saved", {
                      description: `${b.path} (${formatBytes(b.sizeBytes)})`,
                    });
                  }
                },
              })
            }
          >
            Back up now
          </Button>
          <Button
            variant="destructive"
            disabled={restore.isPending}
            onClick={() => setRestoreOpen(true)}
          >
            Restore from backup
          </Button>
        </div>
        {error && (
          <p role="alert" className="text-destructive text-sm">
            {error.message}
          </p>
        )}
      </CardContent>
      <ConfirmDialog
        open={restoreOpen}
        onOpenChange={setRestoreOpen}
        title="Restore from a backup?"
        description="Your current data is replaced by the backup you pick. The current database is kept next to it as gamelapse.db.bak. Stop any running session first."
        confirmLabel="Choose backup and restore"
        onConfirm={() =>
          restore.mutate(undefined, {
            onSuccess: () => toast.success("Backup restored"),
          })
        }
      />
    </Card>
  );
}

function SteamApiCard() {
  const saved = useSteamApiSettings();
  const save = useSetSteamApiSettings();
  const fetchPlaytime = useFetchSteamPlaytime();
  const [apiKey, setApiKey] = useState<string | null>(null);
  const [steamId, setSteamId] = useState<string | null>(null);

  const keyValue = apiKey ?? saved.data?.apiKey ?? "";
  const idValue = steamId ?? saved.data?.steamId64 ?? "";
  const dirty =
    keyValue !== (saved.data?.apiKey ?? "") ||
    idValue !== (saved.data?.steamId64 ?? "");

  return (
    <Card>
      <CardHeader>
        <CardTitle>Steam Web API</CardTitle>
        <CardDescription>
          Fetch your Steam playtime. Get a key at steamcommunity.com/dev/apikey,
          and make sure the game details of your profile are public.
        </CardDescription>
      </CardHeader>
      <CardContent className="space-y-4">
        <div className="grid gap-4 sm:grid-cols-2">
          <div className="space-y-2">
            <Label htmlFor="steam-api-key">API key</Label>
            <Input
              id="steam-api-key"
              type="password"
              autoComplete="off"
              value={keyValue}
              onChange={(e) => setApiKey(e.target.value)}
            />
          </div>
          <div className="space-y-2">
            <Label htmlFor="steam-id">SteamID64</Label>
            <Input
              id="steam-id"
              inputMode="numeric"
              autoComplete="off"
              value={idValue}
              onChange={(e) => setSteamId(e.target.value)}
            />
          </div>
        </div>
        <div className="flex flex-wrap items-center gap-3">
          <Button
            disabled={!dirty || save.isPending}
            onClick={() =>
              save.mutate(
                {
                  apiKey: keyValue.trim() || null,
                  steamId64: idValue.trim() || null,
                },
                {
                  onSuccess: () => {
                    setApiKey(null);
                    setSteamId(null);
                    toast.success("Steam settings saved");
                  },
                },
              )
            }
          >
            Save
          </Button>
          <Button
            variant="outline"
            disabled={dirty || fetchPlaytime.isPending}
            onClick={() => fetchPlaytime.mutate()}
          >
            Fetch playtime
          </Button>
        </div>
        {save.error && (
          <p role="alert" className="text-destructive text-sm">
            {save.error.message}
          </p>
        )}
        {fetchPlaytime.error && (
          <p role="alert" className="text-destructive text-sm">
            {fetchPlaytime.error.message}
          </p>
        )}
        {fetchPlaytime.data && (
          <div className="space-y-1 text-sm">
            <p>
              Updated playtime for {fetchPlaytime.data.updated} of{" "}
              {fetchPlaytime.data.discovered} Steam games.
            </p>
            {fetchPlaytime.data.errors.length > 0 && (
              <ul className="text-destructive list-disc pl-5">
                {fetchPlaytime.data.errors.map((e, i) => (
                  <li key={i}>{e}</li>
                ))}
              </ul>
            )}
          </div>
        )}
      </CardContent>
    </Card>
  );
}

function DeveloperCard() {
  const seed = useSeedDemoData();
  const ping = usePing();
  return (
    <Card>
      <CardHeader>
        <CardTitle>Developer</CardTitle>
        <CardDescription>Tools for testing the app.</CardDescription>
      </CardHeader>
      <CardContent className="space-y-3">
        <div className="flex flex-wrap items-center gap-3">
          <Button
            variant="outline"
            disabled={seed.isPending}
            onClick={() => seed.mutate()}
          >
            Seed demo data
          </Button>
          {seed.isSuccess && (
            <span className="text-muted-foreground text-sm">
              Demo data is in place.
            </span>
          )}
        </div>
        <div className="flex flex-wrap items-center gap-3">
          <Button
            variant="outline"
            disabled={ping.isPending}
            onClick={() => ping.mutate()}
          >
            Ping
          </Button>
          {ping.isSuccess && (
            <span className="font-mono text-sm">{ping.data}</span>
          )}
        </div>
      </CardContent>
    </Card>
  );
}

export default function Settings() {
  const info = useAppInfo();
  const settings = useSettings();

  return (
    <div className="space-y-6">
      <PageHeader
        title="Settings"
        description="Application information and preferences."
      />

      {settings.isPending && <RowsSkeleton rows={4} />}
      {settings.error && (
        <ErrorState error={settings.error} onRetry={() => settings.refetch()} />
      )}
      <GeneralCard />
      {settings.data && <TrackingCard settings={settings.data} />}
      {settings.data && <LimitsCard settings={settings.data} />}
      {settings.data && <ImportCard settings={settings.data} />}
      <GamesFoldersCard />
      <SteamApiCard />
      <DataCard />
      <UpdatesCard />

      <Card>
        <CardHeader>
          <CardTitle>About</CardTitle>
          <CardDescription>Where Gamelapse keeps its data.</CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          {info.isPending && <RowsSkeleton rows={2} />}
          {info.error && (
            <ErrorState error={info.error} onRetry={() => info.refetch()} />
          )}
          {info.data && (
            <>
              <dl className="space-y-2">
                <InfoRow label="Version" value={info.data.version} />
                <InfoRow label="Data folder" value={info.data.dataDir} />
                <InfoRow label="Database" value={info.data.dbPath} />
                <InfoRow label="Log folder" value={info.data.logDir} />
              </dl>
              <Button
                variant="outline"
                onClick={() =>
                  openPath(info.data.dataDir).catch((e) =>
                    toast.error(String(e)),
                  )
                }
              >
                Open data folder
              </Button>
            </>
          )}
        </CardContent>
      </Card>

      <DeveloperCard />
    </div>
  );
}
