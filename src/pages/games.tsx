import { useState } from "react";
import { Link, useNavigate } from "react-router";
import { useQueryClient } from "@tanstack/react-query";
import {
  Download,
  FolderSearch,
  Image as ImageIcon,
  Play,
  Plus,
} from "lucide-react";
import { toast } from "sonner";
import { api } from "@/api";
import { GameCover } from "@/components/game-cover";
import { ImportDialog } from "@/components/import-dialog";
import { PageHeader } from "@/components/page-header";
import { ScanDialog } from "@/components/scan-dialog";
import { EmptyState, ErrorState, RowsSkeleton } from "@/components/query-state";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { useAddGameFromExe, useGames } from "@/hooks/use-games";
import { useScanRoots } from "@/hooks/use-scan";
import { useTags } from "@/hooks/use-tags";
import { useStartManualSession, useTrackerStatus } from "@/hooks/use-tracker";
import { invalidateFamilies } from "@/hooks/query-keys";
import { bumpCoverVersion } from "@/lib/assets";
import { formatDuration, formatRelative } from "@/lib/format";

/** Minimum gap between two metadata lookups. */
const COVER_LOOKUP_GAP_MS = 300;

export default function Games() {
  const [search, setSearch] = useState("");
  const [showHidden, setShowHidden] = useState(false);
  const [importOpen, setImportOpen] = useState(false);
  const [scanOpen, setScanOpen] = useState(false);
  const [finding, setFinding] = useState<{
    done: number;
    total: number;
  } | null>(null);
  const client = useQueryClient();
  const navigate = useNavigate();
  const roots = useScanRoots();
  const games = useGames(showHidden);
  const status = useTrackerStatus();
  const addGame = useAddGameFromExe();
  const start = useStartManualSession();

  const tags = useTags();
  const [selectedTags, setSelectedTags] = useState<number[]>([]);

  const activeIds = new Set(status.data?.active.map((a) => a.gameId));
  const needle = search.trim().toLowerCase();
  const visible = games.data?.filter(
    (g) =>
      g.game.name.toLowerCase().includes(needle) &&
      (selectedTags.length === 0 ||
        g.tags.some((t) => selectedTags.includes(t.id))),
  );

  function toggleTag(id: number) {
    setSelectedTags((cur) =>
      cur.includes(id) ? cur.filter((t) => t !== id) : [...cur, id],
    );
  }

  const missing = visible?.filter((s) => !s.game.coverPath) ?? [];

  async function findMissingCovers() {
    const targets = missing.map((s) => ({ id: s.game.id, name: s.game.name }));
    if (targets.length === 0) return;
    setFinding({ done: 0, total: targets.length });
    let found = 0;
    let notFound = 0;
    for (const [i, t] of targets.entries()) {
      if (i > 0) await new Promise((r) => setTimeout(r, COVER_LOOKUP_GAP_MS));
      try {
        const [best] = await api.lookupGameMetadata(t.name);
        if (best) {
          await api.applyGameMetadata(t.id, best, false);
          found += 1;
        } else {
          notFound += 1;
        }
      } catch {
        notFound += 1;
      }
      setFinding({ done: i + 1, total: targets.length });
    }
    setFinding(null);
    if (found > 0) {
      bumpCoverVersion();
      await invalidateFamilies(client, "games", "stats", "import");
    }
    const text = `Covers: ${found} found, ${notFound} not found.`;
    if (notFound > 0) toast.warning(text);
    else toast.success(text);
  }

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
          <Button
            variant="outline"
            disabled={roots.isPending}
            onClick={() => {
              if (roots.data && roots.data.length > 0) setScanOpen(true);
              else navigate("/settings#games-folders");
            }}
          >
            <FolderSearch /> Scan folders
          </Button>
          <Button
            variant="outline"
            disabled={missing.length === 0 || finding !== null}
            onClick={() => void findMissingCovers()}
          >
            <ImageIcon />{" "}
            {finding
              ? `Finding covers ${Math.min(finding.done + 1, finding.total)}/${finding.total}`
              : "Find missing covers"}
          </Button>
          <Button onClick={handleAdd} disabled={addGame.isPending}>
            <Plus /> Add game
          </Button>
        </div>
      </div>
      <ImportDialog open={importOpen} onOpenChange={setImportOpen} />
      {scanOpen && <ScanDialog open onOpenChange={setScanOpen} />}

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

      {tags.data && tags.data.length > 0 && (
        <div
          className="flex flex-wrap items-center gap-2"
          role="group"
          aria-label="Filter by tag"
        >
          <Button
            size="sm"
            variant={selectedTags.length === 0 ? "default" : "outline"}
            aria-pressed={selectedTags.length === 0}
            onClick={() => setSelectedTags([])}
          >
            All
          </Button>
          {tags.data.map((t) => {
            const on = selectedTags.includes(t.id);
            return (
              <Button
                key={t.id}
                size="sm"
                variant={on ? "default" : "outline"}
                aria-pressed={on}
                onClick={() => toggleTag(t.id)}
              >
                {t.name}
              </Button>
            );
          })}
        </div>
      )}

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
                    {s.tags.map((t) => (
                      <Badge key={t.id} variant="secondary">
                        {t.name}
                      </Badge>
                    ))}
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
