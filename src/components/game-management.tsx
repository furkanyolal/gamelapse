import { useState } from "react";
import { useQueries } from "@tanstack/react-query";
import { ImageOff, ImagePlus, Merge, Plus } from "lucide-react";
import { toast } from "sonner";
import { api } from "@/api";
import type { GameSummary, MergeResult } from "@/api";
import { GameCover } from "@/components/game-cover";
import { RowsSkeleton } from "@/components/query-state";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Checkbox } from "@/components/ui/checkbox";
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
import { keys } from "@/hooks/query-keys";
import {
  useChooseCover,
  useClearCover,
  useGames,
  useMergeGames,
} from "@/hooks/use-games";
import { useCreateTag, useSetGameTags, useTags } from "@/hooks/use-tags";
import { formatDuration } from "@/lib/format";

const MAX_TAG_LEN = 40;

export function TagsCard({ summary }: { summary: GameSummary }) {
  const tags = useTags();
  const setTags = useSetGameTags(summary.game.id);
  const create = useCreateTag();
  const [name, setName] = useState("");

  const current = summary.tags.map((t) => t.id);
  const trimmed = name.trim();
  const duplicate = tags.data?.some(
    (t) => t.name.toLowerCase() === trimmed.toLowerCase(),
  );
  const problem =
    trimmed.length > MAX_TAG_LEN
      ? `At most ${MAX_TAG_LEN} characters`
      : duplicate
        ? "A tag with that name already exists"
        : null;

  function toggle(id: number) {
    setTags.mutate(
      current.includes(id) ? current.filter((t) => t !== id) : [...current, id],
    );
  }

  function handleCreate() {
    if (!trimmed || problem) return;
    create.mutate(trimmed, {
      onSuccess: (tag) => {
        setName("");
        setTags.mutate([...current, tag.id]);
      },
    });
  }

  return (
    <Card>
      <CardHeader>
        <CardTitle>Tags</CardTitle>
        <CardDescription>Group games to filter the library.</CardDescription>
      </CardHeader>
      <CardContent className="space-y-4">
        {tags.data?.length === 0 && (
          <p className="text-muted-foreground text-sm">
            No tags yet. Create the first one below.
          </p>
        )}
        <div
          className="flex flex-wrap gap-2"
          role="group"
          aria-label="Game tags"
        >
          {tags.data?.map((t) => {
            const on = current.includes(t.id);
            return (
              <Button
                key={t.id}
                size="sm"
                variant={on ? "default" : "outline"}
                aria-pressed={on}
                disabled={setTags.isPending}
                onClick={() => toggle(t.id)}
              >
                {t.name}
              </Button>
            );
          })}
        </div>
        <form
          className="flex flex-wrap items-center gap-2"
          onSubmit={(e) => {
            e.preventDefault();
            handleCreate();
          }}
        >
          <Input
            aria-label="New tag name"
            placeholder="New tag"
            className="w-48"
            value={name}
            onChange={(e) => setName(e.target.value)}
          />
          <Button
            type="submit"
            variant="outline"
            disabled={!trimmed || problem !== null || create.isPending}
          >
            <Plus /> Create tag
          </Button>
        </form>
        {(problem ?? create.error?.message ?? setTags.error?.message) && (
          <p role="alert" className="text-destructive text-sm">
            {problem ?? create.error?.message ?? setTags.error?.message}
          </p>
        )}
      </CardContent>
    </Card>
  );
}

export function CoverCard({ summary }: { summary: GameSummary }) {
  const { game } = summary;
  const choose = useChooseCover(game.id);
  const clear = useClearCover(game.id);
  const error = choose.error ?? clear.error;
  return (
    <Card>
      <CardHeader>
        <CardTitle>Cover</CardTitle>
        <CardDescription>
          Shown in the library and on this page.
        </CardDescription>
      </CardHeader>
      <CardContent className="flex flex-wrap items-center gap-4">
        <GameCover coverPath={game.coverPath} className="h-24 w-48" />
        <div className="flex flex-wrap gap-2">
          <Button
            variant="outline"
            disabled={choose.isPending}
            onClick={() => choose.mutate()}
          >
            <ImagePlus /> Choose image
          </Button>
          <Button
            variant="outline"
            disabled={game.coverPath === null || clear.isPending}
            onClick={() => clear.mutate()}
          >
            <ImageOff /> Clear
          </Button>
        </div>
        {error && (
          <p role="alert" className="text-destructive w-full text-sm">
            {error.message}
          </p>
        )}
      </CardContent>
    </Card>
  );
}

