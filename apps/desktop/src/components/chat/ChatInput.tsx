import type { ProviderModel } from "../../lib/providerCatalog";
import { useLocalStorage } from "../../lib/useLocalStorage";
import { AiAssist } from "../AiAssist";
import { DropdownSelect } from "../DropdownMenu";
import { ChatInputShell } from "./ChatInputShell";
import { SendButton } from "./SendButton";

export function ChatInput({
  onSend,
  onCancel,
  streaming,
  models,
  selectedModel,
  onModelChange,
  draftKey,
}: {
  onSend: (text: string) => void;
  onCancel: () => void;
  streaming: boolean;
  models: ProviderModel[];
  selectedModel: string | null;
  onModelChange: (m: string | null) => void;
  draftKey: string;
}) {
  const [text, setText] = useLocalStorage(draftKey, "");

  function submit() {
    if (streaming) {
      onCancel();
      return;
    }
    if (!text.trim()) return;
    onSend(text.trim());
    setText("");
  }

  const buttonDisabled = streaming ? false : !text.trim();

  return (
    <AiAssist value={text} onChange={setText} fieldLabel="chat message">
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
              <SendButton onClick={submit} disabled={buttonDisabled} busy={streaming} />
            </div>
          </>
        }
      />
    </AiAssist>
  );
}
