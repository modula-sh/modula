import { QueryClientProvider } from "@tanstack/react-query";
import { ReactQueryDevtools } from "@tanstack/react-query-devtools";
import { RouterProvider } from "react-router-dom";
import { queryClient } from "./queries/queryClient";
import { router } from "./router";

/** App entrypoint. The whole component tree (layout + workspace state +
 * snapshot stream + selection + routes) is set up in `router.tsx` →
 * `RootLayout`. The `QueryClientProvider` sits above every route so all data
 * hooks share one cache. This file is intentionally minimal so adding new
 * state providers, route guards, or error boundaries doesn't require touching
 * the root component. */
export default function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <RouterProvider router={router} />
      {import.meta.env.DEV && <ReactQueryDevtools initialIsOpen={false} />}
    </QueryClientProvider>
  );
}
