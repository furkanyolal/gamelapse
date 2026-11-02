import { useState } from "react";
import { ChevronLeft, ChevronRight, Pencil, Plus, Trash2 } from "lucide-react";
import type { SessionWithGame } from "@/api";
import { ConfirmDialog } from "@/components/confirm-dialog";
import { PageHeader } from "@/components/page-header";
import { EmptyState, ErrorState, RowsSkeleton } from "@/components/query-state";
import { SessionDialog } from "@/components/session-dialog";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { useGames } from "@/hooks/use-games";
import { useNow } from "@/hooks/use-now";
import { useDeleteSession, useSessions } from "@/hooks/use-sessions";
import { formatDateTime, formatDuration, startOfLocalDay } from "@/lib/format";

const PAGE_SIZE = 25;
const PRESETS = [
  { value: "today", label: "Today", offset: 0 },
  { value: "7", label: "7 days", offset: -6 },
  { value: "30", label: "30 days", offset: -29 },
  { value: "all", label: "All", offset: null },
] as const;

export default function Sessions() {
  const now = useNow(60_000);
  const [gameFilter, setGameFilter] = useState("all");
  const [preset, setPreset] = useState<string>("30");
  const [offset, setOffset] = useState(0);
  const [editing, setEditing] = useState<SessionWithGame | null>(null);
  const [adding, setAdding] = useState(false);
  const [deleting, setDeleting] = useState<SessionWithGame | null>(null);

  const games = useGames(true);
  const presetOffset = PRESETS.find((p) => p.value === preset)?.offset ?? null;
  const sessions = useSessions({
    gameId: gameFilter === "all" ? null : Number(gameFilter),
    from: presetOffset === null ? null : startOfLocalDay(now, presetOffset),
    to: null,
    limit: PAGE_SIZE,
    offset,
  });
  const del = useDeleteSession();

  const rows = sessions.data ?? [];

  return (
    <div className="space-y-6">
      <div className="flex items-start justify-between gap-4">
        <PageHeader
          title="Sessions"
          description="Every recorded play session."
        />
        <Button
          onClick={() => setAdding(true)}
          disabled={!games.data || games.data.length === 0}
        >
          <Plus /> Add session
        </Button>
      </div>

      <div className="flex flex-wrap items-center gap-4">
        <Select
          value={gameFilter}
          onValueChange={(v) => {
            setGameFilter(v);
            setOffset(0);
          }}
        >
          <SelectTrigger aria-label="Filter by game" className="w-56">
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
        <Tabs
          value={preset}
          onValueChange={(v) => {
            setPreset(v);
            setOffset(0);
          }}
        >
          <TabsList>
            {PRESETS.map((p) => (
              <TabsTrigger key={p.value} value={p.value}>
                {p.label}
              </TabsTrigger>
            ))}
          </TabsList>
        </Tabs>
      </div>

      {sessions.isPending && <RowsSkeleton rows={8} />}
      {sessions.error && (
        <ErrorState error={sessions.error} onRetry={() => sessions.refetch()} />
      )}
      {sessions.data && rows.length === 0 && offset === 0 && (
        <EmptyState title="No sessions in this range" />
      )}
      {sessions.data && (rows.length > 0 || offset > 0) && (
        <>
          <div className="rounded-lg border">
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Game</TableHead>
                  <TableHead>Start</TableHead>
                  <TableHead>End</TableHead>
                  <TableHead className="text-right">Duration</TableHead>
                  <TableHead>Source</TableHead>
                  <TableHead>Note</TableHead>
                  <TableHead className="w-24" />
                </TableRow>
              </TableHeader>
              <TableBody>
                {rows.map((s) => (
                  <TableRow key={s.id}>
                    <TableCell className="font-medium">{s.gameName}</TableCell>
                    <TableCell>{formatDateTime(s.startedAt)}</TableCell>
                    <TableCell>
                      {s.endedAt === null ? (
                        <Badge>In progress</Badge>
                      ) : (
                        formatDateTime(s.endedAt)
                      )}
                    </TableCell>
                    <TableCell className="text-right tabular-nums">
                      {formatDuration(s.durationSecs)}
                    </TableCell>
                    <TableCell className="capitalize">{s.source}</TableCell>
                    <TableCell className="text-muted-foreground max-w-48 truncate">
                      {s.note}
                    </TableCell>
                    <TableCell>
                      {s.endedAt !== null && (
                        <div className="flex justify-end gap-1">
                          <Button
                            variant="ghost"
                            size="icon-sm"
                            aria-label={`Edit session of ${s.gameName}`}
                            onClick={() => setEditing(s)}
                          >
                            <Pencil />
                          </Button>
                          <Button
                            variant="ghost"
                            size="icon-sm"
                            aria-label={`Delete session of ${s.gameName}`}
                            onClick={() => setDeleting(s)}
                          >
                            <Trash2 />
                          </Button>
                        </div>
                      )}
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </div>
          <div className="flex items-center justify-end gap-3 text-sm">
            <span className="text-muted-foreground">
              {rows.length > 0
                ? `${offset + 1}-${offset + rows.length}`
                : "No more sessions"}
            </span>
            <Button
              variant="outline"
              size="sm"
              disabled={offset === 0}
              onClick={() => setOffset(Math.max(0, offset - PAGE_SIZE))}
            >
              <ChevronLeft /> Previous
            </Button>
            <Button
              variant="outline"
              size="sm"
              disabled={rows.length < PAGE_SIZE}
              onClick={() => setOffset(offset + PAGE_SIZE)}
            >
              Next <ChevronRight />
            </Button>
          </div>
        </>
      )}

      <SessionDialog
        key={editing?.id ?? "edit"}
        open={editing !== null}
        onOpenChange={(open) => !open && setEditing(null)}
        games={games.data ?? []}
        session={editing}
      />
      <SessionDialog
        open={adding}
        onOpenChange={setAdding}
        games={games.data ?? []}
        defaultGameId={gameFilter === "all" ? null : Number(gameFilter)}
      />
      <ConfirmDialog
        open={deleting !== null}
        onOpenChange={(open) => !open && setDeleting(null)}
        title="Delete this session?"
        description={
          deleting
            ? `${deleting.gameName}, ${formatDateTime(deleting.startedAt)}, ${formatDuration(deleting.durationSecs)}. This cannot be undone.`
            : ""
        }
        onConfirm={() => {
          if (deleting) del.mutate(deleting.id);
        }}
      />
    </div>
  );
}
