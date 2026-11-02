import type { ImportSource } from "@/api";

export const SOURCE_LABELS: Record<ImportSource, string> = {
  steam: "Steam",
  epic: "Epic Games",
  gog: "GOG",
  heroic: "Heroic",
};

export const IMPORT_SOURCES = Object.keys(SOURCE_LABELS) as ImportSource[];