export function MergeDialog({
  open,
  onOpenChange,
  summary,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  summary: GameSummary;
}) {
  const games = useGames(true);
  const merge = useMergeGames(summary.game.id);
  const [selected, setSelected] = useState<number[]>([]);
  const [confirming, setConfirming] = useState(false);
  const [result, setResult] = useState<MergeResult | null>(null);

  const others = games.data?.filter((g) => g.game.id !== summary.game.id);
  const picked = others?.filter((g) => selected.includes(g.game.id)) ?? [];
  const sessionCount = picked.reduce((n, g) => n + g.sessionCount, 0);

  const exes = useQueries({
    queries: selected.map((id) => ({
      queryKey: keys.gameExecutables(id),
      queryFn: () => api.listGameExecutables(id),
      enabled: confirming,
    })),
  });
  const exeCount = exes.reduce((n, q) => n + (q.data?.length ?? 0), 0);
  const exesLoading = exes.some((q) => q.isPending);

  function toggle(id: number) {
    setSelected((cur) =>
      cur.includes(id) ? cur.filter((x) => x !== id) : [...cur, id],
    );
  }

  function handleMerge() {
    merge.mutate(selected, {
      onSuccess: (r) => {
        setResult(r);
        toast.success(
          `Merged ${r.mergedSessions} sessions and ${r.mergedExecutables} executables`,
        );
      },
    });
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        {result ? (
          <>
            <DialogHeader>
              <DialogTitle>Games merged</DialogTitle>
              <DialogDescription>
                {summary.game.name} now has {result.mergedSessions} more session
                {result.mergedSessions === 1 ? "" : "s"} and{" "}
                {result.mergedExecutables} more executable
                {result.mergedExecutables === 1 ? "" : "s"}.
              </DialogDescription>
            </DialogHeader>
            <DialogFooter>
              <Button onClick={() => onOpenChange(false)}>Done</Button>
            </DialogFooter>
          </>
        ) : confirming ? (
          <>
            <DialogHeader>
              <DialogTitle>Merge into {summary.game.name}?</DialogTitle>
              <DialogDescription>
                The games below are deleted once their data has moved. This
                cannot be undone.
              </DialogDescription>
            </DialogHeader>
            <ul className="list-disc space-y-1 pl-5 text-sm">
              {picked.map((g) => (
                <li key={g.game.id}>{g.game.name}</li>
              ))}
            </ul>
            <p className="text-sm">
              Moves {sessionCount} session{sessionCount === 1 ? "" : "s"} (
              {formatDuration(picked.reduce((n, g) => n + g.totalSecs, 0))}) and
              up to {exesLoading ? "…" : exeCount} executable
              {exeCount === 1 ? "" : "s"}, plus their tags. Duplicate
              executables are skipped.
            </p>
            {merge.error && (
              <p role="alert" className="text-destructive text-sm">
                {merge.error.message}
              </p>
            )}
            <DialogFooter>
              <Button variant="outline" onClick={() => setConfirming(false)}>
                Back
              </Button>
              <Button
                variant="destructive"
                disabled={merge.isPending || exesLoading}
                onClick={handleMerge}
              >
                Merge {picked.length} game{picked.length === 1 ? "" : "s"}
              </Button>
            </DialogFooter>
          </>
        ) : (
          <>
            <DialogHeader>
              <DialogTitle>Merge games</DialogTitle>
              <DialogDescription>
                Choose games to merge into {summary.game.name}. Their sessions
                and executables move here.
              </DialogDescription>
            </DialogHeader>
            {games.isPending && <RowsSkeleton rows={3} />}
            {others?.length === 0 && (
              <p className="text-muted-foreground text-sm">
                There are no other games.
              </p>
            )}
            <ul className="max-h-72 space-y-1 overflow-y-auto">
              {others?.map((g) => (
                <li key={g.game.id} className="flex items-center gap-3 py-1">
                  <Checkbox
                    id={`merge-${g.game.id}`}
                    checked={selected.includes(g.game.id)}
                    onCheckedChange={() => toggle(g.game.id)}
                  />
                  <Label htmlFor={`merge-${g.game.id}`} className="flex-1">
                    {g.game.name}
                  </Label>
                  <span className="text-muted-foreground text-xs">
                    {g.sessionCount} session{g.sessionCount === 1 ? "" : "s"}
                  </span>
                </li>
              ))}
            </ul>
            <DialogFooter>
              <Button
                disabled={selected.length === 0}
                onClick={() => setConfirming(true)}
              >
                <Merge /> Continue
              </Button>
            </DialogFooter>
          </>
        )}
      </DialogContent>
    </Dialog>
  );
}
