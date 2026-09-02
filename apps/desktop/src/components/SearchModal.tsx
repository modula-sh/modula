import { useContext, useEffect, useRef, useState } from "react";
import { useNavigate } from "react-router-dom";
import { WorkspaceContext } from "../contexts/WorkspaceContext";
import { SEARCH_KINDS } from "../lib/search";
import { useDebouncedValue } from "../lib/useDebounced";
import { useSearch } from "../queries/search";
import type { SearchHit, SearchKind } from "../types";
import { BaseModal } from "./BaseModal";
import { Spinner } from "./Spinner";

const DEBOUNCE_MS = 250;

/** Contiguous runs of one kind. The engine returns each source's hits together
 * and already ranked, so grouping is a scan and never a re-sort. */
function groupByKind(hits: SearchHit[]) {
  const groups: { kind: SearchKind; hits: SearchHit[] }[] = [];
  for (const hit of hits) {
    const last = groups.at(-1);
    if (last?.kind === hit.kind) last.hits.push(hit);
    else groups.push({ kind: hit.kind, hits: [hit] });
  }
  return groups;
}

export function SearchModal({ onClose }: { onClose: () => void }) {
  const ws = useContext(WorkspaceContext);
  const navigate = useNavigate();
  const [query, setQuery] = useState("");
  const debounced = useDebouncedValue(query.trim(), DEBOUNCE_MS);
  const { data, isFetching } = useSearch(ws, debounced);
  const [active, setActive] = useState(0);
  const activeRef = useRef<HTMLButtonElement>(null);

  const hits = data ?? [];

  useEffect(() => {
    setActive(0);
  }, [debounced]);

  useEffect(() => {
    activeRef.current?.scrollIntoView({ block: "nearest" });
  }, [active]);

  function open(hit: SearchHit) {
    navigate(SEARCH_KINDS[hit.kind].path(hit.id));
    onClose();
  }

  function onKeyDown(e: React.KeyboardEvent) {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setActive((i) => Math.min(i + 1, hits.length - 1));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setActive((i) => Math.max(i - 1, 0));
    } else if (e.key === "Enter" && hits[active]) {
      e.preventDefault();
      open(hits[active]);
    }
  }

  return (
    <BaseModal
      open
      onCancel={onClose}
      align="top"
      chromeless
      panelClassName="w-[36rem] bg-bg/70 backdrop-blur-2xl border border-border rounded-xl shadow-popover"
    >
      <input
        autoFocus
        value={query}
        onChange={(e) => setQuery(e.target.value)}
        onKeyDown={onKeyDown}
        placeholder="Search tasks, conversations, agents..."
        className="w-full bg-transparent border-0 focus:outline-none px-4 py-3 text-sm text-fg placeholder-fg-subtle"
      />
      <div className="border-b border-edge" />
      <div className="max-h-[50vh] overflow-y-auto p-1.5">
        {debounced.length === 0 ? (
          <div className="px-2.5 py-6 text-center text-xs text-fg-subtle">
            Search tasks, chats, agents, projects, providers and the wiki.
          </div>
        ) : isFetching && hits.length === 0 ? (
          <div className="flex justify-center py-6">
            <Spinner />
          </div>
        ) : hits.length === 0 ? (
          <div className="px-2.5 py-6 text-center text-xs text-fg-subtle">No results</div>
        ) : (
          groupByKind(hits).map((group) => (
            <div key={group.kind}>
              <div className="px-2.5 pt-2 pb-1 text-[10px] uppercase tracking-wider text-fg-subtle/70">
                {SEARCH_KINDS[group.kind].label}
              </div>
              {group.hits.map((hit) => {
                const index = hits.indexOf(hit);
                const isActive = index === active;
                const Icon = SEARCH_KINDS[hit.kind].icon;
                return (
                  <button
                    key={`${hit.kind}:${hit.id}`}
                    ref={isActive ? activeRef : undefined}
                    type="button"
                    onClick={() => open(hit)}
                    onMouseMove={() => setActive(index)}
                    className={`w-full flex items-start gap-2.5 px-2.5 py-1.5 rounded text-left transition-colors ${
                      isActive ? "bg-surface-2/60" : "hover:bg-surface-2/30"
                    }`}
                  >
                    <Icon size={15} className="shrink-0 mt-0.5 text-fg-subtle" />
                    <span className="min-w-0 flex-1 flex flex-col gap-0.5">
                      <span className="flex items-baseline gap-2 min-w-0">
                        <span className="truncate text-[13px] text-fg">{hit.title}</span>
                        {hit.subtitle && (
                          <span className="shrink-0 text-[10px] text-fg-subtle truncate">
                            {hit.subtitle}
                          </span>
                        )}
                      </span>
                      {hit.excerpt.length > 0 && (
                        <span className="truncate text-[11px] text-fg-muted">
                          {hit.excerpt.map((span, i) =>
                            span.is_match ? (
                              <mark key={i} className="bg-fg/15 text-fg rounded-[2px]">
                                {span.text}
                              </mark>
                            ) : (
                              <span key={i}>{span.text}</span>
                            ),
                          )}
                        </span>
                      )}
                    </span>
                  </button>
                );
              })}
            </div>
          ))
        )}
      </div>
    </BaseModal>
  );
}
