import { useEffect, useState } from "react";
import { toast } from "sonner";
import type { GameSummary, MetadataMatch } from "@/api";
import { GameCover } from "@/components/game-cover";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  useApplyGameMetadata,
  useLookupGameMetadata,
} from "@/hooks/use-metadata";

export function FindCoverDialog({
  open,
  onOpenChange,
  summary,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  summary: GameSummary;
}) {
  const { game } = summary;
  const [input, setInput] = useState(game.name);
  // Nothing is looked up until the user clicks Search.
  const [term, setTerm] = useState("");
  const lookup = useLookupGameMetadata(term);
  const apply = useApplyGameMetadata(game.id);

  const { refetch } = lookup;
  useEffect(() => {
    if (term) void refetch();
  }, [term, refetch]);

  function search() {
    const next = input.trim();
    if (!next) return;
    if (next === term) void refetch();
    else setTerm(next);
  }

  function use(match: MetadataMatch, renameGame: boolean) {
    apply.mutate(
      { match, renameGame },
      {
        onSuccess: () => {
          toast.success(
            renameGame ? "Cover and name updated" : "Cover updated",
          );
          onOpenChange(false);
        },
      },
    );
  }

  const error = lookup.error ?? apply.error;

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-xl">
        <DialogHeader>
          <DialogTitle>Find cover</DialogTitle>
          <DialogDescription>
            Searches the Steam store for matching games.
          </DialogDescription>
        </DialogHeader>
        <form
          className="flex items-end gap-2"
          onSubmit={(e) => {
            e.preventDefault();
            search();
          }}
        >
          <div className="flex-1 space-y-2">
            <Label htmlFor="cover-search">Game name</Label>
            <Input
              id="cover-search"
              value={input}
              onChange={(e) => setInput(e.target.value)}
            />
          </div>
          <Button type="submit" disabled={!input.trim() || lookup.isFetching}>
            {lookup.isFetching ? "Searching…" : "Search"}
          </Button>
        </form>

        {error && (
          <p role="alert" className="text-destructive text-sm">
            {error.message}
          </p>
        )}
        {lookup.data && lookup.data.length === 0 && (
          <p className="text-muted-foreground text-sm">No matches.</p>
        )}
        {lookup.data && lookup.data.length > 0 && (
          <ul className="divide-y rounded-md border">
            {lookup.data.map((m) => (
              <li key={m.steamAppId} className="flex items-center gap-3 p-3">
                <GameCover coverPath={m.coverUrl} className="h-12 w-24" />
                <span className="min-w-0 flex-1 truncate text-sm font-medium">
                  {m.name}
                </span>
                <Button
                  size="sm"
                  variant="outline"
                  disabled={apply.isPending}
                  onClick={() => use(m, false)}
                >
                  Use cover
                </Button>
                <Button
                  size="sm"
                  disabled={apply.isPending}
                  onClick={() => use(m, true)}
                >
                  Use cover and name
                </Button>
              </li>
            ))}
          </ul>
        )}
      </DialogContent>
    </Dialog>
  );
}
