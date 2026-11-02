import { useState } from "react";
import type { GameSummary, Limit, LimitAction, LimitScope } from "@/api";
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
import { useCreateLimit, useUpdateLimit } from "@/hooks/use-limits";
import { splitHoursMinutes } from "@/lib/format";

const MIN_SECS = 60;
const MAX_SECS = 24 * 3600;

interface Props {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  games: GameSummary[];
  /** Edit this limit; omit to add a new one. */
  limit?: Limit | null;
}

function LimitForm({
  games,
  limit,
  onDone,
}: Omit<Props, "open" | "onOpenChange"> & { onDone: () => void }) {
  const create = useCreateLimit();
  const update = useUpdateLimit();
  const editing = !!limit;
  const initial = splitHoursMinutes(limit?.dailySecs ?? 2 * 3600);

  const [scope, setScope] = useState<LimitScope>(limit?.scope ?? "global");
  const [gameId, setGameId] = useState(
    limit?.gameId === null || limit?.gameId === undefined
      ? ""
      : String(limit.gameId),
  );
  const [hours, setHours] = useState(String(initial.hours));
  const [minutes, setMinutes] = useState(String(initial.minutes));
  const [action, setAction] = useState<LimitAction>(limit?.action ?? "notify");

  const h = Number(hours);
  const m = Number(minutes);
  const inputsValid =
    hours !== "" &&
    minutes !== "" &&
    Number.isInteger(h) &&
    Number.isInteger(m) &&
    h >= 0 &&
    m >= 0 &&
    m <= 59;
  const dailySecs = h * 3600 + m * 60;
  const durationValid =
    inputsValid && dailySecs >= MIN_SECS && dailySecs <= MAX_SECS;
  const valid = durationValid && (scope === "global" || gameId !== "");
  const pending = create.isPending || update.isPending;
  const error = create.error ?? update.error;

  function submit(e: React.FormEvent) {
    e.preventDefault();
    if (!valid) return;
    const input = {
      scope,
      gameId: scope === "game" ? Number(gameId) : null,
      dailySecs,
      action,
    };
    if (limit) {
      update.mutate({ limitId: limit.id, input }, { onSuccess: onDone });
    } else {
      create.mutate(input, { onSuccess: onDone });
    }
  }

  return (
    <form onSubmit={submit} className="space-y-4">
      <DialogHeader>
        <DialogTitle>{editing ? "Edit limit" : "Add limit"}</DialogTitle>
        <DialogDescription>
          Get notified when the time played in a day passes the cap.
        </DialogDescription>
      </DialogHeader>
      <div className="space-y-2">
        <Label htmlFor="limit-scope">Applies to</Label>
        <Select value={scope} onValueChange={(v) => setScope(v as LimitScope)}>
          <SelectTrigger id="limit-scope" className="w-full">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="global">All games</SelectItem>
            <SelectItem value="game">A single game</SelectItem>
          </SelectContent>
        </Select>
      </div>
      {scope === "game" && (
        <div className="space-y-2">
          <Label htmlFor="limit-game">Game</Label>
          <Select value={gameId} onValueChange={setGameId}>
            <SelectTrigger id="limit-game" className="w-full">
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
      <div className="space-y-2">
        <Label htmlFor="limit-hours">Daily limit</Label>
        <div className="flex items-center gap-2">
          <Input
            id="limit-hours"
            type="number"
            min={0}
            max={24}
            className="w-20"
            value={hours}
            onChange={(e) => setHours(e.target.value)}
          />
          <span className="text-muted-foreground text-sm">h</span>
          <Input
            id="limit-minutes"
            aria-label="Minutes"
            type="number"
            min={0}
            max={59}
            className="w-20"
            value={minutes}
            onChange={(e) => setMinutes(e.target.value)}
          />
          <span className="text-muted-foreground text-sm">min</span>
        </div>
        {!durationValid && (
          <p className="text-destructive text-sm">
            Choose between 1 minute and 24 hours.
          </p>
        )}
      </div>
      <div className="space-y-2">
        <Label htmlFor="limit-action">When reached</Label>
        <Select
          value={action}
          onValueChange={(v) => setAction(v as LimitAction)}
        >
          <SelectTrigger id="limit-action" className="w-full">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="notify">Notify once</SelectItem>
            <SelectItem value="notify_repeat">
              Notify, then every 15 minutes
            </SelectItem>
          </SelectContent>
        </Select>
      </div>
      {error && (
        <p role="alert" className="text-destructive text-sm">
          {error.message}
        </p>
      )}
      <DialogFooter>
        <Button type="submit" disabled={!valid || pending}>
          {editing ? "Save" : "Add limit"}
        </Button>
      </DialogFooter>
    </form>
  );
}

export function LimitDialog({ open, onOpenChange, ...rest }: Props) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <LimitForm {...rest} onDone={() => onOpenChange(false)} />
      </DialogContent>
    </Dialog>
  );
}
