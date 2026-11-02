import { lazy } from "react";
import { BrowserRouter, Route, Routes } from "react-router";
import { AppShell } from "@/components/app-shell";
import { TooltipProvider } from "@/components/ui/tooltip";

const Dashboard = lazy(() => import("@/pages/dashboard"));
const GameDetail = lazy(() => import("@/pages/game-detail"));
const Games = lazy(() => import("@/pages/games"));
const Sessions = lazy(() => import("@/pages/sessions"));
const Settings = lazy(() => import("@/pages/settings"));
const Stats = lazy(() => import("@/pages/stats"));

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
