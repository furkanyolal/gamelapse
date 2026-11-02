import { describe, expect, it, vi } from "vitest";
import { createMockApi } from "@/api/mock";

async function importedHollowKnight() {
  const api = createMockApi();
  await api.importGames("steam");
  const games = await api.listGames(true);
  const game = games.find((s) => s.game.sourceId === "367520")!.game;
  return { api, game };
}

describe("mock ignored imports", () => {
  it("records an entry when an imported game is deleted", async () => {
    const { api, game } = await importedHollowKnight();
    await api.deleteGame(game.id);
    const ignored = await api.listIgnoredImports();
    expect(ignored).toHaveLength(1);
    expect(ignored[0]).toMatchObject({
      source: "steam",
      sourceId: "367520",
      name: "Hollow Knight",
    });
  });

  it("does not record manual games", async () => {
    const api = createMockApi();
    const added = await api.addGameFromExe("c:\\games\\mine\\mine.exe", "Mine");
    await api.deleteGame(added.id);
    expect(await api.listIgnoredImports()).toEqual([]);
  });

  it("marks the game ignored in scans and skips it on import", async () => {
    const { api, game } = await importedHollowKnight();
    await api.deleteGame(game.id);

    const scanned = await api.scanImportSource("steam");
    expect(scanned.find((d) => d.sourceId === "367520")?.ignored).toBe(true);
    expect(scanned.find((d) => d.sourceId === "632470")?.ignored).toBe(false);

    const result = await api.importGames("steam");
    expect(result.added).toBe(0);
    expect(result.skipped).toBeGreaterThanOrEqual(1);
    const games = await api.listGames(true);
    expect(games.some((s) => s.game.sourceId === "367520")).toBe(false);
  });

  it("restore then import brings the game back", async () => {
    const { api, game } = await importedHollowKnight();
    await api.deleteGame(game.id);
    const changed = vi.fn();
    api.on("library-changed", changed);

    await api.restoreIgnoredImport("steam", "367520");
    expect(changed).toHaveBeenCalled();
    expect(await api.listIgnoredImports()).toEqual([]);

    const result = await api.importGames("steam");
    expect(result.added).toBe(1);
    const games = await api.listGames(true);
    expect(games.some((s) => s.game.sourceId === "367520")).toBe(true);
  });

  it("lists newest first", async () => {
    let t = 1_000;
    const api = createMockApi({ now: () => t });
    await api.importGames("steam");
    const games = await api.listGames(true);
    const ids = ["367520", "632470"].map(
      (sid) => games.find((s) => s.game.sourceId === sid)!.game.id,
    );
    await api.deleteGame(ids[0]);
    t = 2_000;
    await api.deleteGame(ids[1]);
    expect((await api.listIgnoredImports()).map((i) => i.sourceId)).toEqual([
      "632470",
      "367520",
    ]);
  });
});
