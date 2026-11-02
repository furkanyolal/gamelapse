import { useState } from "react";
import { Link, useNavigate, useParams } from "react-router";
import { ArrowLeft, Merge, Pencil, Play, Plus, Trash2, X } from "lucide-react";
import { toast } from "sonner";
import type { GameSummary, MatchMode } from "@/api";
import { DailyChart } from "@/components/charts";
import { ConfirmDialog } from "@/components/confirm-dialog";
import { GameCover } from "@/components/game-cover";
import { CoverCard, MergeDialog, TagsCard } from "@/components/game-management";
import { EmptyState, ErrorState, RowsSkeleton } from "@/components/query-state";
import { StatCard } from "@/components/stat-card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Skeleton } from "@/components/ui/skeleton";
import { Switch } from "@/components/ui/switch";
import {
  useAddGameExecutable,
  useDeleteGame,
  useGame,
  useGameExecutables,
  useRemoveGameExecutable,
  useUpdateGame,
} from "@/hooks/use-games";
import { useSessions } from "@/hooks/use-sessions";
import { useDailyTotals } from "@/hooks/use-stats";
import { useStartManualSession, useTrackerStatus } from "@/hooks/use-tracker";
import {
  formatDateTime,
  formatDuration,
  formatRelativeShort,
} from "@/lib/format";

const MATCH_MODES: { value: MatchMode; label: string }[] = [
  { value: "exact_path", label: "Exact path" },
  { value: "install_dir_prefix", label: "Install folder" },
  { value: "exe_name", label: "File name" },
];

