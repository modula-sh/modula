import type { ProviderDetail, ProviderSummary } from "../../../types";
import { call } from "../invoke";
import type { GenerateTextBody, ProviderWriteBody } from "../types";

export class ProviderResource {
  all(ws: string) {
    return call<ProviderSummary[]>("provider_list", { workspaceId: ws });
  }

  get(ws: string, id: string) {
    return call<ProviderDetail>("provider_get", { workspaceId: ws, providerId: id });
  }

  create(ws: string, body: ProviderWriteBody) {
    return call<{ id: string }>("provider_create", {
      workspaceId: ws,
      name: body.name,
      providerType: body.type,
      configDir: body.config_dir,
      description: body.description,
      mcpServers: body.mcp_servers ?? [],
    });
  }

  update(ws: string, id: string, body: ProviderWriteBody) {
    return call<void>("provider_update", {
      workspaceId: ws,
      providerId: id,
      name: body.name,
      providerType: body.type,
      configDir: body.config_dir,
      description: body.description ?? undefined,
      clearDescription: body.description === null,
      mcpServers: body.mcp_servers,
    });
  }

  generate(ws: string, body: GenerateTextBody) {
    return call<string>("provider_generate", {
      workspaceId: ws,
      providerId: body.provider_id,
      model: body.model,
      instruction: body.instruction,
      currentText: body.current_text,
      fieldLabel: body.field_label,
    });
  }

  delete(ws: string, id: string) {
    return call<void>("provider_delete", { workspaceId: ws, providerId: id });
  }
}
