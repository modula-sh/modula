// Overview — isometric 3D dashboard of factory state.
// Self-contained: only React + three. Safe to delete this file (and its
// nav entry in App.tsx) without touching anything else.
//
// Design rules:
//   - Every visual element encodes data. Decorative geometry is forbidden.
//   - Static camera; no animation that doesn't reflect a state change.
//   - Restrained palette: only severity (rework/blocked) and live activity
//     earn color. Everything else is monochrome slate.
//   - Role conveyed by label, not color. One accent (cyan) marks "live."

import { useEffect, useRef } from "react";
import * as THREE from "three";
import { useThemeContext } from "./contexts/ThemeContext";
import type { Theme } from "./hooks/useTheme";
import { useSnapshotQuery } from "./queries/snapshot";

type Approved = boolean | null;

/** Pipeline entry sourced from the `pipeline` config block (rows in
 * `pipeline_statuses`). Drives station grouping, status colors, and special-
 * case treatment of error/terminal states. The frontend never assumes a
 * specific set of status keys. */
type PipelineTone = "zinc" | "yellow" | "red" | "blue" | "purple" | "green";

interface PipelineStatus {
  key: string;
  label: string;
  tone: PipelineTone;
  station?: string | null;
  terminal?: boolean;
  error?: boolean;
}

interface Task {
  id: string;
  /** External system identifier (e.g. "JIRA-123"). Null for internal tasks. */
  external_id?: string | null;
  title: string;
  approved: Approved;
  variants: { id: string; status: string }[];
  projects_touched?: string[];
}

interface RoadmapItem {
  task: string;
  /** Free-form status string — must match a `key` in `pipeline[]`. */
  status: string;
  variants: string[];
}

interface Agent {
  pid: number;
  name: string;
  task?: string | null;
  spec?: string | null;
  branch?: string | null;
  started_at?: string;
}

interface Snapshot {
  tasks: Task[];
  roadmap: RoadmapItem[];
  agents: Agent[];
  config?: {
    projects?: { name?: string; path?: string }[];
    pipeline?: PipelineStatus[];
  };
  ts: string;
}

//
// Stations are derived from the `pipeline` config block. Statuses with a
// `station:` field group together (adjacent statuses sharing a station name
// merge into one tile); statuses with `error: true` are excluded from the
// conveyor and rendered as accents instead. An implicit "INBOX" station
// at the leftmost position holds tasks that don't have a roadmap entry
// yet (= sit outside the pipeline) and workspace-scoped agents (= no task).

interface Station {
  key: string;
  code: string;
  name: string;
  statuses: string[]; // pipeline keys grouped under this station
  x: number;
}

const INBOX_KEY = "_inbox";
const STATION_RAIL_HALF = 12.5; // ±x extent of the station rail

/** Build the station list from a workspace's pipeline config.
 *
 * Layout strategy: deduplicate `station` names in pipeline order (skipping
 * pipeline entries with `error: true`). Always prepend an implicit INBOX
 * station for tasks without a roadmap entry. Distribute stations across
 * [-STATION_RAIL_HALF, +STATION_RAIL_HALF] equally. */
function buildStations(pipeline: PipelineStatus[]): Station[] {
  const orderedNames: string[] = [];
  const statusesByStation = new Map<string, string[]>();
  for (const p of pipeline) {
    if (p.error) continue;
    const stationName = p.station;
    if (!stationName) continue;
    if (!statusesByStation.has(stationName)) {
      orderedNames.push(stationName);
      statusesByStation.set(stationName, []);
    }
    statusesByStation.get(stationName)!.push(p.key);
  }
  // Implicit inbox always leftmost.
  const all: { name: string; statuses: string[] }[] = [
    { name: "INBOX", statuses: [] },
    ...orderedNames.map((n) => ({ name: n, statuses: statusesByStation.get(n) ?? [] })),
  ];
  const span = STATION_RAIL_HALF * 2;
  return all.map((s, i) => ({
    key: i === 0 ? INBOX_KEY : s.name.toLowerCase().replace(/\s+/g, "-"),
    code: `STN-${String(i + 1).padStart(2, "0")}`,
    name: s.name,
    statuses: s.statuses,
    x: all.length === 1 ? 0 : -STATION_RAIL_HALF + (span * i) / (all.length - 1),
  }));
}

/** Compact label for an agent name. Hyphenated names → first letter of each
 * segment uppercased ("code-reviewer" → "CR"). Single-word names → first 3
 * chars uppercased ("researcher" → "RES"). Caps at 4 chars so it stays
 * readable in 3D. */
function shortLabelForAgent(name: string): string {
  if (!name) return "?";
  const segments = name.split("-").filter(Boolean);
  if (segments.length >= 2) {
    return segments
      .map((s) => s[0]?.toUpperCase() ?? "")
      .join("")
      .slice(0, 4);
  }
  return name.slice(0, 3).toUpperCase();
}

