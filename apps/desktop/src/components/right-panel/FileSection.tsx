import { type FilePatch, type FilePatchAction, FilePatchRow } from "./FilePatchRow";

// Standard panel section: a 40px header bar (matches the conversation
// header height) followed by a bordered, divided list of collapsible file
// diffs. Used by every right-panel content type that lists patches.
export function FileSection({
  header,
  headerExtra,
  headerAction,
  files,
  fileRefs,
  refKeyFor,
  autoOpenPath,
  actionFor,
}: {
  header: React.ReactNode;
  headerExtra?: React.ReactNode;
  headerAction?: FilePatchAction;
  files: FilePatch[];
  fileRefs: React.RefObject<Map<string, HTMLDivElement>>;
  refKeyFor: (path: string) => string;
  autoOpenPath?: string;
  actionFor?: (file: FilePatch) => FilePatchAction | undefined;
}) {
  if (files.length === 0) return null;
  return (
    <section>
      <h3 className="h-10 flex items-center gap-2 px-3 border-b border-border text-[10px] uppercase tracking-wide text-fg-subtle/70 font-inter">
        <span className="flex-1 min-w-0 truncate">{header}</span>
        {headerExtra && <span className="shrink-0">{headerExtra}</span>}
        {headerAction && (
          <button
            type="button"
            onClick={headerAction.onClick}
            title={headerAction.title}
            className="shrink-0 p-1 rounded text-fg-subtle hover:text-fg hover:bg-surface-2 transition-colors"
          >
            {headerAction.icon}
          </button>
        )}
      </h3>
      <div className="border-b border-border divide-y divide-border">
        {files.map((f) => (
          <FilePatchRow
            key={f.path}
            file={f}
            refKey={refKeyFor(f.path)}
            fileRefs={fileRefs}
            defaultOpen={autoOpenPath === f.path}
            action={actionFor?.(f)}
          />
        ))}
      </div>
    </section>
  );
}
