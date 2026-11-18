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
};

export type ImportPreview = {
  units: { insunits: number | null; metresPerUnit: number; overridden: boolean };
  sampleSize: string | null;
  unitRecord: string | null;
  contours: ImportContour[];
  leftOut: { entity: string; layer: string; reason: string }[];
  leftOutTotal: number;
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
  undo: () => request<DocumentSummary>("/api/undo", { method: "POST" }),
  redo: () => request<DocumentSummary>("/api/redo", { method: "POST" }),
};
