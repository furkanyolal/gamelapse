import { AlertCircle } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";

export function ErrorState({
  error,
  onRetry,
}: {
  error: Error;
  onRetry?: () => void;
}) {
  return (
    <div
      role="alert"
      className="border-destructive/40 flex items-center gap-3 rounded-lg border p-4 text-sm"
    >
      <AlertCircle className="text-destructive size-4 shrink-0" />
      <span className="flex-1">{error.message}</span>
      {onRetry && (
        <Button variant="outline" size="sm" onClick={onRetry}>
          Retry
        </Button>
      )}
    </div>
  );
}

export function RowsSkeleton({ rows = 4 }: { rows?: number }) {
  return (
    <div className="space-y-2" aria-busy="true" aria-label="Loading">
      {Array.from({ length: rows }, (_, i) => (
        <Skeleton key={i} className="h-10 w-full" />
      ))}
    </div>
  );
}

export function EmptyState({
  title,
  children,
}: {
  title: string;
  children?: React.ReactNode;
}) {
  return (
    <div className="text-muted-foreground flex flex-col items-center gap-3 rounded-lg border border-dashed p-8 text-center text-sm">
      <p className="text-foreground font-medium">{title}</p>
      {children}
    </div>
  );
}
