import { render, screen } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { describe, expect, it } from "vitest";
import App from "@/App";

describe("App shell", () => {
  it("renders the sidebar navigation and the dashboard page", async () => {
    const client = new QueryClient();
    render(
      <QueryClientProvider client={client}>
        <App />
      </QueryClientProvider>,
    );

    const nav = screen.getByRole("list", { name: "Main navigation" });
    for (const label of [
      "Dashboard",
      "Games",
      "Sessions",
      "Stats",
      "Settings",
    ]) {
      expect(nav).toHaveTextContent(label);
    }
    expect(
      await screen.findByRole(
        "heading",
        { name: "Dashboard" },
        { timeout: 10_000 },
      ),
    ).toBeInTheDocument();
  });
});
