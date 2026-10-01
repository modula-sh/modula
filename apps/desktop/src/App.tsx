import { QueryClientProvider } from "@tanstack/react-query";
import { ReactQueryDevtools } from "@tanstack/react-query-devtools";
import { queryClient } from "./queries/queryClient";
import { RootLayout } from "./views/RootLayout";

/** App entrypoint. The whole component tree (workspace state + per-workspace
 * routers + layout) is set up in `RootLayout`. The `QueryClientProvider` sits
 * above every workspace so all data hooks share one cache. This file is
 * intentionally minimal so adding new state providers, route guards, or error
 * boundaries doesn't require touching the root component. */
export default function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <RootLayout />
      {import.meta.env.DEV && <ReactQueryDevtools initialIsOpen={false} />}
    </QueryClientProvider>
  );
}
