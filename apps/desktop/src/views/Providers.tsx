import { useContext } from "react";
import { useNavigate } from "react-router-dom";
import { Button } from "../components/Button";
import { HeaderSlot } from "../components/HeaderSlot";
import { ProviderCard } from "../components/ProviderCard";
import { WorkspaceContext } from "../contexts/WorkspaceContext";
import { useProviders } from "../queries/provider";

export function ProvidersView() {
  const ws = useContext(WorkspaceContext);
  const navigate = useNavigate();
  const { data: providers, isPending } = useProviders(ws);

  return (
    <main className="flex-1 overflow-y-auto p-4 space-y-4">
      <HeaderSlot>
        <Button className="ml-auto" onClick={() => navigate("/providers/new")}>
          + New Provider
        </Button>
      </HeaderSlot>
      {isPending ? (
        <div className="text-fg-subtle text-sm">loading providers…</div>
      ) : !providers || providers.length === 0 ? (
        <div className="flex flex-col items-center text-center gap-1 py-24 font-inter">
          <div className="text-fg-muted text-sm">No providers</div>
          <div className="text-fg-subtle text-xs">
            Add a provider to let agents spawn against a CLI runtime.
          </div>
        </div>
      ) : (
        <div className="space-y-3">
          {providers.map((p) => (
            <ProviderCard
              key={p.id}
              provider={p}
              onOpen={() => navigate(`/providers/edit/${encodeURIComponent(p.id)}`)}
            />
          ))}
        </div>
      )}
    </main>
  );
}
