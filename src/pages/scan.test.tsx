import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter, Route, Routes } from "react-router";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ScanDialog } from "@/components/scan-dialog";
import { api } from "@/api";
import GameDetail from "@/pages/game-detail";
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

afterEach(() => vi.restoreAllMocks());

describe("Scan dialog", () => {
  it("lists candidates and checks only high confidence new ones", async () => {
    renderAt("/", <ScanDialog open onOpenChange={() => {}} />);
    const dialog = await screen.findByRole("dialog");
    expect(
      await within(dialog).findByLabelText("Name for Hollow Knight"),
    ).toHaveValue("Hollow Knight");
    expect(within(dialog).getByLabelText("Add Hollow Knight")).toBeChecked();
    expect(
      within(dialog).getByLabelText("Add Mystery Project"),
    ).not.toBeChecked();
    expect(within(dialog).getByLabelText("Add Portal 2")).not.toBeChecked();
    expect(within(dialog).getByLabelText("Add Elden Ring")).not.toBeChecked();
    expect(within(dialog).getByText("In library")).toBeInTheDocument();
    expect(
      within(dialog).getByRole("button", { name: /Add selected \(1\)/ }),
    ).toBeEnabled();
  });

  it("fills the name from a match", async () => {
    const user = userEvent.setup();
    renderAt("/", <ScanDialog open onOpenChange={() => {}} />);
    const input = await screen.findByLabelText("Name for Hollow Knight");
    await user.clear(input);
    const use = await screen.findByRole(
      "button",
      { name: "Use Hollow Knight" },
      { timeout: 4000 },
    );
    await user.click(use);
    expect(input).toHaveValue("Hollow Knight");
  });

  it("pre-fills the cover from the best match, clears it, and sends it", async () => {
    const user = userEvent.setup();
    vi.spyOn(api, "lookupGameMetadata").mockImplementation(async (name) => [
      { steamAppId: 1, name, coverUrl: `https://img.test/${name}.jpg` },
    ]);
    const add = vi.spyOn(api, "addScannedGames").mockResolvedValue({
      source: "steam",
      discovered: 1,
      added: 1,
      updated: 0,
      skipped: 0,
      errors: [],
    });
    renderAt("/", <ScanDialog open onOpenChange={() => {}} />);
    const clear = await screen.findByRole(
      "button",
      { name: "Clear cover for Hollow Knight" },
      { timeout: 4000 },
    );
    await user.click(screen.getByRole("button", { name: /Add selected/ }));
    expect(add).toHaveBeenLastCalledWith([
      expect.objectContaining({
        name: "Hollow Knight",
        coverUrl: "https://img.test/Hollow Knight.jpg",
      }),
    ]);

    await user.click(clear);
    expect(
      screen.queryByRole("button", { name: "Clear cover for Hollow Knight" }),
    ).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: /Add selected/ }));
    expect(add).toHaveBeenLastCalledWith([
      expect.objectContaining({ name: "Hollow Knight", coverUrl: null }),
    ]);
  });
});
describe("Games folders card", () => {
  it("shows the roots and the promise", async () => {
    renderAt("/settings", <Settings />);
    expect(await screen.findByText("Games folders")).toBeInTheDocument();
    expect(await screen.findByText("D:\\Games")).toBeInTheDocument();
    expect(
      screen.getByText(/scans only when you click Scan and adds only what you/),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Add folder" }),
    ).toBeInTheDocument();
    expect(await screen.findByText("Heroic")).toBeInTheDocument();
  });
});

describe("Find cover dialog", () => {
  it("searches and shows matches", async () => {
    const user = userEvent.setup();
    renderAt("/games/1", <GameDetail />, "/games/:id");
    await user.click(await screen.findByRole("button", { name: "Find cover" }));
    const dialog = await screen.findByRole("dialog");
    expect(within(dialog).getByLabelText("Game name")).toHaveValue(
      "Elden Ring",
    );
    expect(within(dialog).queryByText("Use cover")).not.toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Search" }));
    expect(
      await within(dialog).findAllByRole("button", { name: "Use cover" }),
    ).toHaveLength(3);
    expect(
      within(dialog).getAllByRole("button", { name: /Use cover and name/ }),
    ).toHaveLength(3);
  });
});
