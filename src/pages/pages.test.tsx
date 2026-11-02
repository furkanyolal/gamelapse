import { render, screen } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter } from "react-router";
import { afterEach, describe, expect, it, vi } from "vitest";
import { api } from "@/api";
import Dashboard from "@/pages/dashboard";
import Games from "@/pages/games";

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
