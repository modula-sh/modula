import { createContext, useContext, useState } from "react";
import { useSnapshot } from "../contexts/SnapshotContext";
import { WorkspaceContext } from "../contexts/WorkspaceContext";
import { useFeedback } from "../hooks/useFeedback";
import { AI_ASSIST_ICON, AI_ASSIST_LABEL, AI_ASSIST_PROVIDER_KEY } from "../lib/aiAssist";
import { useLocalStorage } from "../lib/useLocalStorage";
import { client, errorMessage } from "../services/client";
import { AiAssistModal } from "./AiAssistModal";
import { FeedbackText } from "./FeedbackText";
import { Spinner } from "./Spinner";

/** Null outside an `AiAssist`, and when no provider is configured. */
const AiAssistContext = createContext<{ open: () => void; busy: boolean } | null>(null);

/** Matches the pill controls in the chat input's action row. */
const TRIGGER_CLASS =
  "flex shrink-0 items-center gap-1.5 rounded-full px-3 py-1.5 text-xs font-inter text-fg-muted transition-colors enabled:hover:bg-surface-2 enabled:hover:text-fg focus:outline-none disabled:opacity-50 disabled:cursor-not-allowed";

/** The "Use AI" button, for the field's own action row. */
export function AiAssistTrigger() {
  const assist = useContext(AiAssistContext);
  if (!assist) return null;
  return (
    <button type="button" disabled={assist.busy} onClick={assist.open} className={TRIGGER_CLASS}>
      <AI_ASSIST_ICON size={12} />
      {AI_ASSIST_LABEL}
    </button>
  );
}

/** Refills any controlled text editor from a one-off provider prompt, blurring
 * the wrapped field while it generates. */
export function AiAssist({
  value,
  onChange,
  fieldLabel,
  className = "",
  children,
}: {
  value: string;
  onChange: (text: string) => void;
  fieldLabel: string;
  /** Layout classes for the wrapper, when the field sizes itself in a parent. */
  className?: string;
  children: React.ReactNode;
}) {
  const ws = useContext(WorkspaceContext);
  const { snap } = useSnapshot();
  const providers = snap?.config?.providers ?? [];
  const [storedProvider, setStoredProvider] = useLocalStorage(AI_ASSIST_PROVIDER_KEY, "");
  const [open, setOpen] = useState(false);
  const [prompt, setPrompt] = useState("");
  const [busy, setBusy] = useState(false);
  const fb = useFeedback();

  const providerId = providers.some((p) => p.id === storedProvider)
    ? storedProvider
    : (providers[0]?.id ?? "");

  async function handleSubmit() {
    setOpen(false);
    setBusy(true);
    fb.clear();
    try {
      const text = await client.provider.generate(ws, {
        provider_id: providerId,
        instruction: prompt,
        field_label: fieldLabel,
      });
      onChange(text);
    } catch (e) {
      fb.err(errorMessage(e), { clearAfter: 8000 });
    } finally {
      setBusy(false);
    }
  }

  const handle = providers.length
    ? {
        busy,
        open: () => {
          setPrompt(value);
          setOpen(true);
        },
      }
    : null;

  return (
    <AiAssistContext.Provider value={handle}>
      <div className={`relative ${className}`.trim()}>
        {children}
        {busy && (
          <div className="absolute inset-0 z-10 flex items-center justify-center rounded-lg backdrop-blur-sm bg-bg/60">
            <Spinner size={18} />
          </div>
        )}
        {fb.feedback && (
          <div className="absolute -bottom-4 right-0">
            <FeedbackText feedback={fb.feedback} />
          </div>
        )}
        <AiAssistModal
          open={open}
          prompt={prompt}
          onPromptChange={setPrompt}
          providerId={providerId}
          onProviderChange={setStoredProvider}
          providers={providers}
          onSubmit={handleSubmit}
          onCancel={() => setOpen(false)}
        />
      </div>
    </AiAssistContext.Provider>
  );
}
