import {
  BarChart3,
  Clock,
  Gamepad2,
  LayoutDashboard,
  Settings,
} from "lucide-react";
import { Suspense } from "react";
import { Link, NavLink, Outlet, useLocation } from "react-router";
import { Elapsed } from "@/components/now-playing";
import { RowsSkeleton } from "@/components/query-state";
import { ThemeSync } from "@/components/theme-sync";
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarGroupContent,
  SidebarHeader,
  SidebarInset,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarProvider,
} from "@/components/ui/sidebar";
import { useApiEvents } from "@/hooks/use-api-events";
import { useTrackerStatus } from "@/hooks/use-tracker";

const NAV_ITEMS = [
  { to: "/", label: "Dashboard", icon: LayoutDashboard },
  { to: "/games", label: "Games", icon: Gamepad2 },
  { to: "/sessions", label: "Sessions", icon: Clock },
  { to: "/stats", label: "Stats", icon: BarChart3 },
  { to: "/settings", label: "Settings", icon: Settings },
];

export function AppShell() {
  useApiEvents();
  const { data: status } = useTrackerStatus();
  const playing = status?.active[0];
  const { pathname } = useLocation();
  const isActive = (to: string) =>
    to === "/" ? pathname === "/" : pathname.startsWith(to);

  return (
    <SidebarProvider>
      <ThemeSync />
      <Sidebar collapsible="none" className="border-r">
        <SidebarHeader className="px-4 py-4 text-base font-semibold">
          Gamelapse
        </SidebarHeader>
        <SidebarContent>
          <SidebarGroup>
            <SidebarGroupContent>
              <SidebarMenu aria-label="Main navigation">
                {NAV_ITEMS.map(({ to, label, icon: Icon }) => (
                  <SidebarMenuItem key={to}>
                    <SidebarMenuButton asChild isActive={isActive(to)}>
                      <NavLink to={to}>
                        <Icon />
                        <span>{label}</span>
                      </NavLink>
                    </SidebarMenuButton>
                  </SidebarMenuItem>
                ))}
              </SidebarMenu>
            </SidebarGroupContent>
          </SidebarGroup>
        </SidebarContent>
        <SidebarFooter>
          {playing && (
            <Link
              to={`/games/${playing.gameId}`}
              className="bg-sidebar-accent flex items-center gap-2 rounded-md px-3 py-2 text-xs"
              aria-label="Now playing"
            >
              <span
                className={`size-2 shrink-0 rounded-full ${playing.isAfk ? "bg-amber-500" : "bg-emerald-500"}`}
              />
              <span className="min-w-0 flex-1 truncate">
                {playing.gameName}
              </span>
              {playing.isAfk && (
                <span className="text-muted-foreground font-medium">AFK</span>
              )}
              <Elapsed session={playing} />
            </Link>
          )}
        </SidebarFooter>
      </Sidebar>
      <SidebarInset>
        <main className="p-6">
          <Suspense fallback={<RowsSkeleton rows={6} />}>
            <Outlet />
          </Suspense>
        </main>
      </SidebarInset>
    </SidebarProvider>
  );
}