interface ProjectPlaque {
  name: string;
  code: string;
  x: number;
}
const PROJECT_Z = -3.8;
const PLAQUE_W = 2.5;
const PLAQUE_H = 0.08;
const PLAQUE_D = 1.4;
const PROJECT_RAIL_HALF_W = 5.5; // ±x extent of the project rail

function layoutProjects(names: string[]): ProjectPlaque[] {
  if (names.length === 0) return [];
  if (names.length === 1) return [{ name: names[0], code: "PRJ-A", x: 0 }];
  const span = PROJECT_RAIL_HALF_W * 2;
  return names.map((name, i) => ({
    name,
    code: `PRJ-${String.fromCharCode(65 + i)}`,
    x: -PROJECT_RAIL_HALF_W + (span * i) / (names.length - 1),
  }));
}

//
// The 3D scene is rendered with raw THREE.js, so it can't use Tailwind's
// CSS-variable token layer directly. We instead compute the same palette
// twice (light + dark) and re-render the scene whenever the active theme
// changes (see the `[theme]` dep on the main effect below). Severity
// colors (rework/blocked/live) stay near-identical across themes; surface
// colors (bg/grid/station/agent tiles) and label text invert.

type TaskState = "pending" | "approved" | "rework" | "blocked" | "accepted" | "rejected";

interface ScenePalette {
  // Surfaces (THREE.Color hex)
  bg: number;
  grid: number;
  flowLine: number;
  stationBase: number;
  stationEdge: number;
  stationLive: number;
  taskDim: number;
  taskLive: number;
  taskRework: number;
  taskBlocked: number;
  taskAccepted: number;
  taskRejected: number;
  agentTile: number;
  agentEdge: number;
  agentPin: number;
  // Sprite-canvas text (CSS hex)
  textCode: string;
  textName: string;
  textMeta: string;
  textPrimary: string;
  textSubtle: string;
  // HUD severity / swatch text (CSS hex)
  hudWarn: string;
  hudErr: string;
  hudLive: string;
  taskHex: Record<TaskState, string>;
}

const PALETTES: Record<Theme, ScenePalette> = {
  dark: {
    bg: 0x09090b,
    grid: 0x27272a,
    flowLine: 0x2a3a52,
    stationBase: 0x0f0f11,
    stationEdge: 0x3f3f46,
    stationLive: 0x22d3ee,
    taskDim: 0x475569,
    taskLive: 0x64748b,
    taskRework: 0xf59e0b,
    taskBlocked: 0xef4444,
    taskAccepted: 0x65a30d,
    taskRejected: 0x27303f,
    agentTile: 0x18181b,
    agentEdge: 0x22d3ee,
    agentPin: 0x22d3ee,
    textCode: "#475569",
    textName: "#cbd5e1",
    textMeta: "#94a3b8",
    textPrimary: "#e2e8f0",
    textSubtle: "#64748b",
    hudWarn: "#f59e0b",
    hudErr: "#ef4444",
    hudLive: "#22d3ee",
    taskHex: {
      pending: "#475569",
      approved: "#64748b",
      rework: "#f59e0b",
      blocked: "#ef4444",
      accepted: "#65a30d",
      rejected: "#27303f",
    },
  },
  light: {
    bg: 0xffffff, // page bg — matches other views
    grid: 0xd4d4d8, // zinc-300 — subtle gridlines
    flowLine: 0x71717a, // zinc-500 — visible flow against light bg
    stationBase: 0xfafafa, // zinc-50  — raised station card
    stationEdge: 0x52525b, // zinc-600
    stationLive: 0x0891b2, // cyan-600 — saturated for light bg
    taskDim: 0xa1a1aa, // zinc-400
    taskLive: 0x71717a, // zinc-500
    taskRework: 0xd97706, // amber-600
    taskBlocked: 0xdc2626, // red-600
    taskAccepted: 0x4d7c0f, // lime-700
    taskRejected: 0xd4d4d8, // zinc-300
    agentTile: 0xe4e4e7, // zinc-200
    agentEdge: 0x0891b2, // cyan-600
    agentPin: 0x0891b2,
    textCode: "#71717a", // zinc-500
    textName: "#18181b", // zinc-900 — primary on light bg
    textMeta: "#52525b", // zinc-600
    textPrimary: "#18181b", // zinc-900
    textSubtle: "#71717a", // zinc-500
    hudWarn: "#d97706",
    hudErr: "#dc2626",
    hudLive: "#0891b2",
    taskHex: {
      pending: "#a1a1aa",
      approved: "#71717a",
      rework: "#d97706",
      blocked: "#dc2626",
      accepted: "#4d7c0f",
      rejected: "#d4d4d8",
    },
  },
};

