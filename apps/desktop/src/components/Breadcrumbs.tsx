import { Link, useLocation } from "react-router-dom";
import { useEntityLabels } from "../hooks/useEntityLabels";

type Crumb = { label: string; to?: string };

const ROOT_LABELS: Record<string, string> = {
  tasks: "Tasks",
  roadmap: "Roadmap",
  agents: "Agents",
  projects: "Projects",
  providers: "Providers",
  wiki: "Wiki",
  diffs: "Diffs",
  overview: "Overview",
  settings: "Settings",
};

const NEW_LABELS: Record<string, string> = {
  agents: "New agent",
  projects: "New project",
  providers: "New provider",
};

const SUBSECTION_LABELS: Record<string, Record<string, string>> = {
  agents: { logs: "Runs", usage: "Usage" },
};

function buildCrumbs(pathname: string, labels: Record<string, string>): Crumb[] {
  const parts = pathname.split("/").filter(Boolean);
  if (parts.length === 0) return [];
  const [section, ...rest] = parts;
  const rootLabel = ROOT_LABELS[section];
  if (!rootLabel) return [];

  if (rest.length === 0) return [{ label: rootLabel }];

  const crumbs: Crumb[] = [{ label: rootLabel, to: `/${section}` }];

  if (rest[0] === "new" && rest.length === 1) {
    crumbs.push({ label: NEW_LABELS[section] ?? "New" });
    return crumbs;
  }

  if (rest[0] === "edit" && rest.length >= 2) {
    const id = decodeURIComponent(rest[1]);
    crumbs.push({ label: labels[id] ?? id });
    return crumbs;
  }

  const subLabel = SUBSECTION_LABELS[section]?.[rest[0]];
  if (subLabel) {
    const subPrefix = `/${section}/${rest[0]}`;
    crumbs.push({ label: subLabel, to: rest.length > 1 ? subPrefix : undefined });
    let prefix = subPrefix;
    for (let i = 1; i < rest.length; i++) {
      prefix += `/${rest[i]}`;
      const raw = decodeURIComponent(rest[i]);
      crumbs.push({ label: labels[raw] ?? raw, to: i === rest.length - 1 ? undefined : prefix });
    }
    return crumbs;
  }

  let prefix = `/${section}`;
  for (let i = 0; i < rest.length; i++) {
    prefix += `/${rest[i]}`;
    const raw = decodeURIComponent(rest[i]);
    crumbs.push({
      label: labels[raw] ?? raw,
      to: i === rest.length - 1 ? undefined : prefix,
    });
  }
  return crumbs;
}

export function Breadcrumbs() {
  const { pathname } = useLocation();
  const labels = useEntityLabels();
  const crumbs = buildCrumbs(pathname, labels);
  if (crumbs.length === 0) return null;
  return (
    <nav
      className="flex items-center gap-1 text-xs font-inter uppercase tracking-wider font-medium min-w-0 select-none"
      aria-label="Breadcrumb"
    >
      {crumbs.map((c, i) => (
        <span key={i} className="flex items-center gap-1 min-w-0">
          {i > 0 && (
            <span className="text-fg-subtle/75 shrink-0" aria-hidden>
              /
            </span>
          )}
          {c.to ? (
            <Link to={c.to} className="text-fg-muted hover:text-fg transition-colors truncate">
              {c.label}
            </Link>
          ) : (
            <span className="text-fg truncate">{c.label}</span>
          )}
        </span>
      ))}
    </nav>
  );
}
