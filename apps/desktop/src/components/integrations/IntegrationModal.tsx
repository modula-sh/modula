import { useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { integrationKeys } from "../../queries/integration";
import { useSystemTools } from "../../queries/system";
import { client, errorMessage } from "../../services/client";
import type { Integration } from "../../types";
import { BaseModal } from "../BaseModal";
import { Button } from "../Button";
import { FieldRow } from "../FieldRow";
import { TextInput } from "../TextInput";

const TITLES: Record<string, string> = { github: "GitHub", jira: "Jira", linear: "Linear" };

/** Connect/edit form for one integration; mounts fresh per open, so field
 * state initializes straight from the existing row's `data`. */
export function IntegrationModal({
  workspace,
  id,
  existing,
  onClose,
}: {
  workspace: string;
  id: string;
  existing?: Integration;
  onClose: () => void;
}) {
  const queryClient = useQueryClient();
  const { data: tools } = useSystemTools();
  const ghInstalled = tools?.find((t) => t.id === "gh")?.installed ?? false;

  const data = existing?.data ?? {};
  const str = (key: string) => (typeof data[key] === "string" ? (data[key] as string) : "");
  const [useGhCli, setUseGhCli] = useState(data.use_gh_cli === true);
  const [account, setAccount] = useState(str("account"));
  const [baseUrl, setBaseUrl] = useState(str("base_url"));
  const [email, setEmail] = useState(str("email"));
  const [apiToken, setApiToken] = useState(str("api_token"));
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const payload: Record<string, unknown> =
    id === "github"
      ? { use_gh_cli: useGhCli, account: account.trim() }
      : id === "jira"
        ? { base_url: baseUrl.trim(), email: email.trim(), api_token: apiToken.trim() }
        : { api_token: apiToken.trim() };
  const incomplete =
    id === "github"
      ? !useGhCli
      : id === "jira"
        ? !baseUrl.trim() || !email.trim() || !apiToken.trim()
        : !apiToken.trim();

  async function run(action: () => Promise<void>) {
    setBusy(true);
    setError(null);
    try {
      await action();
      await queryClient.invalidateQueries({ queryKey: integrationKeys.all(workspace) });
      onClose();
    } catch (e: unknown) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <BaseModal open busy={busy} onCancel={onClose}>
      <div className="text-base font-semibold text-fg">{TITLES[id] ?? id}</div>
      <section className="border border-card-border/50 bg-card rounded-xl px-3">
        {id === "github" && (
          <>
            <FieldRow
              label="Use gh CLI"
              description={
                ghInstalled
                  ? "Search and fetch issues via the local gh CLI."
                  : "gh is not installed."
              }
            >
              <button
                type="button"
                onClick={() => setUseGhCli((v) => !v)}
                disabled={!ghInstalled}
                className={[
                  "px-3 py-1 rounded border text-xs transition-colors",
                  useGhCli
                    ? "border-border-focus bg-surface-2 text-fg"
                    : "border-border bg-surface text-fg-muted",
                  ghInstalled
                    ? "cursor-pointer hover:border-border-focus"
                    : "cursor-default opacity-40",
                ].join(" ")}
              >
                {useGhCli ? "On" : "Off"}
              </button>
            </FieldRow>
            <FieldRow
              label="Account"
              description="gh account to use here; blank uses the active one."
            >
              <TextInput
                mono
                value={account}
                onChange={(e) => setAccount(e.target.value)}
                placeholder="octocat"
              />
            </FieldRow>
          </>
        )}
        {id === "jira" && (
          <>
            <FieldRow label="Jira URL">
              <TextInput
                mono
                value={baseUrl}
                onChange={(e) => setBaseUrl(e.target.value)}
                placeholder="https://you.atlassian.net"
                autoFocus
              />
            </FieldRow>
            <FieldRow label="Email">
              <TextInput
                value={email}
                onChange={(e) => setEmail(e.target.value)}
                placeholder="you@example.com"
              />
            </FieldRow>
            <FieldRow label="API Token">
              <TextInput
                mono
                type="password"
                value={apiToken}
                onChange={(e) => setApiToken(e.target.value)}
              />
            </FieldRow>
          </>
        )}
        {id === "linear" && (
          <FieldRow label="API Key" description="Personal API key from Linear settings.">
            <TextInput
              mono
              type="password"
              value={apiToken}
              onChange={(e) => setApiToken(e.target.value)}
              autoFocus
            />
          </FieldRow>
        )}
      </section>
      <div className="flex items-center gap-2">
        {existing && (
          <Button
            onClick={() => run(() => client.integration.remove(workspace, id))}
            disabled={busy}
            tone="link"
          >
            Disconnect
          </Button>
        )}
        <span className="ml-auto flex items-center gap-2">
          <Button onClick={onClose} disabled={busy} tone="link">
            Cancel
          </Button>
          <Button
            onClick={() => run(() => client.integration.connect(workspace, id, payload))}
            disabled={busy || incomplete}
          >
            {busy ? "connecting…" : "Connect"}
          </Button>
        </span>
      </div>
      {error && <div className="text-[11px] text-red-400">{error}</div>}
    </BaseModal>
  );
}
