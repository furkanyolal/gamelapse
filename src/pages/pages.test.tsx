import { render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter } from "react-router";
import { afterEach, describe, expect, it, vi } from "vitest";
import { api } from "@/api";
import Dashboard from "@/pages/dashboard";
import Games from "@/pages/games";
import Settings from "@/pages/settings";

function renderPage(page: React.ReactNode) {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <QueryClientProvider client={client}>
      <MemoryRouter>{page}</MemoryRouter>
    </QueryClientProvider>,
  );
}

afterEach(() => vi.restoreAllMocks());

describe("Dashboard", () => {
  it("shows stats, top games and the now playing strip", async () => {
    renderPage(<Dashboard />);
    expect(await screen.findByText("Now playing")).toBeInTheDocument();
    expect(await screen.findByText("All time")).toBeInTheDocument();
    expect(screen.getByText("Top games")).toBeInTheDocument();
    expect(await screen.findAllByText("Elden Ring")).not.toHaveLength(0);
    expect(screen.getByRole("button", { name: /stop/i })).toBeInTheDocument();
  });

  it("points to Add game when the library is empty", async () => {
    vi.spyOn(api, "listGames").mockResolvedValue([]);
    renderPage(<Dashboard />);
    expect(await screen.findByText("No games yet")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Add game" })).toHaveAttribute(
      "href",
      "/games",
    );
  });

  it("shows the Limits today card when limits exist", async () => {
    renderPage(<Dashboard />);
    expect(await screen.findByText("Limits today")).toBeInTheDocument();
    expect(await screen.findByText("All games")).toBeInTheDocument();
    expect(screen.getAllByRole("progressbar")).toHaveLength(2);
  });

  it("hides the Limits today card when there are none", async () => {
    const spy = vi.spyOn(api, "listLimits").mockResolvedValue([]);
    renderPage(<Dashboard />);
    expect(await screen.findByText("All time")).toBeInTheDocument();
    await waitFor(() => expect(spy).toHaveBeenCalled());
    await new Promise((r) => setTimeout(r, 20));
    expect(screen.queryByText("Limits today")).not.toBeInTheDocument();
  });

  it("shows an error state when stats fail", async () => {
    vi.spyOn(api, "getDashboardStats").mockRejectedValue(new Error("boom"));
    renderPage(<Dashboard />);
    expect(await screen.findByRole("alert")).toHaveTextContent("boom");
  });
});

describe("Games", () => {
  it("lists games with totals and filters by search", async () => {
    renderPage(<Games />);
    expect(await screen.findByText("Elden Ring")).toBeInTheDocument();
    expect(screen.getByText("Hades II")).toBeInTheDocument();
    expect(
      screen.getAllByRole("button", { name: /start session/i }).length,
    ).toBeGreaterThan(0);

    const { default: userEvent } = await import("@testing-library/user-event");
    await userEvent.type(screen.getByLabelText("Search games"), "hades");
    expect(screen.queryByText("Elden Ring")).not.toBeInTheDocument();
    expect(screen.getByText("Hades II")).toBeInTheDocument();
  });
});

describe("Games import button", () => {
  it("opens the import dialog", async () => {
    renderPage(<Games />);
    const { default: userEvent } = await import("@testing-library/user-event");
    await userEvent.click(
      await screen.findByRole("button", { name: /import/i }),
    );
    expect(await screen.findByText("Import games")).toBeInTheDocument();
  });
});

describe("Settings limits", () => {
  it("renders the Limits card with usage, notifications and reminders", async () => {
    renderPage(<Settings />);
    expect(await screen.findByText("Limits")).toBeInTheDocument();
    expect(
      await screen.findByRole("button", { name: "Edit limit for All games" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Delete limit for Factorio" }),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /add limit/i })).toBeEnabled();
    expect(screen.getByLabelText("Notifications")).toBeChecked();
    expect(screen.getByLabelText("Play reminder")).toBeInTheDocument();
  });
});

describe("Settings", () => {
  it("renders the Import and Steam Web API cards", async () => {
    renderPage(<Settings />);
    expect(await screen.findByText("Steam Web API")).toBeInTheDocument();
    expect(
      await screen.findByLabelText("Import on startup"),
    ).toBeInTheDocument();
    expect(await screen.findByText("Epic Games")).toBeInTheDocument();
    expect(await screen.findByText("Not found")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Scan Steam" })).toBeEnabled();
    expect(
      screen.getByRole("button", { name: "Scan Epic Games" }),
    ).toBeDisabled();
  });
});
