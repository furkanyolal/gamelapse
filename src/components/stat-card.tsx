import { Card, CardContent } from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";

export function StatCard({
  label,
  value,
  loading,
}: {
  label: string;
  value: string | undefined;
  loading?: boolean;
}) {
  return (
    <Card size="sm">
      <CardContent className="space-y-1">
        <p className="text-muted-foreground text-xs">{label}</p>
        {loading || value === undefined ? (
          <Skeleton className="h-7 w-20" />
        ) : (
          <p className="text-2xl font-semibold tabular-nums">{value}</p>
        )}
      </CardContent>
    </Card>
  );
}
