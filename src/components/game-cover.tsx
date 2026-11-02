import { Gamepad2 } from "lucide-react";
import { useSyncExternalStore } from "react";
import { coverSrc, getCoverVersion, subscribeCoverVersion } from "@/lib/assets";
import { cn } from "@/lib/utils";

/** Cover art with a neutral placeholder when the game has none. */
export function GameCover({
  coverPath,
  className,
}: {
  coverPath: string | null;
  className?: string;
}) {
  // Re-render when a cover is replaced in place (same path, new bytes).
  useSyncExternalStore(subscribeCoverVersion, getCoverVersion);
  const src = coverSrc(coverPath);
  return (
    <div
      className={cn(
        "bg-muted text-muted-foreground flex shrink-0 items-center justify-center overflow-hidden rounded-md",
        className,
      )}
    >
      {src ? (
        <img
          src={src}
          alt=""
          loading="lazy"
          className="size-full object-cover"
        />
      ) : (
        <Gamepad2 className="size-1/2" aria-hidden />
      )}
    </div>
  );
}
