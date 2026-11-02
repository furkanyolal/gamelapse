import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter } from "react-router";
import { afterEach, describe, expect, it, vi } from "vitest";
import { api } from "@/api";
import Games from "@/pages/games";

const toast = vi.hoisted(() => ({
  success: vi.fn(),
  warning: vi.fn(),
  error: vi.fn(),
}));
vi.mock("sonner", () => ({ toast }));

function renderGames() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <QueryClientProvider client={client}>
      <MemoryRouter>
        <Games />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

afterEach(() => {
  vi.restoreAllMocks();
  vi.clearAllMocks();
});

describe("Find missing covers", () => {
  it("is disabled when every game has a cover", async () => {
    const all = await api.listGames(false);
    vi.spyOn(api, "listGames").mockResolvedValue(
      all.map((s) => ({
        ...s,
        game: { ...s.game, coverPath: "C:\\covers\\x.jpg" },
      })),
    );
    renderGames();
    await screen.findByText(all[0].game.name);
    expect(
      screen.getByRole("button", { name: "Find missing covers" }),
    ).toBeDisabled();
  });

  it("looks up and applies covers without renaming, then toasts counts", async () => {
    const user = userEvent.setup();
    const all = await api.listGames(false);
    const [a, b, ...rest] = all;
    vi.spyOn(api, "listGames").mockResolvedValue([
      { ...a, game: { ...a.game, coverPath: null } },
      { ...b, game: { ...b.game, coverPath: null } },
      ...rest.map((s) => ({
        ...s,
        game: { ...s.game, coverPath: "C:\\covers\\x.jpg" },
      })),
    ]);
    const lookup = vi
      .spyOn(api, "lookupGameMetadata")
      .mockImplementation(async (name) =>
        name === a.game.name
          ? [{ steamAppId: 7, name, coverUrl: "https://img.test/a.jpg" }]
          : [],
      );
    const apply = vi.spyOn(api, "applyGameMetadata");
    renderGames();
    await screen.findByText(a.game.name);
    await user.click(
      screen.getByRole("button", { name: "Find missing covers" }),
    );
    await waitFor(() => expect(toast.warning).toHaveBeenCalled(), {
      timeout: 4000,
    });
    expect(lookup).toHaveBeenCalledTimes(2);
    expect(apply).toHaveBeenCalledTimes(1);
    expect(apply).toHaveBeenCalledWith(
      a.game.id,
      expect.objectContaining({ steamAppId: 7 }),
      false,
    );
    expect(toast.warning).toHaveBeenCalledWith(
      expect.stringMatching(/1 found, 1 not found/),
    );
  });
});
