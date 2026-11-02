import { describe, expect, it, vi } from "vitest";
import { createMockApi } from "@/api/mock";

describe("mock folder scan", () => {
  it("adds and removes roots, rejecting duplicates and overlaps", async () => {
    const api = createMockApi();
    expect((await api.listScanRoots()).map((r) => r.path)).toEqual([
      "D:\\Games",
    ]);
    const picked = await api.pickFolder();
    expect(picked).toBeTruthy();
    const root = await api.addScanRoot(picked!);
    expect(await api.listScanRoots()).toHaveLength(2);
    await expect(api.addScanRoot(picked!)).rejects.toThrow(/already/);
    await expect(api.addScanRoot("D:\\Games\\Sub")).rejects.toThrow(/overlaps/);
    await api.removeScanRoot(root.id);
    expect(await api.listScanRoots()).toHaveLength(1);
  });

  it("scans to candidates with ranked executables", async () => {
    const api = createMockApi();
    const found = await api.scanFolders(null);
    expect(found).toHaveLength(4);
    expect(found.some((c) => c.existingGameId !== null)).toBe(true);
    expect(new Set(found.map((c) => c.confidence)).size).toBeGreaterThan(1);
    for (const c of found) {
      expect(c.executables.length).toBeGreaterThanOrEqual(2);
      expect(c.executables.length).toBeLessThanOrEqual(4);
      const scores = c.executables.map((e) => e.score);
      expect(scores).toEqual([...scores].sort((a, b) => b - a));
    }
    await api.removeScanRoot(1);
    expect(await api.scanFolders(null)).toEqual([]);
  });

  it("adds selected games with rules and a cover", async () => {
    const api = createMockApi({ empty: true });
    const onChange = vi.fn();
    api.on("library-changed", onChange);
    const [first] = await api.scanFolders(null);
    const result = await api.addScannedGames([
      {
        folder: first.folder,
        name: "Elden Ring",
        exePath: first.executables[0].path,
        coverUrl: "https://example.com/c.jpg",
      },
    ]);
    expect(result).toMatchObject({ added: 1, skipped: 0 });
    expect(onChange).toHaveBeenCalled();
    const [game] = await api.listGames(true);
    expect(game.game.source).toBe("manual");
    expect(game.game.coverPath).toBe("https://example.com/c.jpg");
    const rules = await api.listGameExecutables(game.game.id);
    expect(rules.map((r) => r.matchMode).sort()).toEqual([
      "exact_path",
      "install_dir_prefix",
    ]);
  });

  it("looks up metadata and fails for names with fail", async () => {
    const api = createMockApi();
    const matches = await api.lookupGameMetadata("Hades");
    expect(matches).toHaveLength(3);
    expect(matches.every((m) => m.coverUrl.startsWith("https://"))).toBe(true);
    await expect(api.lookupGameMetadata("will fail")).rejects.toThrow();
  });

  it("applies a match to a game", async () => {
    const api = createMockApi();
    const [{ game }] = await api.listGames(false);
    const [match] = await api.lookupGameMetadata("Something New");
    const cover = await api.applyGameMetadata(game.id, match, false);
    expect(cover.coverPath).toBe(match.coverUrl);
    expect(cover.name).toBe(game.name);
    const renamed = await api.applyGameMetadata(game.id, match, true);
    expect(renamed.name).toBe("Something New");
  });

  it("offers Heroic as an import source", async () => {
    const api = createMockApi({ empty: true });
    const heroic = (await api.listImportSources()).find(
      (s) => s.source === "heroic",
    );
    expect(heroic?.available).toBe(true);
    const found = await api.scanImportSource("heroic");
    expect(found).toHaveLength(2);
    await api.importGames("heroic");
    const sources = (await api.listGames(true)).map((g) => g.game.source);
    expect(sources.sort()).toEqual(["epic", "gog"]);
  });
});
