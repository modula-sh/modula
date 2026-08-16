import { useContext } from "react";
import { WorkspaceContext } from "../contexts/WorkspaceContext";
import Overview from "../Overview";

/** Tiny route wrapper around the imperative three.js Overview component.
 * Pulls `workspace` from context so the route table doesn't have to plumb
 * it as a prop. */
export function OverviewRoute() {
  const workspace = useContext(WorkspaceContext);
  return <Overview workspace={workspace} />;
}
