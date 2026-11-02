import { openPath } from "@tauri-apps/plugin-opener";
import { PageHeader } from "@/components/page-header";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { useAppInfo } from "@/hooks/use-app-info";

function InfoRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="grid grid-cols-[8rem_1fr] gap-2 text-sm">
      <dt className="text-muted-foreground">{label}</dt>
      <dd className="font-mono break-all">{value}</dd>
    </div>
  );
}

export default function Settings() {
  const { data, error, isPending } = useAppInfo();

  return (
    <div className="space-y-6">
      <PageHeader
        title="Settings"
        description="Application information and preferences."
      />
      <Card>
        <CardHeader>
          <CardTitle>About</CardTitle>
          <CardDescription>
            Where GameTimeTracker keeps its data.
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          {isPending && (
            <p className="text-muted-foreground text-sm">Loading…</p>
          )}
          {error && (
            <p className="text-destructive text-sm">
              Failed to load app info: {error.message}
            </p>
          )}
          {data && (
            <>
              <dl className="space-y-2">
                <InfoRow label="Version" value={data.version} />
                <InfoRow label="Data folder" value={data.data_dir} />
                <InfoRow label="Database" value={data.db_path} />
                <InfoRow label="Log folder" value={data.log_dir} />
              </dl>
              <Button variant="outline" onClick={() => openPath(data.data_dir)}>
                Open data folder
              </Button>
            </>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