function taskColor(p: ScenePalette, state: TaskState): number {
  switch (state) {
    case "pending":
      return p.taskDim;
    case "approved":
      return p.taskLive;
    case "rework":
      return p.taskRework;
    case "blocked":
      return p.taskBlocked;
    case "accepted":
      return p.taskAccepted;
    case "rejected":
      return p.taskRejected;
  }
}

function useSnapshot(workspace: string): Snapshot | null {
  return (useSnapshotQuery(workspace).data as Snapshot | undefined) ?? null;
}

function makeTextSprite(
  text: string,
  color = "#cbd5e1",
  px = 56,
  weight: 400 | 600 | 700 = 600,
): THREE.Sprite {
  // Scale canvas to text length so fonts stay crisp; sprite world height
  // is fixed (px controls resolution, not on-screen size).
  const canvas = document.createElement("canvas");
  const measureCtx = canvas.getContext("2d")!;
  measureCtx.font = `${weight} ${px}px "IBM Plex Mono", ui-monospace, "SF Mono", Menlo, Consolas, monospace`;
  const metrics = measureCtx.measureText(text);
  const w = Math.max(64, Math.ceil(metrics.width + 32));
  const h = px + 24;
  canvas.width = w;
  canvas.height = h;
  const ctx = canvas.getContext("2d")!;
  ctx.font = `${weight} ${px}px "IBM Plex Mono", ui-monospace, "SF Mono", Menlo, Consolas, monospace`;
  ctx.textAlign = "center";
  ctx.textBaseline = "middle";
  ctx.fillStyle = color;
  ctx.fillText(text, w / 2, h / 2);
  const tex = new THREE.CanvasTexture(canvas);
  tex.minFilter = THREE.LinearFilter;
  tex.magFilter = THREE.LinearFilter;
  tex.anisotropy = 4;
  const mat = new THREE.SpriteMaterial({ map: tex, transparent: true, depthWrite: false });
  const sprite = new THREE.Sprite(mat);
  const heightWorld = 0.5;
  const aspect = w / h;
  sprite.scale.set(heightWorld * aspect, heightWorld, 1);
  return sprite;
}

function taskState(t: Task, status: string | null, pipeline: PipelineStatus[]): TaskState {
  const cfg = status ? pipeline.find((p) => p.key === status) : null;
  const variants = t.variants ?? [];
  const hasAccepted = variants.some((v) => v.status === "accepted");
  const hasRework = variants.some((v) => v.status === "rework");
  if (cfg?.error) return "blocked";
  if (cfg?.terminal || hasAccepted) return "accepted";
  if (t.approved === false) return "rejected";
  if (hasRework) return "rework";
  if (t.approved === null) return "pending";
  return "approved";
}

function stationForTask(
  taskId: string,
  roadmapByTask: Map<string, RoadmapItem>,
  stations: Station[],
): { station: Station; status: string | null } {
  const r = roadmapByTask.get(taskId);
  if (!r) return { station: stations[0], status: null };
  const s = stations.find((st) => st.statuses.includes(r.status));
  return { station: s ?? stations[0], status: r.status };
}

/** Resolve an agent to the task id it's working on. Workers are spawned
 * with `--spec`/`--branch` (no `--task`), so we fall back to substring
 * matching against the workspace's task UUIDs. */
function taskIdForAgent(a: Agent, taskIdsByLengthDesc: string[]): string | null {
  if (a.task) return a.task;
  const spec = a.spec ?? "";
  const branch = (a.branch ?? "").toLowerCase();
  for (const id of taskIdsByLengthDesc) {
    if (spec.includes(id)) return id;
    if (branch.includes(id.toLowerCase())) return id;
  }
  return null;
}

function elapsedShort(iso: string | undefined): string {
  if (!iso) return "";
  const ms = Date.now() - new Date(iso).getTime();
  if (Number.isNaN(ms) || ms < 0) return "";
  const m = Math.floor(ms / 60000);
  if (m < 1) return "<1m";
  if (m < 60) return `${m}m`;
  const h = Math.floor(m / 60);
  return `${h}h${(m % 60).toString().padStart(2, "0")}`;
}

