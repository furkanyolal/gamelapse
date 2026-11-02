import { useEffect, useState } from "react";
import type { ImportResult, ImportSource } from "@/api";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Label } from "@/components/ui/label";
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
import {
  useImportGames,
  useImportSources,
  useScanImportSource,
} from "@/hooks/use-import";
import { formatDuration } from "@/lib/format";
import { IMPORT_SOURCES, SOURCE_LABELS } from "@/lib/import-sources";

interface Props {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** Source to select and scan as soon as the dialog opens. */
  initialSource?: ImportSource | null;
}

function ImportForm({ initialSource }: { initialSource: ImportSource | null }) {
  const sources = useImportSources();
  const [source, setSource] = useState<ImportSource | null>(initialSource);
  const [result, setResult] = useState<ImportResult | null>(null);
  const scan = useScanImportSource(source);
  const importGames = useImportGames();

  const { refetch } = scan;
  useEffect(() => {
    if (initialSource) void refetch();
  }, [initialSource, refetch]);

  const found = scan.data;
  const newCount = found?.filter((g) => !g.alreadyImported).length ?? 0;

  function pick(next: string) {
    setSource(next as ImportSource);
    setResult(null);
    importGames.reset();
  }

  function runImport() {
    if (!source) return;
    importGames.mutate(source, {
      onSuccess: (r) => {
        setResult(r);
        void refetch();
      },
    });
  }

  const error = scan.error ?? importGames.error;

  return (
    <div className="space-y-4">
      <DialogHeader>
        <DialogTitle>Import games</DialogTitle>
        <DialogDescription>
          Scan a launcher, review what it found, then add it to your library.
        </DialogDescription>
      </DialogHeader>

      <div className="flex flex-wrap items-end gap-3">
        <div className="space-y-2">
          <Label htmlFor="import-source">Launcher</Label>
          <Select value={source ?? ""} onValueChange={pick}>
            <SelectTrigger id="import-source" className="w-48">
              <SelectValue placeholder="Choose a launcher" />
            </SelectTrigger>
            <SelectContent>
              {IMPORT_SOURCES.map((s) => {
                const available = sources.data?.find(
                  (x) => x.source === s,
                )?.available;
                return (
                  <SelectItem key={s} value={s} disabled={!available}>
                    {SOURCE_LABELS[s]}
                    {available === false ? " (not found)" : ""}
                  </SelectItem>
                );
              })}
            </SelectContent>
          </Select>
        </div>
        <Button
          variant="outline"
          disabled={!source || scan.isFetching}
          onClick={() => {
            setResult(null);
            void refetch();
          }}
        >
          {scan.isFetching ? "Scanning…" : "Scan"}
        </Button>
      </div>

      {error && (
        <p role="alert" className="text-destructive text-sm">
          {error.message}
        </p>
      )}

      {found && found.length === 0 && (
        <p className="text-muted-foreground text-sm">No games found.</p>
      )}
      {found && found.length > 0 && (
        <div className="max-h-72 overflow-y-auto rounded-md border">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Name</TableHead>
                <TableHead>Install folder</TableHead>
                <TableHead className="text-right">Playtime</TableHead>
                <TableHead />
              </TableRow>
            </TableHeader>
            <TableBody>
              {found.map((g) => (
                <TableRow key={g.sourceId}>
                  <TableCell className="font-medium">{g.name}</TableCell>
                  <TableCell className="text-muted-foreground max-w-56 truncate font-mono text-xs">
                    {g.installDir ?? "-"}
                  </TableCell>
                  <TableCell className="text-right tabular-nums">
                    {g.playtimeMinutes === null
                      ? "-"
                      : formatDuration(g.playtimeMinutes * 60)}
                  </TableCell>
                  <TableCell>
                    {g.alreadyImported && (
                      <Badge variant="secondary">Already imported</Badge>
                    )}
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </div>
      )}

      {result && (
        <div className="space-y-2 text-sm" data-testid="import-result">
          <p>
            Discovered {result.discovered}, added {result.added}, updated{" "}
            {result.updated}, skipped {result.skipped}.
          </p>
          {result.errors.length > 0 && (
            <ul className="text-destructive list-disc pl-5">
              {result.errors.map((e, i) => (
                <li key={i}>{e}</li>
              ))}
            </ul>
          )}
        </div>
      )}

      <DialogFooter>
        <Button
          disabled={!source || !found?.length || importGames.isPending}
          onClick={runImport}
        >
          {importGames.isPending
            ? "Importing…"
            : newCount > 0
              ? `Import ${newCount} new game${newCount === 1 ? "" : "s"}`
              : "Import"}
        </Button>
      </DialogFooter>
    </div>
  );
}

export function ImportDialog({ open, onOpenChange, initialSource }: Props) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-2xl">
        <ImportForm initialSource={initialSource ?? null} />
      </DialogContent>
    </Dialog>
  );
}
