import { useContext, useState } from "react";
import { useSnapshot } from "../contexts/SnapshotContext";
import { WorkspaceContext } from "../contexts/WorkspaceContext";
import { useFeedback } from "../hooks/useFeedback";
import { AI_ASSIST_ICON, AI_ASSIST_LABEL, AI_ASSIST_PROVIDER_KEY } from "../lib/aiAssist";
import { useLocalStorage } from "../lib/useLocalStorage";
import { client, errorMessage } from "../services/client";
import { AiAssistModal } from "./AiAssistModal";
import { FeedbackText } from "./FeedbackText";
import { Spinner } from "./Spinner";

/** Wraps any controlled text editor with a hover "Use AI" pill that refills it
 * from a one-off provider prompt. The child is untouched. */
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

  return (
    <div className={`relative group ${className}`.trim()}>
      {children}
      {!busy && providers.length > 0 && (
        <button
          type="button"
          onClick={() => {
            setPrompt(value);
            setOpen(true);
          }}
          className="absolute top-1.5 right-1.5 z-10 hidden group-hover:flex group-focus-within:flex items-center gap-1 font-inter text-[11px] text-fg-subtle hover:text-fg bg-surface border border-border rounded px-1.5 py-0.5"
        >
          <AI_ASSIST_ICON size={11} />
          {AI_ASSIST_LABEL}
        </button>
      )}
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
  );
}