export default function Overview({ workspace }: { workspace: string }) {
  const mountRef = useRef<HTMLDivElement>(null);
  const snap = useSnapshot(workspace);
  const { theme } = useThemeContext();

  // Snap held in a ref so the scene-construction effect (which reruns on
  // theme change) can rebuild with the latest data without depending on
  // `snap` directly — depending on snap would tear the scene down on every
  // SSE tick.
  const snapRef = useRef<Snapshot | null>(null);
  useEffect(() => {
    snapRef.current = snap;
  }, [snap]);

  useEffect(() => {
    const mount = mountRef.current;
    if (!mount) return;
    while (mount.firstChild) mount.removeChild(mount.firstChild);
    const palette = PALETTES[theme];

    const renderer = new THREE.WebGLRenderer({ antialias: true });
    renderer.setPixelRatio(window.devicePixelRatio);
    const canvas = renderer.domElement;
    canvas.style.cssText = "display:block;position:absolute;inset:0;width:100%;height:100%;";
    mount.appendChild(canvas);

    const scene = new THREE.Scene();
    scene.background = new THREE.Color(palette.bg);

    const lookTarget = new THREE.Vector3(0, 0.5, 0);
    const camera = new THREE.OrthographicCamera(-10, 10, 6, -6, 0.1, 400);
    camera.position.copy(lookTarget).add(new THREE.Vector3(50, 50, 50));
    camera.lookAt(lookTarget);

    // World extents we want visible at any aspect ratio.
    const TARGET_W = 34;
    const TARGET_H = 15;
    function fit() {
      if (!mount) return;
      const w = mount.clientWidth || 1;
      const h = mount.clientHeight || 1;
      renderer.setSize(w, h, false);
      const aspect = w / h;
      const d = Math.max(TARGET_H / 2, TARGET_W / (2 * aspect));
      camera.left = -d * aspect;
      camera.right = d * aspect;
      camera.top = d;
      camera.bottom = -d;
      camera.updateProjectionMatrix();
    }
    fit();
    const ro = new ResizeObserver(fit);
    ro.observe(mount);

    // Flat lighting — slabs/tiles get just enough shading to read as solid.
    scene.add(new THREE.AmbientLight(0xffffff, 0.85));
    const dir = new THREE.DirectionalLight(0xffffff, 0.25);
    dir.position.set(8, 14, 6);
    scene.add(dir);

    scene.add(new THREE.GridHelper(40, 40, palette.grid, palette.grid));

    // Stations + flow line are dynamic too: they're rebuilt every snapshot
    // because the pipeline lives in the DB and may change between ticks.
    const SLAB_W = 4.2;
    const SLAB_H = 0.12;
    const SLAB_D = 3.6;

    const dynGroup = new THREE.Group();
    scene.add(dynGroup);

    function clearDynamic() {
      while (dynGroup.children.length) {
        const c = dynGroup.children[0];
        dynGroup.remove(c);
        c.traverse?.((o: THREE.Object3D) => {
          const m = o as THREE.Mesh;
          if (m.geometry) m.geometry.dispose?.();
          const mat = m.material;
          if (Array.isArray(mat)) mat.forEach((mm) => mm.dispose());
          else mat?.dispose?.();
        });
      }
    }

    function rebuildFromSnapshot(s: Snapshot) {
      clearDynamic();

      // Stations — derived from the pipeline config block.
      const pipeline = s.config?.pipeline ?? [];
      const stations = buildStations(pipeline);

      // Pipeline flow line connecting station centers.
      if (stations.length >= 2) {
        const FIRST_X = stations[0].x;
        const LAST_X = stations[stations.length - 1].x;
        const flow = new THREE.Mesh(
          new THREE.BoxGeometry(LAST_X - FIRST_X, 0.012, 0.06),
          new THREE.MeshBasicMaterial({ color: palette.flowLine }),
        );
        flow.position.set((FIRST_X + LAST_X) / 2, 0.006, 0);
        dynGroup.add(flow);
      }

      // Station slabs — low rectangle + perimeter edge + labels.
      const stationEdgeMat = new THREE.MeshBasicMaterial({ color: palette.stationEdge });
      stations.forEach((st) => {
        const g = new THREE.Group();
        g.position.set(st.x, 0, 0);

        const slab = new THREE.Mesh(
          new THREE.BoxGeometry(SLAB_W, SLAB_H, SLAB_D),
          new THREE.MeshStandardMaterial({
            color: palette.stationBase,
            roughness: 0.85,
            metalness: 0.05,
          }),
        );
        slab.position.y = SLAB_H / 2;
        g.add(slab);

        const eY = SLAB_H + 0.005;
        const eT = 0.025;
        const eH = 0.008;
        const front = new THREE.Mesh(new THREE.BoxGeometry(SLAB_W, eH, eT), stationEdgeMat);
        front.position.set(0, eY, SLAB_D / 2);
        g.add(front);
        const back = new THREE.Mesh(new THREE.BoxGeometry(SLAB_W, eH, eT), stationEdgeMat);
        back.position.set(0, eY, -SLAB_D / 2);
        g.add(back);
        const leftEdge = new THREE.Mesh(new THREE.BoxGeometry(eT, eH, SLAB_D), stationEdgeMat);
        leftEdge.position.set(-SLAB_W / 2, eY, 0);
        g.add(leftEdge);
        const rightEdge = new THREE.Mesh(new THREE.BoxGeometry(eT, eH, SLAB_D), stationEdgeMat);
        rightEdge.position.set(SLAB_W / 2, eY, 0);
        g.add(rightEdge);

        const codeLbl = makeTextSprite(st.code, palette.textCode, 28, 700);
        codeLbl.position.set(0, 1.4, 2.05);
        g.add(codeLbl);
        const nameLbl = makeTextSprite(st.name, palette.textName, 56, 700);
        nameLbl.position.set(0, 0.85, 2.05);
        g.add(nameLbl);

        dynGroup.add(g);
      });

      // Project rail — derive from this workspace's config.
      const projectNames = (s.config?.projects ?? [])
        .map((p) => p?.name)
        .filter((n): n is string => typeof n === "string" && n.length > 0);
      const projects = layoutProjects(projectNames);
      const projectByName = new Map(projects.map((p) => [p.name, p]));

      const plaqueEdgeMat = new THREE.MeshBasicMaterial({ color: palette.stationEdge });
      for (const p of projects) {
        const g = new THREE.Group();
        g.position.set(p.x, 0, PROJECT_Z);

        const body = new THREE.Mesh(
          new THREE.BoxGeometry(PLAQUE_W, PLAQUE_H, PLAQUE_D),
          new THREE.MeshStandardMaterial({
            color: palette.stationBase,
            roughness: 0.85,
            metalness: 0.05,
          }),
        );
        body.position.y = PLAQUE_H / 2;
        g.add(body);

        const eY = PLAQUE_H + 0.005;
        const eT = 0.025;
        const eH = 0.008;
        const front = new THREE.Mesh(new THREE.BoxGeometry(PLAQUE_W, eH, eT), plaqueEdgeMat);
        front.position.set(0, eY, PLAQUE_D / 2);
        g.add(front);
        const back = new THREE.Mesh(new THREE.BoxGeometry(PLAQUE_W, eH, eT), plaqueEdgeMat);
        back.position.set(0, eY, -PLAQUE_D / 2);
        g.add(back);
        const leftEdge = new THREE.Mesh(new THREE.BoxGeometry(eT, eH, PLAQUE_D), plaqueEdgeMat);
        leftEdge.position.set(-PLAQUE_W / 2, eY, 0);
        g.add(leftEdge);
        const rightEdge = new THREE.Mesh(new THREE.BoxGeometry(eT, eH, PLAQUE_D), plaqueEdgeMat);
        rightEdge.position.set(PLAQUE_W / 2, eY, 0);
        g.add(rightEdge);

        const code = makeTextSprite(p.code, palette.textCode, 24, 700);
        code.position.set(0, 1.05, 0);
        g.add(code);
        const nameLbl = makeTextSprite(p.name, palette.textMeta, 38, 700);
        nameLbl.position.set(0, 0.55, 0);
        g.add(nameLbl);

        dynGroup.add(g);
      }

      const roadmapByTask = new Map<string, RoadmapItem>();
      s.roadmap.forEach((r) => roadmapByTask.set(r.task, r));

      // Resolve every agent to a task id once (handles workers that ship
      // only --spec/--branch). Longest ids first inside the helper.
      const sortedTaskIds = s.tasks.map((t) => t.id).sort((x, y) => y.length - x.length);
      const agentTaskId = new Map<number, string | null>();
      for (const a of s.agents) {
        agentTaskId.set(a.pid, taskIdForAgent(a, sortedTaskIds));
      }

      const tasksWithAgent = new Set<string>();
      for (const a of s.agents) {
        const tid = agentTaskId.get(a.pid);
        if (tid) tasksWithAgent.add(tid);
      }

      const byStation = new Map<
        string,
        { task: Task; status: string | null; state: TaskState }[]
      >();
      // Task id → display name (external_id like "ENG-1234" when present,
      // otherwise the title). Used to label agent tiles by their task.
      const taskNiceName = new Map<string, string>();
      for (const t of s.tasks) {
        taskNiceName.set(t.id, t.external_id ?? t.title);
        const { station, status } = stationForTask(t.id, roadmapByTask, stations);
        const arr = byStation.get(station.key) ?? [];
        arr.push({ task: t, status, state: taskState(t, status, pipeline) });
        byStation.set(station.key, arr);
      }

      // Agent → station: follow the agent's task (if any) to its roadmap
      // status, then to the station that owns that status. Agents with no
      // task (e.g. jira-scan, project-manager) dock at the leftmost station.
      const agentsByStation = new Map<string, Agent[]>();
      const FALLBACK_STATION = stations[0];
      for (const a of s.agents) {
        let stationKey: string = FALLBACK_STATION?.key ?? INBOX_KEY;
        const tid = agentTaskId.get(a.pid);
        if (tid) {
          const r = roadmapByTask.get(tid);
          if (r) {
            const st = stations.find((x) => x.statuses.includes(r.status));
            if (st) stationKey = st.key;
          }
        }
        const arr = agentsByStation.get(stationKey) ?? [];
        arr.push(a);
        agentsByStation.set(stationKey, arr);
      }

      const COLS = 5;
      const ROWS = 5;
      const MAX_VISIBLE = COLS * ROWS;
      const TILE_W = 0.36;
      const TILE_H = 0.05;
      const COL_S = 0.62;
      const ROW_S = 0.55;
      const TILE_BASE_Y = SLAB_H + 0.005;
      const TILE_BASE_Z = -1.2;

      // Filled as tasks render so agents drawn later can wire back to them.
      const taskPos = new Map<string, { px: number; py: number; pz: number }>();

      for (const station of stations) {
        const tasks = byStation.get(station.key) ?? [];
        const agents = agentsByStation.get(station.key) ?? [];

        // Front-edge accent — one severity wins, in priority order.
        let accent: number | null = null;
        if (tasks.some((t) => t.state === "blocked")) accent = palette.taskBlocked;
        else if (tasks.some((t) => t.state === "rework")) accent = palette.taskRework;
        else if (agents.length > 0) accent = palette.stationLive;

        if (accent !== null) {
          const accentMesh = new THREE.Mesh(
            new THREE.BoxGeometry(SLAB_W, 0.018, 0.06),
            new THREE.MeshBasicMaterial({ color: accent }),
          );
          accentMesh.position.set(station.x, SLAB_H + 0.018, SLAB_D / 2);
          dynGroup.add(accentMesh);
        }

        // Tasks — flat tiles in a 5x5 grid on the slab top.
        const visible = tasks.slice(0, MAX_VISIBLE);
        visible.forEach((entry, i) => {
          const col = i % COLS;
          const row = Math.floor(i / COLS);
          const px = station.x + (col - (COLS - 1) / 2) * COL_S;
          const py = TILE_BASE_Y + TILE_H / 2;
          const pz = TILE_BASE_Z + row * ROW_S;

          const tile = new THREE.Mesh(
            new THREE.BoxGeometry(TILE_W, TILE_H, TILE_W),
            new THREE.MeshBasicMaterial({ color: taskColor(palette, entry.state) }),
          );
          tile.position.set(px, py, pz);
          dynGroup.add(tile);
          taskPos.set(entry.task.id, { px, py, pz });

          // "Agent on it" marker — a thin vertical pin above the tile.
          if (tasksWithAgent.has(entry.task.id)) {
            const pinH = 0.4;
            const pin = new THREE.Mesh(
              new THREE.BoxGeometry(0.025, pinH, 0.025),
              new THREE.MeshBasicMaterial({ color: palette.agentPin }),
            );
            pin.position.set(px, py + TILE_H / 2 + pinH / 2, pz);
            dynGroup.add(pin);
          }

          // Wires to each project this task has worktrees in.
          const touched = entry.task.projects_touched ?? [];
          for (const projName of touched) {
            const proj = projectByName.get(projName);
            if (!proj) continue;
            const RISE_Y = 0.42;
            const points = [
              new THREE.Vector3(px, py + TILE_H / 2 + 0.005, pz),
              new THREE.Vector3(px, RISE_Y, pz),
              new THREE.Vector3(proj.x, RISE_Y, PROJECT_Z + PLAQUE_D / 2),
              new THREE.Vector3(proj.x, PLAQUE_H + 0.005, PROJECT_Z + PLAQUE_D / 2),
            ];
            const geom = new THREE.BufferGeometry().setFromPoints(points);
            const wire = new THREE.Line(
              geom,
              new THREE.LineBasicMaterial({
                color: palette.agentEdge,
                transparent: true,
                opacity: tasksWithAgent.has(entry.task.id) ? 0.7 : 0.35,
              }),
            );
            dynGroup.add(wire);
          }
        });

        // Count label sits under the station name, in front of the slab.
        if (tasks.length > 0) {
          const overflow = tasks.length > MAX_VISIBLE ? ` +${tasks.length - MAX_VISIBLE}` : "";
          const txt = `${String(tasks.length).padStart(2, "0")}${overflow}`;
          const lbl = makeTextSprite(txt, palette.textMeta, 36, 700);
          lbl.position.set(station.x, 0.35, 2.05);
          dynGroup.add(lbl);
        }

        // Agents — flat tiles in front of the slab. Single accent stripe.
        if (agents.length > 0) {
          const A_W = 0.95;
          const A_H = 0.08;
          const A_D = 0.55;
          const A_SPACING = 1.05;
          const A_Z = 3.7;
          agents.forEach((agent, i) => {
            const offset = (i - (agents.length - 1) / 2) * A_SPACING;
            const px = station.x + offset;

            const tile = new THREE.Mesh(
              new THREE.BoxGeometry(A_W, A_H, A_D),
              new THREE.MeshStandardMaterial({
                color: palette.agentTile,
                roughness: 0.85,
                metalness: 0.05,
              }),
            );
            tile.position.set(px, A_H / 2, A_Z);
            dynGroup.add(tile);

            const stripe = new THREE.Mesh(
              new THREE.BoxGeometry(A_W, 0.018, 0.03),
              new THREE.MeshBasicMaterial({ color: palette.agentEdge }),
            );
            stripe.position.set(px, A_H + 0.005, A_Z + A_D / 2 - 0.018);
            dynGroup.add(stripe);

            const roleLbl = makeTextSprite(
              shortLabelForAgent(agent.name),
              palette.textMeta,
              26,
              700,
            );
            roleLbl.position.set(px, A_H + 0.22, A_Z);
            dynGroup.add(roleLbl);

            const taskLabel = agent.task ? (taskNiceName.get(agent.task) ?? "-") : "-";
            const tkt = makeTextSprite(taskLabel, palette.textPrimary, 36, 700);
            tkt.position.set(px, A_H + 0.55, A_Z);
            dynGroup.add(tkt);

            const e = elapsedShort(agent.started_at);
            if (e) {
              const eLbl = makeTextSprite(e, palette.textSubtle, 22, 600);
              eLbl.position.set(px, A_H + 0.85, A_Z);
              dynGroup.add(eLbl);
            }

            // Wire agent → task. Mirrors the task→project bend: rise
            // from the front edge of the agent tile, jog over the slab to
            // the task's column, then drop onto the task's tile.
            const tid = agentTaskId.get(agent.pid);
            const tpos = tid ? taskPos.get(tid) : null;
            if (tpos) {
              const RISE_Y = 0.42;
              const points = [
                new THREE.Vector3(px, A_H + 0.005, A_Z - A_D / 2),
                new THREE.Vector3(px, RISE_Y, A_Z - A_D / 2),
                new THREE.Vector3(tpos.px, RISE_Y, tpos.pz),
                new THREE.Vector3(tpos.px, tpos.py + TILE_H / 2 + 0.005, tpos.pz),
              ];
              const geom = new THREE.BufferGeometry().setFromPoints(points);
              const wire = new THREE.Line(
                geom,
                new THREE.LineBasicMaterial({
                  color: palette.agentEdge,
                  transparent: true,
                  opacity: 0.7,
                }),
              );
              dynGroup.add(wire);
            }
          });
        }
      }
    }

    let raf = 0;
    function tick() {
      renderer.render(scene, camera);
      raf = requestAnimationFrame(tick);
    }
    raf = requestAnimationFrame(tick);

    // Theme rebuilds tear the scene down and recreate it; seed with the
    // most recent snapshot so the user sees data immediately rather than
    // waiting for the next SSE tick.
    rebuildFromSnapshot(snapRef.current ?? { tasks: [], roadmap: [], agents: [], ts: "" });
    (mount as HTMLDivElement & { __rebuild?: (s: Snapshot) => void }).__rebuild =
      rebuildFromSnapshot;

    return () => {
      cancelAnimationFrame(raf);
      ro.disconnect();
      renderer.dispose();
      scene.traverse((obj) => {
        const m = obj as THREE.Mesh;
        if (m.geometry) m.geometry.dispose?.();
        const mat = m.material;
        if (Array.isArray(mat)) mat.forEach((mm) => mm.dispose());
        else mat?.dispose?.();
      });
      if (mount.contains(canvas)) mount.removeChild(canvas);
    };
  }, [theme]);

  useEffect(() => {
    if (!snap) return;
    const m = mountRef.current as (HTMLDivElement & { __rebuild?: (s: Snapshot) => void }) | null;
    m?.__rebuild?.(snap);
  }, [snap]);

  return (
    <main className="flex-1 relative overflow-hidden bg-bg">
      <div ref={mountRef} className="absolute inset-0" />
      <HUD snap={snap} theme={theme} />
    </main>
  );
}

