import { useState } from "react";
import type { GameSummary, SessionWithGame } from "@/api";
import { Button } from "@/components/ui/button";
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
import { Textarea } from "@/components/ui/textarea";
import { useCreateSession, useUpdateSession } from "@/hooks/use-sessions";
import { fromDateTimeLocal, toDateTimeLocal } from "@/lib/format";

interface Props {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  games: GameSummary[];
  /** Edit this session; omit to add a new one. */
  session?: SessionWithGame | null;
  defaultGameId?: number | null;
}

function SessionForm({
  games,
  session,
  defaultGameId,
  onDone,
}: Omit<Props, "open" | "onOpenChange"> & { onDone: () => void }) {
  const create = useCreateSession();
  const update = useUpdateSession();
  const editing = !!session;

  const [gameId, setGameId] = useState<string>(() => {
    const id = defaultGameId ?? games[0]?.game.id;
    return id === undefined ? "" : String(id);
  });
  const [start, setStart] = useState(() =>
    toDateTimeLocal(session?.startedAt ?? Math.floor(Date.now() / 1000) - 3600),
  );
  const [end, setEnd] = useState(() =>
    toDateTimeLocal(session?.endedAt ?? Math.floor(Date.now() / 1000)),
  );
  const [note, setNote] = useState(session?.note ?? "");

  const startedAt = fromDateTimeLocal(start);
  const endedAt = fromDateTimeLocal(end);
  const rangeValid =
    startedAt !== null && endedAt !== null && endedAt > startedAt;
  const valid = rangeValid && (editing || gameId !== "");
  const pending = create.isPending || update.isPending;

  function submit(e: React.FormEvent) {
    e.preventDefault();
    if (!valid || startedAt === null || endedAt === null) return;
    if (session) {
      update.mutate(
        {
          sessionId: session.id,
          patch: { startedAt, endedAt, note },
        },
        { onSuccess: onDone },
      );
    } else {
      create.mutate(
        {
          gameId: Number(gameId),
          startedAt,
          endedAt,
          note: note.trim() || null,
        },
        { onSuccess: onDone },
      );
    }
  }

  return (
    <form onSubmit={submit} className="space-y-4">
      <DialogHeader>
        <DialogTitle>{editing ? "Edit session" : "Add session"}</DialogTitle>
        <DialogDescription>
          {editing
            ? `Change the times or note of this ${session.gameName} session.`
            : "Record a session that was not tracked automatically."}
        </DialogDescription>
      </DialogHeader>
      {!editing && (
        <div className="space-y-2">
          <Label htmlFor="session-game">Game</Label>
          <Select value={gameId} onValueChange={setGameId}>
            <SelectTrigger id="session-game" className="w-full">
              <SelectValue placeholder="Choose a game" />
            </SelectTrigger>
            <SelectContent>
              {games.map((g) => (
                <SelectItem key={g.game.id} value={String(g.game.id)}>
                  {g.game.name}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
      )}
      <div className="grid grid-cols-2 gap-3">
        <div className="space-y-2">
          <Label htmlFor="session-start">Start</Label>
          <Input
            id="session-start"
            type="datetime-local"
            value={start}
            onChange={(e) => setStart(e.target.value)}
          />
        </div>
        <div className="space-y-2">
          <Label htmlFor="session-end">End</Label>
          <Input
            id="session-end"
            type="datetime-local"
            value={end}
            onChange={(e) => setEnd(e.target.value)}
          />
        </div>
      </div>
      {!rangeValid && (
        <p className="text-destructive text-sm">End must be after start.</p>
      )}
      <div className="space-y-2">
        <Label htmlFor="session-note">Note</Label>
        <Textarea
          id="session-note"
          value={note}
          onChange={(e) => setNote(e.target.value)}
          rows={2}
        />
      </div>
      <DialogFooter>
        <Button type="submit" disabled={!valid || pending}>
          {editing ? "Save" : "Add session"}
        </Button>
      </DialogFooter>
    </form>
  );
}

export function SessionDialog({ open, onOpenChange, ...rest }: Props) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <SessionForm {...rest} onDone={() => onOpenChange(false)} />
      </DialogContent>
    </Dialog>
  );
}
