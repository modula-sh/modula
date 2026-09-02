import type { ProviderModel } from "../../lib/providerCatalog";
import { useLocalStorage } from "../../lib/useLocalStorage";
import { DropdownSelect } from "../DropdownMenu";
import { ChatInputShell } from "./ChatInputShell";
import { SendButton } from "./SendButton";

export function ChatInput({
  onSend,
  onQueue,
  onCancel,
  streaming,
  models,
  selectedModel,
  onModelChange,
  draftKey,
}: {
  onSend: (text: string) => void;
  onQueue: (text: string) => void;
  onCancel: () => void;
  streaming: boolean;
  models: ProviderModel[];
  selectedModel: string | null;
  onModelChange: (m: string | null) => void;
  draftKey: string;
}) {
  const [text, setText] = useLocalStorage(draftKey, "");

  function submit() {
    const trimmed = text.trim();
    if (streaming && !trimmed) {
      onCancel();
      return;
    }
    if (!trimmed) return;
    if (streaming) onQueue(trimmed);
    else onSend(trimmed);
    setText("");
  }

  const buttonDisabled = !streaming && !text.trim();

  return (
    <ChatInputShell
      value={text}
      onChange={setText}
      onSubmit={submit}
      placeholder="Send a message (⌘↵)"
      bottomRow={
        <>
          <DropdownSelect
            value={selectedModel ?? ""}
            options={[
              { value: "", label: "Default Model" },
              ...models.map((m) => ({ value: m.id, label: m.label })),
            ]}
            onChange={(v) => onModelChange(v === "" ? null : v)}
            disabled={streaming}
            title="Model. Changes apply to the next message"
          />
          <div className="ml-auto">
            <SendButton
              onClick={submit}
              disabled={buttonDisabled}
              busy={streaming && !text.trim()}
            />
          </div>
        </>
      }
    />
  );
}
