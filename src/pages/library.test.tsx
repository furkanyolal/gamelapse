import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter, Route, Routes } from "react-router";
import { describe, expect, it } from "vitest";
import { api } from "@/api";
import GameDetail from "@/pages/game-detail";
import Games from "@/pages/games";
import Settings from "@/pages/settings";

function renderAt(path: string, element: React.ReactNode, route = path) {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <QueryClientProvider client={client}>
      <MemoryRouter initialEntries={[path]}>
        <Routes>
          <Route path={route} element={element} />
        </Routes>
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe("Games tag filter", () => {
  it("shows tag chips and filters by one", async () => {
    renderAt("/games", <Games />);
    expect(await screen.findByText("Elden Ring")).toBeInTheDocument();
    const filter = await screen.findByRole("group", { name: "Filter by tag" });
    expect(within(filter).getByRole("button", { name: "All" })).toBeVisible();

    await userEvent.click(
      within(filter).getByRole("button", { name: "Co-op" }),
    );
    expect(screen.queryByText("Elden Ring")).not.toBeInTheDocument();
    expect(screen.getByText("Counter-Strike 2")).toBeInTheDocument();
    expect(screen.getByText("Baldur's Gate 3")).toBeInTheDocument();

    await userEvent.click(within(filter).getByRole("button", { name: "All" }));
    expect(await screen.findByText("Elden Ring")).toBeInTheDocument();
  });
});

describe("Game detail", () => {
  it("renders the Tags and Cover sections", async () => {
    const [first] = await api.listGames(false);
    renderAt(`/games/${first.game.id}`, <GameDetail />, "/games/:id");
    expect(await screen.findByText("Tags")).toBeInTheDocument();
    const group = await screen.findByRole("group", { name: "Game tags" });
    expect(within(group).getByRole("button", { name: "RPG" })).toBeVisible();
    expect(screen.getByText("Cover")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /choose image/i })).toBeEnabled();
    expect(screen.getByRole("button", { name: /merge games/i })).toBeEnabled();
  });

  it("shows the launcher baseline note only when importedMinutes > 0", async () => {
    const [first] = await api.listGames(false);
    expect(first.importedMinutes).toBe(0);
    const plain = renderAt(
      `/games/${first.game.id}`,
      <GameDetail />,
      "/games/:id",
    );
    await screen.findByText("Tags");
    expect(
      screen.queryByText(/Includes .* reported by/),
    ).not.toBeInTheDocument();
    expect(screen.queryByText("Imported playtime")).not.toBeInTheDocument();
    plain.unmount();

    await api.importGames("steam");
    const hollow = (await api.listGames(false)).find(
      (g) => g.game.name === "Hollow Knight",
    )!;
    expect(hollow.importedMinutes).toBeGreaterThan(0);
    renderAt(`/games/${hollow.game.id}`, <GameDetail />, "/games/:id");
    expect(
      await screen.findByText(
        /Includes .* reported by Steam before Gamelapse started tracking\./,
      ),
    ).toBeInTheDocument();
  });
});

describe("Settings Data card", () => {
  it("renders export, backup and restore controls", async () => {
    renderAt("/settings", <Settings />);
    expect(await screen.findByText("Data")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Export" })).toBeEnabled();
    expect(screen.getByRole("button", { name: "Back up now" })).toBeEnabled();
    expect(
      screen.getByRole("button", { name: "Restore from backup" }),
    ).toBeEnabled();
  });
});
