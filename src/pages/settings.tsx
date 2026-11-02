import { useState } from "react";
import { openPath } from "@tauri-apps/plugin-opener";
import { toast } from "sonner";
import type { Settings as AppSettings, Theme } from "@/api";
import { PageHeader } from "@/components/page-header";
import { ErrorState, RowsSkeleton } from "@/components/query-state";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { useAppInfo } from "@/hooks/use-app-info";
import {
  usePing,
  useSeedDemoData,
  useSettings,
  useUpdateSettings,
} from "@/hooks/use-settings";
import { useSetTrackingEnabled } from "@/hooks/use-tracker";

function InfoRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="grid grid-cols-[8rem_1fr] gap-2 text-sm">
      <dt className="text-muted-foreground">{label}</dt>
      <dd className="font-mono break-all">{value}</dd>
    </div>
  );
}

function SwitchRow({
  id,
  label,
  description,
  checked,
  onCheckedChange,
}: {
  id: string;
  label: string;
  description?: string;
  checked: boolean;
  onCheckedChange: (checked: boolean) => void;
}) {
  return (
    <div className="flex items-center justify-between gap-4">
      <div className="space-y-0.5">
        <Label htmlFor={id}>{label}</Label>
        {description && (
          <p className="text-muted-foreground text-xs">{description}</p>
        )}
      </div>
      <Switch id={id} checked={checked} onCheckedChange={onCheckedChange} />
    </div>
  );
}

function NumberField({
  id,
  label,
  suffix,
  value,
  min,
  max,
  onChange,
}: {
  id: string;
  label: string;
  suffix: string;
  value: string;
  min: number;
  max?: number;
  onChange: (value: string) => void;
}) {
  return (
    <div className="space-y-2">
      <Label htmlFor={id}>{label}</Label>
      <div className="flex items-center gap-2">
        <Input
          id={id}
          type="number"
          min={min}
          max={max}
          className="w-28"
          value={value}
          onChange={(e) => onChange(e.target.value)}
        />
        <span className="text-muted-foreground text-sm">{suffix}</span>
      </div>
    </div>
  );
}

function TrackingCard({ settings }: { settings: AppSettings }) {
  const update = useUpdateSettings();
  const setTracking = useSetTrackingEnabled();
  const [poll, setPoll] = useState(String(settings.pollIntervalSecs));
  const [afk, setAfk] = useState(String(settings.afkThresholdSecs / 60));
  const [gap, setGap] = useState(String(settings.sessionMergeGapSecs / 60));

  const pollN = Number(poll);
  const afkN = Number(afk);
  const gapN = Number(gap);
  const valid =
    Number.isInteger(pollN) &&
    pollN >= 1 &&
    pollN <= 60 &&
    afk !== "" &&
    afkN >= 1 &&
    gap !== "" &&
    gapN >= 0;
  const dirty =
    pollN !== settings.pollIntervalSecs ||
    Math.round(afkN * 60) !== settings.afkThresholdSecs ||
    Math.round(gapN * 60) !== settings.sessionMergeGapSecs;

  return (
    <Card>
      <CardHeader>
        <CardTitle>Tracking</CardTitle>
        <CardDescription>
          How games are detected. The tracker reads changes on its next poll.
        </CardDescription>
      </CardHeader>
      <CardContent className="space-y-5">
        <SwitchRow
          id="tracking-enabled"
          label="Tracking enabled"
          description="Turning this off ends automatic sessions. Manual sessions keep running."
          checked={settings.trackingEnabled}
          onCheckedChange={(enabled) => setTracking.mutate(enabled)}
        />
        <SwitchRow
          id="require-foreground"
          label="Require foreground"
          description="Only count a game while its window is in front."
          checked={settings.requireForeground}
          onCheckedChange={(requireForeground) =>
            update.mutate({ requireForeground })
          }
        />
        <div className="flex items-center justify-between gap-4">
          <Label htmlFor="theme">Theme</Label>
          <Select
            value={settings.theme}
            onValueChange={(theme) => update.mutate({ theme: theme as Theme })}
          >
            <SelectTrigger id="theme" className="w-36">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="dark">Dark</SelectItem>
              <SelectItem value="light">Light</SelectItem>
              <SelectItem value="system">System</SelectItem>
            </SelectContent>
          </Select>
        </div>
        <div className="flex flex-wrap gap-6">
          <NumberField
            id="poll-interval"
            label="Poll interval"
            suffix="seconds (1-60)"
            value={poll}
            min={1}
            max={60}
            onChange={setPoll}
          />
          <NumberField
            id="afk-threshold"
            label="AFK threshold"
            suffix="minutes"
            value={afk}
            min={1}
            onChange={setAfk}
          />
          <NumberField
            id="merge-gap"
            label="Merge gap"
            suffix="minutes"
            value={gap}
            min={0}
            onChange={setGap}
          />
        </div>
        <Button
          disabled={!valid || !dirty || update.isPending}
          onClick={() =>
            update.mutate(
              {
                pollIntervalSecs: pollN,
                afkThresholdSecs: Math.round(afkN * 60),
                sessionMergeGapSecs: Math.round(gapN * 60),
              },
              { onSuccess: () => toast.success("Settings saved") },
            )
          }
        >
          Save
        </Button>
      </CardContent>
    </Card>
  );
}

function DeveloperCard() {
  const seed = useSeedDemoData();
  const ping = usePing();
  return (
    <Card>
      <CardHeader>
        <CardTitle>Developer</CardTitle>
        <CardDescription>Tools for testing the app.</CardDescription>
      </CardHeader>
      <CardContent className="space-y-3">
        <div className="flex flex-wrap items-center gap-3">
          <Button
            variant="outline"
            disabled={seed.isPending}
            onClick={() => seed.mutate()}
          >
            Seed demo data
          </Button>
          {seed.isSuccess && (
            <span className="text-muted-foreground text-sm">
              Demo data is in place.
            </span>
          )}
        </div>
        <div className="flex flex-wrap items-center gap-3">
          <Button
            variant="outline"
            disabled={ping.isPending}
            onClick={() => ping.mutate()}
          >
            Ping
          </Button>
          {ping.isSuccess && (
            <span className="font-mono text-sm">{ping.data}</span>
          )}
        </div>
      </CardContent>
    </Card>
  );
}

export default function Settings() {
  const info = useAppInfo();
  const settings = useSettings();

  return (
    <div className="space-y-6">
      <PageHeader
        title="Settings"
        description="Application information and preferences."
      />

      {settings.isPending && <RowsSkeleton rows={4} />}
      {settings.error && (
        <ErrorState error={settings.error} onRetry={() => settings.refetch()} />
      )}
      {settings.data && <TrackingCard settings={settings.data} />}

      <Card>
        <CardHeader>
          <CardTitle>About</CardTitle>
          <CardDescription>
            Where GameTimeTracker keeps its data.
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          {info.isPending && <RowsSkeleton rows={2} />}
          {info.error && (
            <ErrorState error={info.error} onRetry={() => info.refetch()} />
          )}
          {info.data && (
            <>
              <dl className="space-y-2">
                <InfoRow label="Version" value={info.data.version} />
                <InfoRow label="Data folder" value={info.data.dataDir} />
                <InfoRow label="Database" value={info.data.dbPath} />
                <InfoRow label="Log folder" value={info.data.logDir} />
              </dl>
              <Button
                variant="outline"
                onClick={() =>
                  openPath(info.data.dataDir).catch((e) =>
                    toast.error(String(e)),
                  )
                }
              >
                Open data folder
              </Button>
            </>
          )}
        </CardContent>
      </Card>

      <DeveloperCard />
    </div>
  );
}
