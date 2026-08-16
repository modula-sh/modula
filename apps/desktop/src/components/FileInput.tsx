import { open } from "@tauri-apps/plugin-dialog";
import { Folder } from "lucide-react";
import { TextInput } from "./TextInput";

/** Text input paired with a folder-icon button that opens the native file
 *  picker. Falls back to a disabled button outside Tauri (e.g. browser dev). */
export function FileInput({
  value,
  onChange,
  directory = false,
  placeholder,
  className = "",
  mono,
  padded,
  autoFocus,
}: {
  value: string;
  onChange: (next: string) => void;
  /** When true, the picker selects a folder instead of a file. */
  directory?: boolean;
  placeholder?: string;
  className?: string;
  mono?: boolean;
  padded?: boolean;
  autoFocus?: boolean;
}) {
  async function pick() {
    try {
      const selected = await open({
        directory,
        multiple: false,
        defaultPath: value || undefined,
      });
      if (typeof selected === "string") onChange(selected);
    } catch {
      // Not in Tauri context or user dismissed — leave value alone.
    }
  }
  return (
    <div className={`relative ${className}`.trim()}>
      <TextInput
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={placeholder}
        mono={mono}
        padded={padded}
        autoFocus={autoFocus}
        className="w-full pr-8"
      />
      <button
        type="button"
        onClick={pick}
        title={directory ? "Pick folder" : "Pick file"}
        className="absolute inset-y-0 right-0 px-2 flex items-center text-fg-subtle hover:text-fg"
      >
        <Folder size={14} />
      </button>
    </div>
  );
}
