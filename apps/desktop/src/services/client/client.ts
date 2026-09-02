// The engine API client: a singleton exposing namespaced, typed methods that
// own the gRPC bridge command, args, and response typing. All frontend engine
// access goes through `client.<domain>.<method>(...)`; workspace-scoped methods
// take the workspace id as their first argument. Unary calls go over Tauri
// `invoke` (→ `src-tauri/src/grpc/*`); streams go over Tauri `Channel`.

import { AgentResource } from "./resources/agent";
import { CatalogResource } from "./resources/catalog";
import { ConversationResource } from "./resources/conversation";
import { DiffResource } from "./resources/diff";
import { IntegrationResource } from "./resources/integration";
import { LabelResource } from "./resources/label";
import { ProjectResource } from "./resources/project";
import { ProviderResource } from "./resources/provider";
import { RemoteResource } from "./resources/remote";
import { RoadmapResource } from "./resources/roadmap";
import { SearchResource } from "./resources/search";
import { SnapshotResource } from "./resources/snapshot";
import { SystemResource } from "./resources/system";
import { TaskResource } from "./resources/task";
import { ThreadResource } from "./resources/thread";
import { UsageResource } from "./resources/usage";
import { VariantResource } from "./resources/variant";
import { WikiResource } from "./resources/wiki";
import { WorkspaceResource } from "./resources/workspace";

export const client = Object.freeze({
  workspace: new WorkspaceResource(),
  task: new TaskResource(),
  label: new LabelResource(),
  integration: new IntegrationResource(),
  variant: new VariantResource(),
  roadmap: new RoadmapResource(),
  remote: new RemoteResource(),
  search: new SearchResource(),
  snapshot: new SnapshotResource(),
  thread: new ThreadResource(),
  agent: new AgentResource(),
  project: new ProjectResource(),
  diff: new DiffResource(),
  provider: new ProviderResource(),
  conversation: new ConversationResource(),
  wiki: new WikiResource(),
  usage: new UsageResource(),
  system: new SystemResource(),
  catalog: new CatalogResource(),
});

export { errorMessage } from "./invoke";