function RenameDialog({
  open,
  onOpenChange,
  summary,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  summary: GameSummary;
}) {
  const update = useUpdateGame(summary.game.id);
  const [name, setName] = useState(summary.game.name);
  const trimmed = name.trim();
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <form
          className="space-y-4"
          onSubmit={(e) => {
            e.preventDefault();
            if (!trimmed) return;
            update.mutate(
              { name: trimmed },
              { onSuccess: () => onOpenChange(false) },
            );
          }}
        >
          <DialogHeader>
            <DialogTitle>Rename game</DialogTitle>
            <DialogDescription>
              Changes the name shown everywhere in the app.
            </DialogDescription>
          </DialogHeader>
          <div className="space-y-2">
            <Label htmlFor="game-name">Name</Label>
            <Input
              id="game-name"
              value={name}
              onChange={(e) => setName(e.target.value)}
            />
          </div>
          <DialogFooter>
            <Button type="submit" disabled={!trimmed || update.isPending}>
              Save
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

function GameDetailBody({ summary }: { summary: GameSummary }) {
  const { game } = summary;
  const navigate = useNavigate();
  const status = useTrackerStatus();
  const start = useStartManualSession();
  const update = useUpdateGame(game.id);
  const del = useDeleteGame();
  const executables = useGameExecutables(game.id);
  const addExe = useAddGameExecutable(game.id);
  const removeExe = useRemoveGameExecutable();
  const daily = useDailyTotals(30, game.id);
  const sessions = useSessions({
    gameId: game.id,
    from: null,
    to: null,
    limit: 20,
    offset: 0,
  });

  const [renameOpen, setRenameOpen] = useState(false);
  const [deleteOpen, setDeleteOpen] = useState(false);
  const [mergeOpen, setMergeOpen] = useState(false);
  const [matchMode, setMatchMode] = useState<MatchMode>("exact_path");

  const playing = status.data?.active.some((a) => a.gameId === game.id);

  return (
    <div className="space-y-6">
      <div className="space-y-3">
        <Button asChild variant="ghost" size="sm" className="-ml-2">
          <Link to="/games">
            <ArrowLeft /> Games
          </Link>
        </Button>
        <div className="flex flex-wrap items-center gap-3">
          <GameCover coverPath={game.coverPath} className="h-16 w-32" />
          <h1 className="text-2xl font-semibold tracking-tight">{game.name}</h1>
          {game.hidden && <Badge variant="outline">Hidden</Badge>}
          {playing && <Badge>Playing</Badge>}
          <div className="ml-auto flex gap-2">
            <Button variant="outline" onClick={() => setRenameOpen(true)}>
              <Pencil /> Rename
            </Button>
            <Button variant="outline" onClick={() => setMergeOpen(true)}>
              <Merge /> Merge games
            </Button>
            <Button
              variant="outline"
              disabled={playing || start.isPending}
              onClick={() => start.mutate(game.id)}
            >
              <Play /> Start session
            </Button>
            <Button variant="destructive" onClick={() => setDeleteOpen(true)}>
              <Trash2 /> Delete
            </Button>
          </div>
        </div>
      </div>

      <div className="grid gap-4 sm:grid-cols-3 lg:grid-cols-6">
        <StatCard label="Total" value={formatDuration(summary.totalSecs)} />
        <StatCard label="Today" value={formatDuration(summary.todaySecs)} />
        <StatCard label="This week" value={formatDuration(summary.weekSecs)} />
        <StatCard label="Sessions" value={String(summary.sessionCount)} />
        <StatCard
          label="Last played"
          value={formatRelativeShort(summary.lastPlayedAt)}
        />
        {summary.importedMinutes > 0 && (
          <StatCard
            label="Imported playtime"
            value={formatDuration(summary.importedMinutes * 60)}
          />
        )}
      </div>

      <TagsCard summary={summary} />
      <CoverCard summary={summary} />

      <Card>
        <CardHeader>
          <CardTitle>Options</CardTitle>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="flex items-center justify-between">
            <Label htmlFor="game-hidden">Hide from library</Label>
            <Switch
              id="game-hidden"
              checked={game.hidden}
              onCheckedChange={(hidden) => update.mutate({ hidden })}
            />
          </div>
          <div className="flex items-center justify-between">
            <Label htmlFor="game-afk">AFK detection</Label>
            <Switch
              id="game-afk"
              checked={game.afkDetectionEnabled}
              onCheckedChange={(afkDetectionEnabled) =>
                update.mutate({ afkDetectionEnabled })
              }
            />
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Executables</CardTitle>
          <CardDescription>
            Processes that count as playing this game.
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          {executables.isPending && <RowsSkeleton rows={2} />}
          {executables.error && <ErrorState error={executables.error} />}
          {executables.data?.length === 0 && (
            <p className="text-muted-foreground text-sm">
              No executables. Add one so the game is detected automatically.
            </p>
          )}
          <ul className="divide-y">
            {executables.data?.map((e) => (
              <li key={e.id} className="flex items-center gap-3 py-2 text-sm">
                <span className="min-w-0 flex-1 font-mono break-all">
                  {e.exePath}
                </span>
                <Badge variant="outline">
                  {MATCH_MODES.find((m) => m.value === e.matchMode)?.label}
                </Badge>
                <Button
                  variant="ghost"
                  size="icon-sm"
                  aria-label={`Remove ${e.exePath}`}
                  onClick={() => removeExe.mutate(e.id)}
                >
                  <X />
                </Button>
              </li>
            ))}
          </ul>
          <div className="flex flex-wrap items-center gap-2">
            <Select
              value={matchMode}
              onValueChange={(v) => setMatchMode(v as MatchMode)}
            >
              <SelectTrigger aria-label="Match mode" className="w-44">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {MATCH_MODES.map((m) => (
                  <SelectItem key={m.value} value={m.value}>
                    {m.label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
            <Button
              variant="outline"
              disabled={addExe.isPending}
              onClick={() =>
                addExe.mutate(matchMode, {
                  onSuccess: (exe) => {
                    if (exe) toast.success("Executable added");
                  },
                })
              }
            >
              <Plus /> Add executable
            </Button>
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Last 30 days</CardTitle>
          <CardDescription>Hours played per day.</CardDescription>
        </CardHeader>
        <CardContent>
          {daily.error ? (
            <ErrorState error={daily.error} />
          ) : daily.data ? (
            <DailyChart data={daily.data} />
          ) : (
            <Skeleton className="h-[220px] w-full" />
          )}
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Sessions</CardTitle>
          <CardDescription>
            Most recent 20.{" "}
            <Link to="/sessions" className="hover:underline">
              All sessions
            </Link>
          </CardDescription>
        </CardHeader>
        <CardContent>
          {sessions.isPending && <RowsSkeleton />}
          {sessions.error && <ErrorState error={sessions.error} />}
          {sessions.data?.length === 0 && (
            <p className="text-muted-foreground text-sm">No sessions yet.</p>
          )}
          <ul className="divide-y">
            {sessions.data?.map((s) => (
              <li
                key={s.id}
                className="flex items-center justify-between py-2 text-sm"
              >
                <span>{formatDateTime(s.startedAt)}</span>
                {s.endedAt === null ? (
                  <Badge>In progress</Badge>
                ) : (
                  <span className="text-muted-foreground tabular-nums">
                    {formatDuration(s.durationSecs)}
                  </span>
                )}
              </li>
            ))}
          </ul>
        </CardContent>
      </Card>

      {mergeOpen && (
        <MergeDialog open onOpenChange={setMergeOpen} summary={summary} />
      )}
      {renameOpen && (
        <RenameDialog open onOpenChange={setRenameOpen} summary={summary} />
      )}
      <ConfirmDialog
        open={deleteOpen}
        onOpenChange={setDeleteOpen}
        title={`Delete ${game.name}?`}
        description="This removes the game, its executables and all of its sessions. It cannot be undone."
        onConfirm={() =>
          del.mutate(game.id, { onSuccess: () => navigate("/games") })
        }
      />
    </div>
  );
}

export default function GameDetail() {
  const { id } = useParams();
  const gameId = Number(id);
  const game = useGame(gameId);

  if (game.isPending) {
    return (
      <div className="space-y-4">
        <Skeleton className="h-8 w-64" />
        <RowsSkeleton rows={5} />
      </div>
    );
  }
  if (game.error) {
    return (
      <div className="space-y-4">
        <ErrorState error={game.error} onRetry={() => game.refetch()} />
        <Button asChild variant="outline">
          <Link to="/games">Back to games</Link>
        </Button>
      </div>
    );
  }
  if (!game.data) return <EmptyState title="Game not found" />;
  return <GameDetailBody summary={game.data} />;
}
