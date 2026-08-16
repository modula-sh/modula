import { useNavigate } from "react-router-dom";
import { AgentCard } from "../components/AgentCard";
import { Button } from "../components/Button";
import { TabsNav } from "../components/TabsNav";
import { useSnapshot } from "../contexts/SnapshotContext";

export function AgentsView() {
  const navigate = useNavigate();
  const { snap } = useSnapshot();
  const running = snap?.agents ?? [];
  const runs = snap?.runs ?? [];
  const agents = snap?.config.agents ?? [];

  return (
    <div className="flex-1 flex flex-col overflow-hidden">
      <TabsNav right={<Button onClick={() => navigate("/agents/new")}>+ New Agent</Button>} />
      <main className="flex-1 overflow-hidden">
        <div className="h-full overflow-y-auto">
          {!snap ? (
            <div className="h-full flex items-center justify-center text-fg-subtle">
              loading agents…
            </div>
          ) : agents.length === 0 ? (
            <div className="flex flex-col items-center text-center gap-1 py-24 font-inter">
              <div className="text-fg-muted text-sm">No agents</div>
              <div className="text-fg-subtle text-xs">
                Add an agent to react to events and run work on tasks.
              </div>
            </div>
          ) : (
            <div className="flex flex-col gap-6 p-4">
              {(() => {
                const scheduledAgents = agents.filter((a) => a.schedule);
                const ruleAgents = agents.filter((a) => !a.schedule && a.rules.length > 0);
                const manualAgents = agents.filter((a) => !a.schedule && a.rules.length === 0);

                const renderCard = (a: (typeof agents)[number]) => {
                  const isRunning = running.some((r) => r.name === a.name);
                  const recent = runs.find((r) => r.agent_name === a.name);
                  const lastTs =
                    recent?.finished_at ?? recent?.started_at ?? recent?.created_at ?? null;
                  return (
                    <AgentCard
                      key={a.name}
                      agent={a}
                      isRunning={isRunning}
                      lastLog={lastTs}
                      onOpen={() => navigate(`/agents/edit/${a.id}`)}
                    />
                  );
                };

                const renderSection = (title: string, list: typeof agents) =>
                  list.length === 0 ? null : (
                    <section className="flex flex-col gap-3">
                      <h2 className="text-xs font-semibold uppercase tracking-wide text-fg-subtle font-inter">
                        {title}
                      </h2>
                      <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
                        {list.map(renderCard)}
                      </div>
                    </section>
                  );

                return (
                  <>
                    {renderSection("Manual", manualAgents)}
                    {renderSection("Scheduled", scheduledAgents)}
                    {renderSection("Rules", ruleAgents)}
                  </>
                );
              })()}
            </div>
          )}
        </div>
      </main>
    </div>
  );
}
