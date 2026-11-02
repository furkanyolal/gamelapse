import { useState } from "react";
import { Link } from "react-router";
import { Download, Play, Plus } from "lucide-react";
import { toast } from "sonner";
import { GameCover } from "@/components/game-cover";
import { ImportDialog } from "@/components/import-dialog";
import { PageHeader } from "@/components/page-header";
import { EmptyState, ErrorState, RowsSkeleton } from "@/components/query-state";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { useAddGameFromExe, useGames } from "@/hooks/use-games";
import { useStartManualSession, useTrackerStatus } from "@/hooks/use-tracker";
import { formatDuration, formatRelative } from "@/lib/format";

export default function Games() {
  const [search, setSearch] = useState("");
  const [showHidden, setShowHidden] = useState(false);
  const [importOpen, setImportOpen] = useState(false);
  const games = useGames(showHidden);
  const status = useTrackerStatus();
  const addGame = useAddGameFromExe();
  const start = useStartManualSession();

  const activeIds = new Set(status.data?.active.map((a) => a.gameId));
  const needle = search.trim().toLowerCase();
  const visible = games.data?.filter((g) =>
    g.game.name.toLowerCase().includes(needle),
  );

  function handleAdd() {
    addGame.mutate(null, {
      onSuccess: (game) => {
        if (game) toast.success(`Added ${game.name}`);
      },
    });
  }

  return (
    <div className="space-y-6">
      <div className="flex items-start justify-between gap-4">
        <PageHeader title="Games" description="Your library and time played." />
        <div className="flex gap-2">
          <Button variant="outline" onClick={() => setImportOpen(true)}>
            <Download /> Import
          </Button>
          <Button onClick={handleAdd} disabled={addGame.isPending}>
            <Plus /> Add game
          </Button>
        </div>
      </div>
      <ImportDialog open={importOpen} onOpenChange={setImportOpen} />

      <div className="flex flex-wrap items-center gap-4">
        <Input
          type="search"
          placeholder="Search games"
          aria-label="Search games"
          className="max-w-xs"
          value={search}
          onChange={(e) => setSearch(e.target.value)}
        />
        <div className="flex items-center gap-2">
          <Switch
            id="show-hidden"
            checked={showHidden}
            onCheckedChange={setShowHidden}
          />
          <Label htmlFor="show-hidden">Show hidden</Label>
        </div>
      </div>

      {games.isPending && <RowsSkeleton rows={6} />}
      {games.error && (
        <ErrorState error={games.error} onRetry={() => games.refetch()} />
      )}
      {games.data && games.data.length === 0 && (
        <EmptyState title="No games yet">
          <p>Pick a game executable to start tracking it.</p>
          <Button onClick={handleAdd} disabled={addGame.isPending}>
            <Plus /> Add game
          </Button>
        </EmptyState>
      )}
      {visible && games.data && games.data.length > 0 && (
        <>
          {visible.length === 0 && (
            <p className="text-muted-foreground text-sm">
              No games match your search.
            </p>
          )}
          <ul className="divide-y rounded-lg border">
            {visible.map((s) => (
              <li
                key={s.game.id}
                className="hover:bg-muted/40 flex items-center gap-4 px-4 py-3"
              >
                <GameCover coverPath={s.game.coverPath} className="h-10 w-20" />
                <Link
                  to={`/games/${s.game.id}`}
                  className="min-w-0 flex-1 space-y-0.5"
                >
                  <div className="flex items-center gap-2">
                    <span className="truncate font-medium">{s.game.name}</span>
                    {s.game.hidden && <Badge variant="outline">Hidden</Badge>}
                    {activeIds.has(s.game.id) && <Badge>Playing</Badge>}
                  </div>
                  <p className="text-muted-foreground text-xs">
                    Last played {formatRelative(s.lastPlayedAt)} ·{" "}
                    {s.sessionCount} session{s.sessionCount === 1 ? "" : "s"}
                  </p>
                </Link>
                <span className="text-sm font-medium tabular-nums">
                  {formatDuration(s.totalSecs)}
                </span>
                <Button
                  variant="outline"
                  size="sm"
                  disabled={activeIds.has(s.game.id) || start.isPending}
                  onClick={() => start.mutate(s.game.id)}
                >
                  <Play /> Start session
                </Button>
              </li>
            ))}
          </ul>
        </>
      )}
    </div>
  );
}
