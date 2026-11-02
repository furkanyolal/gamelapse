import { MutationCache, QueryClient } from "@tanstack/react-query";
import { toast } from "sonner";

/** Query client whose failed mutations surface as toasts. */
export function createQueryClient(): QueryClient {
  return new QueryClient({
    defaultOptions: { queries: { refetchOnWindowFocus: false, retry: false } },
    mutationCache: new MutationCache({
      onError: (error) => toast.error(error.message),
    }),
  });
}
