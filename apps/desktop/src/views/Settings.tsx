import { Button } from "../components/Button";
import { IntegrationsList } from "../components/integrations/IntegrationsList";
import { RecommendedToolsList } from "../components/recommended-tools/RecommendedToolsList";

export function SettingsView() {
  return (
    <main className="flex-1 overflow-y-auto px-4 pt-8 pb-4">
      <div className="max-w-4xl mx-auto space-y-8">
        <div className="space-y-2">
          <header>
            <h1 className="text-lg font-semibold text-fg font-inter">Settings</h1>
          </header>

          <section className="border border-card-border/50 bg-card rounded-xl p-3 space-y-2 text-[11px]">
            <div className="grid grid-cols-[90px_1fr] items-center gap-x-3">
              <span className="text-fg-subtle uppercase tracking-wide text-[10px]">version</span>
              <span className="font-mono text-fg">v{__APP_VERSION__}</span>
            </div>
          </section>
        </div>

        <section className="space-y-3">
          <h2 className="text-lg font-semibold text-fg font-inter">Recommended Tools</h2>
          <RecommendedToolsList />
        </section>

        <section className="space-y-3">
          <h2 className="text-lg font-semibold text-fg font-inter">Integrations</h2>
          <IntegrationsList />
        </section>

        <section className="space-y-3">
          <h2 className="text-lg font-semibold text-fg font-inter">Debug</h2>
          <Button
            onClick={() => {
              try {
                localStorage.removeItem("modula.onboarded");
              } catch {}
              window.location.reload();
            }}
          >
            Go To Onboarding
          </Button>
        </section>
      </div>
    </main>
  );
}
