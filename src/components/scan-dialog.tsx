import { useEffect, useMemo, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { X } from "lucide-react";
import { toast } from "sonner";
import { api } from "@/api";
import type { MetadataMatch, ScanCandidate, ScanSelection } from "@/api";
import { GameCover } from "@/components/game-cover";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
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
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Skeleton } from "@/components/ui/skeleton";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { keys } from "@/hooks/query-keys";
import { useAddScannedGames, useScanFolders } from "@/hooks/use-scan";

/** Minimum gap between two metadata lookups. */
const LOOKUP_GAP_MS = 300;

interface RowEdit {
  checked?: boolean;
  name?: string;
  exePath?: string;
  coverUrl?: string | null;
}

type MatchState =
  | { status: "pending" }
  | { status: "loading" }
  | { status: "done"; match: MetadataMatch | null }
  | { status: "error"; message: string };

const fileName = (path: string) => path.split(/[\\/]/).pop() ?? path;

const defaultChecked = (c: ScanCandidate) =>
  c.confidence === "high" && c.existingGameId === null;

/** Looks candidates up one at a time while mounted; stops on unmount. */
function useSequentialLookups(candidates: ScanCandidate[] | undefined) {
  const client = useQueryClient();
  const [matches, setMatches] = useState<Record<string, MatchState>>({});

  useEffect(() => {
    if (!candidates) return;
    let cancelled = false;
    const queue = candidates.filter((c) => c.existingGameId === null);
    void (async () => {
      for (const [i, c] of queue.entries()) {
        if (cancelled) return;
        if (i > 0) await new Promise((r) => setTimeout(r, LOOKUP_GAP_MS));
        if (cancelled) return;
        setMatches((m) => ({ ...m, [c.folder]: { status: "loading" } }));
        let next: MatchState;
        try {
          const found = await client.fetchQuery({
            queryKey: keys.metadata(c.suggestedName),
            queryFn: () => api.lookupGameMetadata(c.suggestedName),
            staleTime: 5 * 60_000,
          });
          next = { status: "done", match: found[0] ?? null };
        } catch (e) {
          next = {
            status: "error",
            message: e instanceof Error ? e.message : String(e),
          };
        }
        if (cancelled) return;
        setMatches((m) => ({ ...m, [c.folder]: next }));
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [candidates, client]);

  return matches;
}

function MatchCell({
  state,
  onUse,
}: {
  state: MatchState | undefined;
  onUse: (m: MetadataMatch) => void;
}) {
  if (!state) return <span className="text-muted-foreground text-xs">-</span>;
  if (state.status === "pending" || state.status === "loading") {
    return (
      <span className="text-muted-foreground text-xs">
        {state.status === "loading" ? "Looking up…" : "Waiting"}
      </span>
    );
  }
  if (state.status === "error") {
    return (
      <span className="text-destructive text-xs" title={state.message}>
        Lookup failed
      </span>
    );
  }
  if (!state.match) {
    return <span className="text-muted-foreground text-xs">No match</span>;
  }
  const { match } = state;
  return (
    <div className="flex items-center gap-2">
      <GameCover coverPath={match.coverUrl} className="h-8 w-16" />
      <span className="max-w-32 truncate text-xs">{match.name}</span>
      <Button
        size="sm"
        variant="outline"
        aria-label={`Use ${match.name}`}
        onClick={() => onUse(match)}
      >
        Use
      </Button>
    </div>
  );
}

function ScanForm({
  rootIds,
  onDone,
}: {
  rootIds: number[] | null;
  onDone: () => void;
}) {
  const scan = useScanFolders();
  const addGames = useAddScannedGames();
  const [edits, setEdits] = useState<Record<string, RowEdit>>({});

  const { mutate } = scan;
  useEffect(() => {
    mutate(rootIds);
  }, [mutate, rootIds]);

  const candidates = scan.data;
  const matches = useSequentialLookups(candidates);

  const rows = useMemo(
    () =>
      (candidates ?? []).map((c) => {
        const e = edits[c.folder] ?? {};
        const m = matches[c.folder];
        return {
          c,
          checked: e.checked ?? defaultChecked(c),
          name: e.name ?? c.suggestedName,
          exePath: e.exePath ?? c.executables[0]?.path ?? "",
          // undefined means untouched: fall back to the best match's cover.
          coverUrl:
            e.coverUrl !== undefined
              ? e.coverUrl
              : m?.status === "done"
                ? (m.match?.coverUrl ?? null)
                : null,
        };
      }),
    [candidates, edits, matches],
  );

  function edit(folder: string, patch: RowEdit) {
    setEdits((cur) => ({ ...cur, [folder]: { ...cur[folder], ...patch } }));
  }

  const selected = rows.filter((r) => r.checked && r.name.trim() && r.exePath);

  function submit() {
    const selection: ScanSelection[] = selected.map((r) => ({
      folder: r.c.folder,
      name: r.name.trim(),
      exePath: r.exePath,
      coverUrl: r.coverUrl,
    }));
    addGames.mutate(selection, {
      onSuccess: (r) => {
        const text = `Added ${r.added}, skipped ${r.skipped}${
          r.errors.length ? `, ${r.errors.length} error(s)` : ""
        }.`;
        if (r.errors.length) toast.warning(text);
        else toast.success(text);
        onDone();
      },
    });
  }

  const error = scan.error ?? addGames.error;

  return (
    <div className="space-y-4">
      <DialogHeader>
        <DialogTitle>Scan folders</DialogTitle>
        <DialogDescription>
          Review what was found. Nothing is added until you click Add selected.
        </DialogDescription>
      </DialogHeader>

      {scan.isPending && (
        <div className="space-y-2" role="status" aria-label="Scanning">
          <p className="text-muted-foreground text-sm">Scanning folders…</p>
          <Skeleton className="h-8 w-full" />
          <Skeleton className="h-8 w-full" />
          <Skeleton className="h-8 w-full" />
        </div>
      )}
      {error && (
        <p role="alert" className="text-destructive text-sm">
          {error.message}
        </p>
      )}
      {candidates && candidates.length === 0 && (
        <p className="text-muted-foreground text-sm">No games found.</p>
      )}
      {rows.length > 0 && (
        <TooltipProvider>
          <div className="max-h-96 overflow-auto rounded-md border">
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead className="w-8" />
                  <TableHead>Name</TableHead>
                  <TableHead>Executable</TableHead>
                  <TableHead>Match</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {rows.map((r) => {
                  const exe = r.c.executables.find((x) => x.path === r.exePath);
                  return (
                    <TableRow key={r.c.folder}>
                      <TableCell>
                        <Checkbox
                          aria-label={`Add ${r.c.suggestedName}`}
                          checked={r.checked}
                          onCheckedChange={(v) =>
                            edit(r.c.folder, { checked: v === true })
                          }
                        />
                      </TableCell>
                      <TableCell className="space-y-1">
                        <div className="flex items-center gap-2">
                          {r.coverUrl && (
                            <div className="relative shrink-0">
                              <GameCover
                                coverPath={r.coverUrl}
                                className="h-8 w-16"
                              />
                              <button
                                type="button"
                                aria-label={`Clear cover for ${r.c.suggestedName}`}
                                title="Remove cover"
                                className="bg-background/90 hover:bg-background absolute -top-1 -right-1 flex size-4 items-center justify-center rounded-full border"
                                onClick={() =>
                                  edit(r.c.folder, { coverUrl: null })
                                }
                              >
                                <X className="size-3" aria-hidden />
                              </button>
                            </div>
                          )}
                          <Input
                            aria-label={`Name for ${r.c.suggestedName}`}
                            value={r.name}
                            onChange={(e) =>
                              edit(r.c.folder, { name: e.target.value })
                            }
                          />
                        </div>
                        <div className="flex items-center gap-2">
                          <span
                            className="text-muted-foreground max-w-48 truncate font-mono text-xs"
                            title={r.c.folder}
                          >
                            {r.c.folder}
                          </span>
                          {r.c.existingGameId !== null && (
                            <Badge variant="secondary">In library</Badge>
                          )}
                        </div>
                      </TableCell>
                      <TableCell>
                        <Tooltip>
                          <TooltipTrigger asChild>
                            <div>
                              <Select
                                value={r.exePath}
                                onValueChange={(v) =>
                                  edit(r.c.folder, { exePath: v })
                                }
                              >
                                <SelectTrigger
                                  aria-label={`Executable for ${r.c.suggestedName}`}
                                  className="w-48"
                                >
                                  <SelectValue />
                                </SelectTrigger>
                                <SelectContent>
                                  {r.c.executables.map((x) => (
                                    <SelectItem key={x.path} value={x.path}>
                                      {fileName(x.path)}
                                    </SelectItem>
                                  ))}
                                </SelectContent>
                              </Select>
                            </div>
                          </TooltipTrigger>
                          {exe && (
                            <TooltipContent className="max-w-72">
                              <p className="font-mono break-all">{exe.path}</p>
                              <p>Score {exe.score}</p>
                              {exe.reasons.length > 0 && (
                                <p>{exe.reasons.join(", ")}</p>
                              )}
                            </TooltipContent>
                          )}
                        </Tooltip>
                      </TableCell>
                      <TableCell>
                        <MatchCell
                          state={matches[r.c.folder]}
                          onUse={(m) =>
                            edit(r.c.folder, {
                              name: m.name,
                              coverUrl: m.coverUrl,
                            })
                          }
                        />
                      </TableCell>
                    </TableRow>
                  );
                })}
              </TableBody>
            </Table>
          </div>
        </TooltipProvider>
      )}

      <DialogFooter>
        <Button
          disabled={selected.length === 0 || addGames.isPending}
          onClick={submit}
        >
          {addGames.isPending ? "Adding…" : `Add selected (${selected.length})`}
        </Button>
      </DialogFooter>
    </div>
  );
}

interface Props {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** Roots to scan; null scans all of them. */
  rootIds?: number[] | null;
}

export function ScanDialog({ open, onOpenChange, rootIds = null }: Props) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-4xl">
        <ScanForm rootIds={rootIds} onDone={() => onOpenChange(false)} />
      </DialogContent>
    </Dialog>
  );
}
