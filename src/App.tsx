import { BrowserRouter, Route, Routes } from "react-router";
import { AppShell } from "@/components/app-shell";
import { TooltipProvider } from "@/components/ui/tooltip";
import Dashboard from "@/pages/dashboard";
import GameDetail from "@/pages/game-detail";
import Games from "@/pages/games";
import Sessions from "@/pages/sessions";
import Settings from "@/pages/settings";
import Stats from "@/pages/stats";

export default function App() {
  return (
    <TooltipProvider>
      <BrowserRouter>
        <Routes>
          <Route element={<AppShell />}>
            <Route index element={<Dashboard />} />
            <Route path="games" element={<Games />} />
            <Route path="games/:id" element={<GameDetail />} />
            <Route path="sessions" element={<Sessions />} />
            <Route path="stats" element={<Stats />} />
            <Route path="settings" element={<Settings />} />
          </Route>
        </Routes>
      </BrowserRouter>
    </TooltipProvider>
  );
}
