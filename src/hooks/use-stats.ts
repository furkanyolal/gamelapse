import { keepPreviousData, useQuery } from "@tanstack/react-query";
import { api } from "@/api";
import { keys } from "@/hooks/query-keys";

export function useDashboardStats() {
  return useQuery({
    queryKey: keys.statsDashboard,
    queryFn: () => api.getDashboardStats(),
  });
}

export function useDailyTotals(days: number, gameId: number | null = null) {
  return useQuery({
    queryKey: keys.statsDaily(days, gameId),
    queryFn: () => api.getDailyTotals(days, gameId),
    placeholderData: keepPreviousData,
  });
}

export function useGameTotals(
  from: number | null,
  to: number | null,
  limit: number,
) {
  return useQuery({
    queryKey: keys.statsGames(from, to, limit),
    queryFn: () => api.getGameTotals(from, to, limit),
    placeholderData: keepPreviousData,
  });
}

export function useHourlyTotals(days: number) {
  return useQuery({
    queryKey: keys.statsHourly(days),
    queryFn: () => api.getHourlyTotals(days),
    placeholderData: keepPreviousData,
  });
}
