// Thin client for the local Rust engine. No state lives here.

export type BoundaryRef = {
  panel: string;
  side: string;
  edge?: string;
  range: [number, number];
};

export type Panel = {
  id: string;
  name: string;
  fabric: string;
  grain_degrees: number;
  boundaries: string[];
  measurements: unknown[];
  target_lengths: Record<string, number>;
  seam_allowances: Record<string, number>;
  pins: unknown[];
  layout_2d?: [number, number];
};

export type Seam = {
  id: string;
  a: BoundaryRef;
  b: BoundaryRef;
  reverse_b: boolean;
  allow_ease: boolean;
  stiffness: number;
};

export type DocumentSummary = {
  documentSession: string | null;
  revision: number | null;
  canUndo: boolean | null;
  canRedo: boolean | null;
  pendingChange: string | null;
  document: {
    name: string;
    units: string;
    fabrics: { id: string; name: string }[];
    panels: Panel[];
    seams: Seam[];
    counts: { panels: number; seams: number };
  } | null;
};

export type PreviewPath = {
  panel: string;
  closed: boolean;
  internal: boolean;
  points: [number, number][];
};

export type Preview = {
  units: "metres";
  bounds: { min: [number, number]; max: [number, number] };
  paths: PreviewPath[];
  warnings: string[];
  panelCount: number;
  seamCount: number;
};

export type ImportContour = {
  index: number;
  piece: string | null;
  layer: string;
  closed: boolean;
  sourceNodes: number;
  fittedNodes: number | null;
  curvedEdges: number;
  eligible: boolean;
  reason: string | null;
  bounds: { min: [number, number]; max: [number, number] } | null;
};

export type ImportPreview = {
  units: { insunits: number | null; metresPerUnit: number; overridden: boolean };
  sampleSize: string | null;
  unitRecord: string | null;
  contours: ImportContour[];
  leftOut: { entity: string; layer: string; reason: string }[];
  leftOutTotal: number;
};

export type BoardEdge = {
  panel: string;
  name: string;
  points: [number, number][];
  length_m: number;
};

export type EdgeList = { units: "metres"; edges: BoardEdge[] };

export type AvatarEntry = {
  id: string;
  title: string;
  kind: "mannequin" | "dress_form";
  available: boolean;
  reason: string | null;
  height_m: number | null;
  shoulder_height_m: number | null;
  dimensions_cm: Record<string, number>;
  source: string;
  license: string;
};

export type AvatarList = { selected: string | null; avatars: AvatarEntry[] };

export type AvatarMesh = {
  id: string;
  units: "metres";
  shoulder_height_m: number | null;
  positions: [number, number, number][];
  triangles: [number, number, number][];
};

export type DrapeSnapshot = {
  solver: string;
  claim: string;
  avatar?: string | null;
  garment_lift_m?: number;
  time_s: number;
  dt: number;
  stepsDone?: number;
  solverError?: string;
  positions: [number, number, number][];
  panels: { panel: string; triangles: [number, number, number][] }[];
};

export type CatalogEntry = {
  id: string;
  title: string;
  description: string;
  license: string;
};

async function request<T>(url: string, init?: RequestInit): Promise<T> {
  const response = await fetch(url, init);
  const body = await response.json();
  if (!response.ok) {
    const detail = body?.error?.detail ?? response.statusText;
    throw new Error(detail);
  }
  return body as T;
}

export const api = {
  catalog: () => request<{ templates: CatalogEntry[] }>("/api/catalog"),
  document: () => request<DocumentSummary>("/api/document"),
  preview: () => request<Preview>("/api/preview"),
  open: (catalog: string) =>
    request<DocumentSummary>("/api/open", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ catalog }),
    }),
  importPreview: (path: string, metresPerUnit?: number) =>
    request<ImportPreview>("/api/import/dxf/preview", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ path, metres_per_unit: metresPerUnit }),
    }),
  importApply: (path: string, selected: number[], fabric: string, metresPerUnit?: number) =>
    request<DocumentSummary>("/api/import/dxf/apply", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ path, selected, fabric, metres_per_unit: metresPerUnit }),
    }),
  edges: () => request<EdgeList>("/api/edges"),
  command: (command: Record<string, unknown>) =>
    request<DocumentSummary>("/api/command", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ command }),
    }),
  uploadDxf: async (file: File): Promise<string> => {
    // The engine reads bounded local files by path; the dev server stores the
    // upload under .uploads and hands back an absolute path.
    const form = new FormData();
    form.append("file", file);
    const response = await fetch("/api/upload", { method: "POST", body: form });
    if (!response.ok) throw new Error("upload failed");
    const body = (await response.json()) as { path: string };
    return body.path;
  },
  simStep: (steps: number) =>
    request<DrapeSnapshot>("/api/sim/step", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ steps }),
    }),
  avatars: () => request<AvatarList>("/api/avatars"),
  selectAvatar: (id: string | null) =>
    request<{ selected: string | null }>("/api/avatar", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ id }),
    }),
  avatarMesh: () => request<AvatarMesh>("/api/avatar/mesh"),
  download: async (kind: "svg" | "print-svg") => {
    const response = await fetch("/api/export/" + kind);
    if (!response.ok) throw new Error("export failed");
    return response.text();
  },
  undo: () => request<DocumentSummary>("/api/undo", { method: "POST" }),
  redo: () => request<DocumentSummary>("/api/redo", { method: "POST" }),
};
