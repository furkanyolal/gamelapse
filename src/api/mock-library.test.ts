import { describe, expect, it, vi } from "vitest";
import { createMockApi } from "@/api/mock";

const allSessions = {
  gameId: null,
  from: null,
  to: null,
  limit: 10_000,
  offset: 0,
  includeHidden: false,
};

describe("mock tags", () => {
  it("seeds three tags assigned to games", async () => {
    const api = createMockApi();
    expect((await api.listTags()).map((t) => t.name)).toEqual([
      "Backlog",
      "Co-op",
      "RPG",
    ]);
    const games = await api.listGames(false);
    expect(games.some((g) => g.tags.length > 0)).toBe(true);
  });

  it("creates, rejects duplicates and bad names, renames and deletes", async () => {
    const api = createMockApi();
    const events = vi.fn();
    api.on("library-changed", events);

    const tag = await api.createTag("  Roguelike ");
    expect(tag.name).toBe("Roguelike");
    await expect(api.createTag("roguelike")).rejects.toThrow(/already exists/);
    await expect(api.createTag("   ")).rejects.toThrow();
    await expect(api.createTag("x".repeat(41))).rejects.toThrow();

    const renamed = await api.renameTag(tag.id, "Rogue-like");
    expect(renamed.name).toBe("Rogue-like");
    await expect(api.renameTag(tag.id, "rpg")).rejects.toThrow();
    expect(events).toHaveBeenCalled();

    const [game] = await api.listGames(false);
    await api.setGameTags(game.game.id, [tag.id]);
    expect((await api.getGame(game.game.id)).tags).toEqual([renamed]);

    await api.deleteTag(tag.id);
    expect((await api.getGame(game.game.id)).tags).toEqual([]);
    expect((await api.listTags()).some((t) => t.id === tag.id)).toBe(false);
  });
});

describe("mock merge", () => {
  it("moves sessions and executables and removes merged games", async () => {
    const api = createMockApi();
    const games = await api.listGames(false);
    const survivor = games[0];
    const merged = games.slice(1, 3);
    const mergedIds = merged.map((g) => g.game.id);
    const expectedSessions = merged.reduce((n, g) => n + g.sessionCount, 0);

    const result = await api.mergeGames(survivor.game.id, mergedIds);
    expect(result.survivorId).toBe(survivor.game.id);
    expect(result.mergedSessions).toBe(expectedSessions);
    expect(result.mergedExecutables).toBe(2);

    expect((await api.getGame(survivor.game.id)).sessionCount).toBe(
      survivor.sessionCount + expectedSessions,
    );
    expect(await api.listGameExecutables(survivor.game.id)).toHaveLength(3);
    expect(await api.listGames(true)).toHaveLength(8);
    await expect(api.getGame(mergedIds[0])).rejects.toThrow();
    const sessions = await api.listSessions(allSessions);
    expect(sessions.every((s) => !mergedIds.includes(s.gameId))).toBe(true);
  });

  it("rejects the survivor in the merged list", async () => {
    const api = createMockApi();
    const [a, b] = await api.listGames(false);
    await expect(
      api.mergeGames(a.game.id, [b.game.id, a.game.id]),
    ).rejects.toThrow();
    expect(await api.listGames(true)).toHaveLength(10);
  });
});

describe("mock cover", () => {
  it("sets and clears the cover", async () => {
    const api = createMockApi();
    const target = (await api.listGames(false)).find(
      (g) => g.game.coverPath === null,
    )!;
    const picked = await api.pickImage();
    expect(picked).toMatch(/^https:/);
    const set = await api.setGameCover(target.game.id, picked);
    expect(set.coverPath).toBe(picked);
    expect((await api.getGame(target.game.id)).game.coverPath).toBe(picked);
    const cleared = await api.setGameCover(target.game.id, null);
    expect(cleared.coverPath).toBeNull();
  });
});

describe("mock export, backup and restore", () => {
  it("counts sessions and games according to the filters", async () => {
    const api = createMockApi();
    const all = await api.exportData({
      format: "csv",
      from: null,
      to: null,
      gameId: null,
    });
    const sessions = await api.listSessions({
      ...allSessions,
      includeHidden: true,
    });
    expect(all?.sessions).toBe(sessions.length);
    expect(all?.games).toBe(new Set(sessions.map((s) => s.gameId)).size);
    expect(all?.path).toMatch(/gamelapse-sessions-\d{8}\.csv$/);

    const gameId = sessions[0].gameId;
    const one = await api.exportData({
      format: "json",
      from: null,
      to: null,
      gameId,
    });
    expect(one?.games).toBe(1);
    expect(one?.sessions).toBe(
      sessions.filter((s) => s.gameId === gameId).length,
    );
    expect(one?.path.endsWith(".json")).toBe(true);

    const recent = await api.exportData({
      format: "csv",
      from: sessions[0].startedAt,
      to: null,
      gameId: null,
    });
    expect(recent!.sessions).toBeLessThan(all!.sessions);
  });

  it("backs up and emits both events on restore", async () => {
    const api = createMockApi({ empty: true });
    const info = await api.backupDatabase();
    expect(info?.sizeBytes).toBeGreaterThan(0);
    expect(info?.path).toMatch(/\.db$/);

    const library = vi.fn();
    const sessions = vi.fn();
    api.on("library-changed", library);
    api.on("sessions-changed", sessions);
    await api.restoreDatabase();
    expect(library).toHaveBeenCalledTimes(1);
    expect(sessions).toHaveBeenCalledTimes(1);
  });
});
