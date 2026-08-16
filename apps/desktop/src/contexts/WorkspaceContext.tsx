import { createContext } from "react";

/** Current workspace id, available to descendants via useContext. Empty
 * string means "no workspace selected yet" — happens briefly on first load
 * before `WorkspaceService.List` returns. The `useWorkspaceState` hook auto-selects
 * the first available workspace once the list arrives. */
export const WorkspaceContext = createContext<string>("");
