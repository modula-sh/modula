import { Button } from "../components/Button";
import { TERMINAL_MODE_KEY } from "../components/chat/TerminalView";
import { DropdownSelect } from "../components/DropdownMenu";
import { FieldRow } from "../components/FieldRow";
import { IntegrationsList } from "../components/integrations/IntegrationsList";
import { RecommendedToolsList } from "../components/recommended-tools/RecommendedToolsList";
import { RemoteAccessPanel } from "../components/remote/RemoteAccessPanel";
import { Switch } from "../components/Switch";
import { useThemeContext } from "../contexts/ThemeContext";
import { THEMES, type Theme } from "../hooks/useTheme";
import { useLocalStorage } from "../lib/useLocalStorage";
import { useRemoteAvailable } from "../queries/remote";

export function SettingsView() {
  const { data: remoteAvailable } = useRemoteAvailable();
  return (
    <main className="flex-1 overflow-y-auto px-4 pt-8 pb-4 font-inter">
      <div className="max-w-4xl mx-auto space-y-8">
        <div className="space-y-2">
          <header>
            <h1 className="text-lg font-semibold text-fg">Settings</h1>
          </header>

          <section className="border border-card-border/50 bg-card rounded-xl p-3 space-y-2 text-[11px]">
            <div className="grid grid-cols-[90px_1fr] items-center gap-x-3">
              <span className="text-fg-subtle uppercase tracking-wide text-[10px]">version</span>
              <span className="text-fg">v{__APP_VERSION__}</span>
            </div>
          </section>
        </div>

        <section className="space-y-3">
          <h2 className="text-lg font-semibold text-fg">Appearance</h2>
          <AppearanceSettings />
        </section>

        <section className="space-y-3">
          <h2 className="text-lg font-semibold text-fg">Conversations</h2>
          <ConversationSettings />
        </section>

        <section className="space-y-3">
          <h2 className="text-lg font-semibold text-fg">Recommended Tools</h2>
          <RecommendedToolsList />
        </section>

        <section className="space-y-3">
          <h2 className="text-lg font-semibold text-fg">Integrations</h2>
          <IntegrationsList />
        </section>

        {remoteAvailable && (
          <section className="space-y-3">
            <h2 className="text-lg font-semibold text-fg">Remote access</h2>
            <RemoteAccessPanel />
          </section>
        )}

        <section className="space-y-3">
          <h2 className="text-lg font-semibold text-fg">Debug</h2>
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

function AppearanceSettings() {
  const { theme, setTheme } = useThemeContext();
  return (
    <section className="border border-card-border/50 bg-card rounded-xl px-3">
      <FieldRow label="Theme" description="Glass variants blur whatever is behind the window.">
        <DropdownSelect
          variant="field"
          padded
          className="w-44"
          panelClassName="w-44"
          value={theme}
          onChange={(v) => setTheme(v as Theme)}
          options={THEMES}
        />
      </FieldRow>
    </section>
  );
}

function ConversationSettings() {
  const [terminal, setTerminal] = useLocalStorage(TERMINAL_MODE_KEY, false);
  return (
    <section className="border border-card-border/50 bg-card rounded-xl px-3">
      <FieldRow
        label="Open new conversations in terminal"
        description="Runs the provider's own CLI. Claude and Gemini only; others open as chat."
      >
        <Switch
          checked={terminal}
          onChange={setTerminal}
          label="Open new conversations in terminal"
        />
      </FieldRow>
    </section>
  );
}
