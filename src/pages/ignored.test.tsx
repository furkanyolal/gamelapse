import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter, Route, Routes } from "react-router";
import { describe, expect, it } from "vitest";
import { api } from "@/api";
import { ImportDialog } from "@/components/import-dialog";
import GameDetail from "@/pages/game-detail";

const EXTRA = "You can restore it from the Import dialog.";

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

async function gameBySourceId(sourceId: string | null) {
  const games = await api.listGames(true);
  return games.find((s) => s.game.sourceId === sourceId)!.game;
}

describe("Delete confirmation", () => {
  it("mentions restore only for imported games", async () => {
    const user = userEvent.setup();
    await api.importGames("steam");
    const imported = await gameBySourceId("367520");
    const manual = await api.addGameFromExe(
      "c:\\games\\mine\\mine.exe",
      "Mine",
    );

    const first = renderAt(`/games/${manual.id}`, <GameDetail />, "/games/:id");
    await user.click(await screen.findByRole("button", { name: /Delete/ }));
    const manualDialog = await screen.findByRole("alertdialog");
    expect(manualDialog).not.toHaveTextContent(EXTRA);
    first.unmount();

    renderAt(`/games/${imported.id}`, <GameDetail />, "/games/:id");
    await user.click(await screen.findByRole("button", { name: /Delete/ }));
    expect(await screen.findByRole("alertdialog")).toHaveTextContent(
      "It will not be re-imported. " + EXTRA,
    );
  });
});

describe("Import dialog ignored rows", () => {
  it("shows Ignored and Restore, and restoring re-imports", async () => {
    const user = userEvent.setup();
    await api.importGames("steam");
    const game = await gameBySourceId("632470");
    await api.deleteGame(game.id);

    renderAt(
      "/",
      <ImportDialog open onOpenChange={() => {}} initialSource="steam" />,
    );
    const dialog = await screen.findByRole("dialog");
    const row = (await within(dialog).findByText("Disco Elysium")).closest(
      "tr",
    )!;
    expect(within(row).getByText("Ignored")).toBeInTheDocument();

    await user.click(
      within(row).getByRole("button", { name: "Restore Disco Elysium" }),
    );
    await waitFor(() =>
      expect(within(row).getByText("Already imported")).toBeInTheDocument(),
    );
    expect(within(row).queryByText("Ignored")).not.toBeInTheDocument();
    expect((await api.listIgnoredImports()).length).toBe(0);
    expect(
      (await api.listGames(true)).some((s) => s.game.sourceId === "632470"),
    ).toBe(true);
  });
});