function HUD({ snap, theme }: { snap: Snapshot | null; theme: Theme }) {
  const palette = PALETTES[theme];
  const tasks = snap?.tasks ?? [];
  const roadmap = snap?.roadmap ?? [];
  const agents = snap?.agents ?? [];
  const pipeline = snap?.config?.pipeline ?? [];
  // Derive status categories from the pipeline config rather than hardcoding
  // names. `error` flags appear on blocked-style states; `terminal` flags
  // appear on accepted-style states.
  const errorKeys = new Set(pipeline.filter((p) => p.error).map((p) => p.key));
  const terminalKeys = new Set(pipeline.filter((p) => p.terminal).map((p) => p.key));
  const pendingApproval = tasks.filter((t) => t.approved === null).length;
  const blocked = roadmap.filter((r) => errorKeys.has(r.status)).length;
  const accepted = tasks.filter((t) =>
    (t.variants ?? []).some((v) => v.status === "accepted"),
  ).length;
  const inFlight = roadmap.filter(
    (r) => !terminalKeys.has(r.status) && !errorKeys.has(r.status),
  ).length;
  const reworkCount = tasks.reduce(
    (acc, t) => acc + (t.variants ?? []).filter((v) => v.status === "rework").length,
    0,
  );
  const totalAgents = agents.length;
  const totalTasks = tasks.length;
  // Live counts per agent name, sorted descending by count then alpha.
  // Driven entirely by what's currently running, so new agents show up
  // automatically without code changes.
  const byNameMap = new Map<string, number>();
  for (const a of agents) {
    byNameMap.set(a.name, (byNameMap.get(a.name) ?? 0) + 1);
  }
  const byName = [...byNameMap.entries()].sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]));

  return (
    <>
      <div className="absolute top-3 left-5 flex items-center gap-4 font-mono text-[10px] uppercase tracking-[0.18em] pointer-events-none">
        <InlineStat
          label="Pending Approval"
          value={pendingApproval}
          accentColor={pendingApproval > 0 ? palette.hudWarn : undefined}
        />
        <span className="text-fg-subtle">│</span>
        <InlineStat
          label="Blocked"
          value={blocked}
          accentColor={blocked > 0 ? palette.hudErr : undefined}
        />
        <span className="text-fg-subtle">│</span>
        <InlineStat
          label="Rework"
          value={reworkCount}
          accentColor={reworkCount > 0 ? palette.hudWarn : undefined}
        />
        <span className="text-fg-subtle">│</span>
        <InlineStat label="In Flight" value={inFlight} />
      </div>

      <div className="absolute top-3 right-5 flex items-center gap-4 font-mono text-[10px] uppercase tracking-[0.18em] pointer-events-none">
        <InlineStat
          label="Agents"
          value={totalAgents}
          accentColor={totalAgents > 0 ? palette.hudLive : undefined}
        />
        <span className="text-fg-subtle">│</span>
        <InlineStat label="Tasks" value={totalTasks} />
        <span className="text-fg-subtle">│</span>
        <InlineStat label="Accepted" value={accepted} />
      </div>

      <div className="absolute bottom-0 inset-x-0 px-5 py-2.5 flex items-center justify-between border-t border-border/70 bg-bg/40 backdrop-blur-sm pointer-events-none">
        <div className="flex items-center gap-2 font-mono text-[11px] flex-wrap">
          <span className="text-fg-subtle uppercase tracking-[0.18em] mr-2">AGENTS RUNNING</span>
          {byName.length === 0 ? (
            <span className="px-2.5 py-1 border border-border text-fg-subtle tabular-nums">
              none
            </span>
          ) : (
            byName.map(([name, count]) => (
              <span
                key={name}
                title={name}
                className="px-2.5 py-1 border border-border flex items-center gap-2 tabular-nums"
              >
                <span className="text-fg-subtle uppercase tracking-wider">
                  {shortLabelForAgent(name)}
                </span>
                <span
                  className={count > 0 ? "" : "text-fg-subtle"}
                  style={count > 0 ? { color: palette.hudLive } : undefined}
                >
                  {String(count).padStart(2, "0")}
                </span>
              </span>
            ))
          )}
        </div>
        <div className="flex items-center gap-3 font-mono text-[10px] uppercase tracking-wider">
          <span className="text-fg-subtle mr-1">STATE</span>
          <Swatch color={palette.taskHex.pending} label="pending" />
          <Swatch color={palette.taskHex.approved} label="approved" />
          <Swatch color={palette.taskHex.rework} label="rework" />
          <Swatch color={palette.taskHex.blocked} label="blocked" />
          <Swatch color={palette.taskHex.accepted} label="accepted" />
          <span className="text-fg-subtle">│</span>
          <span className="flex items-center gap-1.5">
            <span
              className="inline-block w-[2px] h-3"
              style={{ backgroundColor: palette.hudLive }}
            />
            <span className="text-fg-subtle">agent on it</span>
          </span>
        </div>
      </div>
    </>
  );
}

function InlineStat({
  label,
  value,
  accentColor,
}: {
  label: string;
  value: number;
  accentColor?: string;
}) {
  return (
    <span className="flex items-baseline gap-1.5">
      <span className="text-fg-subtle">{label}</span>
      <span
        className="text-[14px] font-bold tabular-nums tracking-normal text-fg"
        style={accentColor ? { color: accentColor } : undefined}
      >
        {String(value).padStart(2, "0")}
      </span>
    </span>
  );
}

function Swatch({ color, label }: { color: string; label: string }) {
  return (
    <span className="flex items-center gap-1.5">
      <span className="inline-block w-2.5 h-2.5" style={{ backgroundColor: color }} />
      <span className="text-fg-subtle">{label}</span>
    </span>
  );
}
