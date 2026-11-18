import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  type FlatGuide,
  api,
  type BoardEdge,
  type CatalogEntry,
  type ImportContour,
  type DocumentSummary,
  type Panel,
  type AvatarList,
  type AvatarMesh,
  type DrapeSnapshot,
  type Preview,
} from "./api";
import { DrapeView } from "./DrapeView";
import { ImportPanel } from "./ImportPanel";

const PANEL_COLOURS = ["#c8452b", "#2f6f6a", "#8a6b2f", "#4a5b8c", "#7a4a6a", "#3f6b3f"];

type Selection = { panel: string | null; edge: BoardEdge | null };

export default function App() {
  const [catalog, setCatalog] = useState<CatalogEntry[]>([]);
  const [summary, setSummary] = useState<DocumentSummary | null>(null);
  const [preview, setPreview] = useState<Preview | null>(null);
  const [selection, setSelection] = useState<Selection>({ panel: null, edge: null });
 const [edges, setEdges] = useState<BoardEdge[]>([]);
  const [ghosts, setGhosts] = useState<ImportContour[]>([]);
  const [drape, setDrape] = useState<DrapeSnapshot | null>(null);
  const [avatars, setAvatars] = useState<AvatarList | null>(null);
  const [avatarMesh, setAvatarMesh] = useState<AvatarMesh | null>(null);
  // The form's flat guide, drawn behind the pieces to align them to the form.
  const [guide, setGuide] = useState<FlatGuide | null>(null);
  const [guideOn, setGuideOn] = useState(true);
  const [guideFace, setGuideFace] = useState<"front" | "back">("front");
  const [guideCf, setGuideCf] = useState<"left" | "centre" | "right">("centre");
  // A bodice's side seam usually sits behind the form's side: 50 mm matches the sloper the guide was tested against.
  const [guideSide, setGuideSide] = useState(50);
  const [guideNudge, setGuideNudge] = useState({ x: 0, y: 0 });
  // Which piece the guide is aligned to ("" = the first one called front/back), and the guide line pinned to the piece's top.
  const [guidePiece, setGuidePiece] = useState("");
  const [guideAnchor, setGuideAnchor] = useState<"auto" | "neck" | "waist">("auto");
  const [playing, setPlaying] = useState(false);
  const playRef = useRef(false);
  const stepBusy = useRef(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    const [document, drawing, boundaryList] = await Promise.all([
      api.document(),
      api.preview(),
      api.edges(),
    ]);
    setSummary(document);
    setPreview(drawing);
    setEdges(boundaryList.edges);
    // Any document change invalidates a draped preview; the engine rebuilds on
    // the next step.
    setDrape(null);
  }, []);

  useEffect(() => {
    playRef.current = playing;
  }, [playing]);

  useEffect(() => {
    if (!playing) return;
    let cancelled = false;
    const tick = async () => {
      while (!cancelled && playRef.current) {
        if (!stepBusy.current) {
          stepBusy.current = true;
          try {
            const snapshot = await api.simStep(8);
            if (!cancelled) setDrape(snapshot);
          } catch {
            if (!cancelled) setPlaying(false);
          } finally {
            stepBusy.current = false;
          }
        }
        await new Promise((resolve) => setTimeout(resolve, 33));
      }
    };
    void tick();
    return () => {
      cancelled = true;
    };
  }, [playing]);

  // The body the preview drapes on. The server reads and checks the mesh when it is
  // chosen, so a unit error shows up here and not at the first step.
  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const list = await api.avatars();
        if (cancelled) return;
        setAvatars(list);
        if (list.selected) setAvatarMesh(await api.avatarMesh());
      } catch {
        /* an older server has no avatar routes; the picker simply stays hidden */
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    let cancelled = false;
    if (!avatars?.selected) {
      setGuide(null);
      return;
    }
    const timer = window.setTimeout(async () => {
      try {
        const reply = await api.avatarGuide(guideSide);
        if (!cancelled) setGuide(reply.guide);
      } catch {
        if (!cancelled) setGuide(null); // a form without landmarks has no guide
      }
    }, 150);
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [avatars?.selected, guideSide]);

  const chooseAvatar = useCallback(async (id: string | null) => {
    setPlaying(false);
    try {
      await api.selectAvatar(id);
      setDrape(null);
      setAvatarMesh(id ? await api.avatarMesh() : null);
      setAvatars(await api.avatars());
    } catch (problem) {
      setError(problem instanceof Error ? problem.message : String(problem));
    }
  }, []);

  const stepOnce = useCallback(async () => {
    setPlaying(false);
    try {
      setDrape(await api.simStep(24));
    } catch (problem) {
      setError(problem instanceof Error ? problem.message : String(problem));
    }
  }, []);

  // The garment variable shadows the DOM global inside this component, so the
  // browser object is reached through globalThis here.
  const downloadSvg = useCallback(
    async (kind: "svg" | "print-svg") => {
      try {
        const text = await api.download(kind);
        const dom = globalThis.document;
        const url = URL.createObjectURL(new Blob([text], { type: "image/svg+xml" }));
        const link = dom.createElement("a");
        link.href = url;
        link.download =
          (summary?.document?.name ?? "garment") +
          "-" +
          (kind === "svg" ? "preview" : "print") +
          ".svg";
        link.click();
        URL.revokeObjectURL(url);
      } catch (problem) {
        setError(problem instanceof Error ? problem.message : String(problem));
      }
    },
    [summary],
  );

  const open = useCallback(
    async (id: string) => {
      setBusy(true);
      setError(null);
      try {
        await api.open(id);
        setGhosts([]);
        await refresh();
        setSelection({ panel: null, edge: null });
      } catch (problem) {
        setError(problem instanceof Error ? problem.message : String(problem));
      } finally {
        setBusy(false);
      }
    },
    [refresh],
  );

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const { templates } = await api.catalog();
        if (cancelled) return;
        setCatalog(templates);
        const first = templates.find((entry) => entry.id === "tshirt") ?? templates[0];
        if (first) await open(first.id);
      } catch (problem) {
        if (!cancelled) {
          setError(
            problem instanceof Error
              ? `${problem.message} — is cloth-server running?`
              : String(problem),
          );
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [open]);

  const history = useCallback(
    async (action: "undo" | "redo") => {
      setBusy(true);
      try {
        action === "undo" ? await api.undo() : await api.redo();
        await refresh();
      } catch (problem) {
        setError(problem instanceof Error ? problem.message : String(problem));
      } finally {
        setBusy(false);
      }
    },
    [refresh],
  );

  const document = summary?.document ?? null;
  const panelColour = useMemo(() => {
    const map = new Map<string, string>();
    document?.panels.forEach((panel, index) => {
      map.set(panel.id, PANEL_COLOURS[index % PANEL_COLOURS.length]);
    });
    return map;
  }, [document]);

  return (
    <div className="studio">
      <header className="top-bar">
        <div className="brand">
          <span className="brand-mark" aria-hidden="true" />
          <span className="brand-name">FoldCraft</span>
          <span className="brand-stage">Studio</span>
        </div>

        <label className="template-picker">
          <span className="label">Garment</span>
          <select
            value={document ? undefined : ""}
            disabled={busy || catalog.length === 0}
            onChange={(event) => open(event.target.value)}
          >
            {catalog.map((entry) => (
              <option key={entry.id} value={entry.id}>
                {entry.title}
              </option>
            ))}
          </select>
        </label>

        <div className="transport">
          <button type="button" disabled={busy || !summary?.canUndo} onClick={() => history("undo")}>
            Undo
          </button>
          <button type="button" disabled={busy || !summary?.canRedo} onClick={() => history("redo")}>
            Redo
          </button>
          <button type="button" disabled={busy || !document} onClick={() => downloadSvg("svg")}>
            SVG
          </button>
          <button
            type="button"
            disabled={busy || !document}
            title="1:1 millimetre sheet with a 100 mm ruler; untiled, not cutting-certified"
            onClick={() => downloadSvg("print-svg")}
          >
            Print 1:1
          </button>
        </div>

        <div className="transport drape-transport" title="CPU preview, not a fit">
          <button type="button" onClick={() => setPlaying((value) => !value)}>
            {playing ? "Pause" : "Play"}
          </button>
          <button type="button" onClick={stepOnce}>Step</button>
          {avatars ? (
            <select
              aria-label="Body"
              className="drape-body"
              value={avatars.selected ?? ""}
              onChange={(event) => void chooseAvatar(event.target.value || null)}
              title="The body the garment drapes on"
            >
              <option value="">no body</option>
              {avatars.avatars.map((entry) => (
                <option key={entry.id} value={entry.id} disabled={!entry.available} title={entry.reason ?? entry.source}>
                  {entry.title}
                  {entry.available ? "" : " (not installed)"}
                </option>
              ))}
            </select>
          ) : null}
          <span className="drape-label">{drape ? "preview · not a fit" : "no drape"}</span>
        </div>

        {guide ? (
          <div className="transport guide-controls" title="The form laid flat, behind the pattern">
            <label>
              <input type="checkbox" checked={guideOn} onChange={(event) => setGuideOn(event.target.checked)} /> Guide
            </label>
            <select aria-label="Guide face" value={guideFace} onChange={(event) => setGuideFace(event.target.value as "front" | "back")}>
              <option value="front">front</option>
              <option value="back">back</option>
            </select>
            <select aria-label="Align to piece" value={guidePiece} onChange={(event) => setGuidePiece(event.target.value)} title="The piece whose top and centre the guide is aligned to">
              <option value="">align to: first {guideFace}</option>
              {[...new Set((preview?.paths ?? []).filter((path) => path.closed && !path.internal).map((path) => path.panel))].map((id) => (
                <option key={id} value={id}>{id}</option>
              ))}
            </select>
            <select aria-label="Guide anchor" value={guideAnchor} onChange={(event) => setGuideAnchor(event.target.value as "auto" | "neck" | "waist")} title="Which line of the form sits at the piece's top: the high-point shoulder (a bodice) or the waist (a trouser or skirt)">
              <option value="auto">anchor: auto</option>
              <option value="neck">anchor: HPS</option>
              <option value="waist">anchor: waist</option>
            </select>
            <select aria-label="Centre line at" value={guideCf} onChange={(event) => setGuideCf(event.target.value as "left" | "centre" | "right")} title="Where the centre front (or back) of the form sits on the first matching piece">
              <option value="centre">centre at middle</option>
              <option value="left">centre at left edge</option>
              <option value="right">centre at right edge</option>
            </select>
            <label title="How far behind the form's side the side seam sits, in millimetres">
              side <input type="number" step="5" min="-100" max="100" value={guideSide} onChange={(event) => setGuideSide(Number(event.target.value) || 0)} />
            </label>
            <label title="Nudge the guide, millimetres (right, down)">
              nudge <input type="number" step="5" value={guideNudge.x} onChange={(event) => setGuideNudge((n) => ({ ...n, x: Number(event.target.value) || 0 }))} />
              <input type="number" step="5" value={guideNudge.y} onChange={(event) => setGuideNudge((n) => ({ ...n, y: Number(event.target.value) || 0 }))} />
            </label>
          </div>
        ) : null}

        <div className="revision" title="Document revision. Every change bumps it.">
          <span className="dot" data-live={Boolean(document)} />
          rev {summary?.revision ?? "—"}
        </div>
      </header>

      <div className="workspace">
        <nav className="tool-rail" aria-label="Tools">
          {[
            ["select", "Select", "V"],
            ["pen", "Pen", "P"],
            ["sew", "Sew", "S"],
            ["move", "Move", "M"],
            ["sim", "Simulate", "Space"],
          ].map(([id, label, key]) => (
            <button key={id} type="button" className="tool" title={`${label} (${key})`} disabled>
              <span className="tool-glyph" aria-hidden="true">
                {label.slice(0, 2)}
              </span>
              <span className="tool-label">{label}</span>
            </button>
          ))}
        </nav>

        <main className="board" data-split={drape || avatarMesh ? "true" : undefined}>
          {preview ? (
           <Board
              preview={preview}
              ghosts={ghosts.filter((c) => c.closed && c.eligible && c.bounds)}
              edges={edges}
              selection={selection}
              panelColour={panelColour}
              onSelect={(panel) => setSelection({ panel, edge: null })}
              onPickEdge={(edge) => setSelection({ panel: edge.panel, edge })}
              guide={guideOn ? guide : null}
              guideFace={guideFace}
              guideCf={guideCf}
              guideNudge={guideNudge}
              guidePiece={guidePiece}
              guideAnchor={guideAnchor}
            />
          ) : (
            <div className="board-empty">
              {busy ? "Opening…" : "Open a garment to start."}
            </div>
          )}
          {drape || avatarMesh ? (
            <div className="drape-pane">
              <DrapeView snapshot={drape} avatar={avatarMesh} />
              <span className="drape-caption">
                {drape ? (
                  <>
                    {drape.solver} · t {drape.time_s.toFixed(2)} s
                    {drape.garment_lift_m ? <> · garment lifted {(drape.garment_lift_m * 100).toFixed(0)} cm onto the shoulder line</> : null}
                    {" "}·{" "}
                    <strong>preview, not a fit</strong>
                  </>
                ) : (
                  <>{avatarMesh?.id} · body only, press Play or Step to drape</>
                )}
              </span>
            </div>
          ) : null}
        </main>

        <aside className="inspector" aria-label="Inspector">
          <ImportPanelHost
            document={document}
            onApplied={() => {
              setGhosts([]);
              refresh();
            }}
            onError={setError}
            onPreview={setGhosts}
          />
          <Inspector
            document={document}
            edges={edges}
            selection={selection}
            onSelect={(panel) => setSelection({ panel, edge: null })}
            onChanged={refresh}
            onError={setError}
            busy={busy}
            onBusy={setBusy}
          />
        </aside>
      </div>

      <footer className="status-bar">
        <span>{document ? document.name : "No document"}</span>
        <span className="spacer" />
        {preview?.warnings.map((warning) => (
          <span key={warning} className="warning">
            {warning}
          </span>
        ))}
        {error ? <span className="error">{error}</span> : null}
        <span className="units">{preview?.units ?? "metres"}</span>
      </footer>
    </div>
  );
}

function Board({
  preview,
  ghosts,
  edges,
  selection,
  panelColour,
  onSelect,
  onPickEdge,
  guide,
  guideFace,
  guideCf,
  guideNudge,
  guidePiece,
  guideAnchor,
}: {
  guidePiece: string;
  guideAnchor: "auto" | "neck" | "waist";
  guide: FlatGuide | null;
  guideFace: "front" | "back";
  guideCf: "left" | "centre" | "right";
  guideNudge: { x: number; y: number };
  preview: Preview;
  ghosts: ImportContour[];
  edges: BoardEdge[];
  selection: Selection;
  panelColour: Map<string, string>;
  onSelect: (panel: string) => void;
  onPickEdge: (edge: BoardEdge) => void;
}) {
  const [zoom, setZoom] = useState(1);
  const [pan, setPan] = useState({ x: 0, y: 0 });
  const drag = useRef<{ x: number; y: number } | null>(null);
  const svg = useRef<SVGSVGElement | null>(null);

  const fit = useMemo(() => {
    const [minX, minY] = preview.bounds.min;
    const [maxX, maxY] = preview.bounds.max;
    const pad = Math.max((maxX - minX) * 0.08, (maxY - minY) * 0.08, 0.02);
    return { x: minX - pad, y: minY - pad, w: maxX - minX + pad * 2, h: maxY - minY + pad * 2 };
  }, [preview]);

  const view = {
    x: fit.x + pan.x,
    y: fit.y + pan.y,
    w: fit.w / zoom,
    h: fit.h / zoom,
  };

  // The guide, mapped onto the board. Board: x right, y down, metres. Guide: x across (the form's left is +x, which is the viewer's right in a
  // front view), y height. The guide is anchored to the first piece called front (or back): its centre line at the piece's middle or an edge,
  // and the high-point shoulder (a bodice) or the waist (a trouser or skirt) at the piece's top.
  const layer = useMemo(() => {
    if (!guide) return null;
    const closed = preview.paths.filter((p) => p.closed && !p.internal);
    const pool = closed.filter((p) => (guideFace === "front" ? /front/i : /back/i).test(p.panel));
    const named = guidePiece ? closed.find((p) => p.panel === guidePiece) : undefined;
    const piece = named ?? (pool.length ? pool : closed)[0];
    if (!piece) return null;
    const xs = piece.points.map((q) => q[0]);
    const ys = piece.points.map((q) => q[1]);
    const [minX, maxX, minY] = [Math.min(...xs), Math.max(...xs), Math.min(...ys)];
    const cfx = (guideCf === "left" ? minX : guideCf === "right" ? maxX : (minX + maxX) / 2) + guideNudge.x / 1000;
    const waistKind = closed.some((p) => /trouser|pant|skirt/i.test(p.panel));
    const anchorName = guideAnchor === "auto" ? (waistKind ? "waist" : "neck") : guideAnchor;
    const anchor = guide.landmarks.find((l) => l.name === anchorName) ?? guide.landmarks[0];
    const top = minY + guideNudge.y / 1000;
    const X = (x: number) => cfx + x;
    const Y = (y: number) => top + (anchor.y - y);
    const poly = (pts: [number, number][]) => pts.map(([x, y]) => `${X(x)},${Y(y)}`).join(" ");
    const side = guideFace === "front" ? guide.front : guide.back;
    return {
      anchorName: anchor.name,
      torso: poly(side.outline),
      legs: guide.legs.map((l) => ({ side: l.side, points: poly(guideFace === "front" ? l.front : l.back) })),
      arms: guide.arms.map((a) => ({ name: a.name, points: poly(a.outline) })),
      centre: { x: X(0), y1: Y(side.centre[0][1]), y2: Y(guide.landmarks.find((l) => l.name === "floor")?.y ?? side.centre[1][1]) },
      lines: guide.landmarks.map((l) => {
        const half = (l.leg_front != null ? l.leg_front + 0.01 : (guideFace === "front" ? l.half_front : l.half_back)) + 0.03;
        return { name: l.name, y: Y(l.y), x1: X(-half), x2: X(half) };
      }),
      points: guide.points.map((p) => ({ name: p.name, x: X(p.x), y: Y(p.y) })),
    };
  }, [guide, guideFace, guideCf, guideNudge, guidePiece, guideAnchor, preview]);

  const gridStep = useMemo(() => {
    const target = view.w / 10;
    const steps = [0.01, 0.02, 0.05, 0.1, 0.2, 0.5, 1];
    return steps.find((step) => step >= target) ?? 1;
  }, [view.w]);

  const lines: { x1: number; y1: number; x2: number; y2: number }[] = [];
  const startX = Math.floor(view.x / gridStep) * gridStep;
  const startY = Math.floor(view.y / gridStep) * gridStep;
  for (let x = startX; x < view.x + view.w; x += gridStep) {
    lines.push({ x1: x, y1: view.y, x2: x, y2: view.y + view.h });
  }
  for (let y = startY; y < view.y + view.h; y += gridStep) {
    lines.push({ x1: view.x, y1: y, x2: view.x + view.w, y2: y });
  }

  return (
    <svg
      ref={svg}
      className="board-canvas"
      viewBox={`${view.x} ${view.y} ${view.w} ${view.h}`}
      onWheel={(event) => {
        event.preventDefault();
        const factor = Math.exp(-event.deltaY * 0.0015);
        setZoom((value) => Math.min(24, Math.max(0.25, value * factor)));
      }}
      onPointerDown={(event) => {
        drag.current = { x: event.clientX, y: event.clientY };
        event.currentTarget.setPointerCapture(event.pointerId);
      }}
      onPointerMove={(event) => {
        if (!drag.current || !svg.current) return;
        const rect = svg.current.getBoundingClientRect();
        const dx = (event.clientX - drag.current.x) * (view.w / rect.width);
        const dy = (event.clientY - drag.current.y) * (view.h / rect.height);
        setPan((value) => ({ x: value.x - dx, y: value.y - dy }));
        drag.current = { x: event.clientX, y: event.clientY };
      }}
      onPointerUp={() => {
        drag.current = null;
      }}
    >
      <g className="grid">
        {lines.map((line, index) => (
          <line key={index} {...line} />
        ))}
      </g>
      {layer ? (
        <g className="form-guide" data-face={guideFace}>
          {layer.arms.map((a) => (
            <polygon key={a.name} points={a.points} className="guide-arm" />
          ))}
          <polygon points={layer.torso} className="guide-body" />
          {layer.legs.map((l) => (
            <polygon key={l.side} points={l.points} className="guide-body" />
          ))}
          <line x1={layer.centre.x} y1={layer.centre.y1} x2={layer.centre.x} y2={layer.centre.y2} className="guide-centre" />
          {layer.lines.map((l) => (
            <g key={l.name}>
              <line x1={l.x1} y1={l.y} x2={l.x2} y2={l.y} className="guide-line" data-name={l.name} />
              <text x={l.x1} y={l.y - view.w / 300} fontSize={view.w / 120} className="guide-label">
                {l.name === "neck" ? "HPS / neck" : l.name[0].toUpperCase() + l.name.slice(1)}
              </text>
            </g>
          ))}
          {layer.points.map((p) => (
            <circle key={p.name} cx={p.x} cy={p.y} r={view.w / 400} className="guide-point">
              <title>{p.name.replace(/_/g, " ")}</title>
            </circle>
          ))}
        </g>
      ) : null}
      {preview.paths.map((path, index) => {
        const points = path.points.map(([x, y]) => `${x},${y}`).join(" ");
        const colour = panelColour.get(path.panel) ?? "#555";
        const selected = selection.panel === path.panel;
        return path.closed ? (
          <polygon
            key={`${path.panel}-${index}`}
            points={points}
            className="piece"
            data-selected={selected}
            data-internal={path.internal}
            style={{ "--piece": colour } as React.CSSProperties}
            onPointerDown={(event) => {
              event.stopPropagation();
              onSelect(path.panel);
            }}
          />
        ) : (
          <polyline
            key={`${path.panel}-${index}`}
            points={points}
            className="piece internal"
            style={{ "--piece": colour } as React.CSSProperties}
          />
        );
     })}
      {ghosts.map((contour) => {
        const [[minX, minY], [maxX, maxY]] = [contour.bounds!.min, contour.bounds!.max];
        return (
          <rect
            key={`ghost-${contour.index}`}
            x={minX}
            y={minY}
            width={Math.max(maxX - minX, 0.001)}
            height={Math.max(maxY - minY, 0.001)}
            className="ghost"
          >
            <title>{`${contour.piece ?? "contour"} — preview only`}</title>
          </rect>
        );
      })}
      {edges.map((edge) => {
        const points = edge.points.map(([x, y]) => `${x},${y}`).join(" ");
        const active = selection.edge?.panel === edge.panel && selection.edge?.name === edge.name;
        return (
          <polyline
            key={`${edge.panel}/${edge.name}`}
            points={points}
            className="edge-hit"
            data-active={active}
            onPointerDown={(event) => {
              event.stopPropagation();
              onPickEdge(edge);
            }}
          >
            <title>{`${edge.panel} · ${edge.name} · ${(edge.length_m * 100).toFixed(1)} cm`}</title>
          </polyline>
        );
      })}
    </svg>
  );
}

function ImportPanelHost({
  document,
  onApplied,
  onError,
  onPreview,
}: {
  document: DocumentSummary["document"];
  onApplied: () => void;
  onError: (message: string) => void;
  onPreview: (contours: ImportContour[]) => void;
}) {
  const fabricId = document?.fabrics[0]?.id ?? null;
  return (
    <ImportPanel
      fabricId={fabricId}
      onApplied={onApplied}
      onError={onError}
      onPreview={onPreview}
    />
  );
}

function Inspector({
  document,
  edges,
  selection,
  onSelect,
  onChanged,
  onError,
  busy,
  onBusy,
}: {
  document: DocumentSummary["document"];
  edges: BoardEdge[];
  selection: Selection;
  onSelect: (panel: string) => void;
  onChanged: () => void;
  onError: (message: string) => void;
  busy: boolean;
  onBusy: (value: boolean) => void;
}) {
  if (!document) {
    return <div className="inspector-empty">Nothing open yet.</div>;
  }
  const selectedPanel = document.panels.find((panel) => panel.id === selection.panel);
  return (
    <>

      <section className="group">
        <h2>
          Pieces <span className="count">{document.counts.panels}</span>
        </h2>
        <ul className="piece-list">
          {document.panels.map((panel) => (
            <li key={panel.id}>
              <button
                type="button"
                className="piece-row"
                data-selected={selection.panel === panel.id}
                onClick={() => onSelect(panel.id)}
              >
                <span className="piece-name">{panel.name}</span>
                <span className="piece-meta">
                  {panel.fabric} · {panel.boundaries.length} edges
                </span>
              </button>
            </li>
          ))}
        </ul>
      </section>

      {selection.edge ? <EdgeCard edge={selection.edge} /> : null}
      {selectedPanel ? (
        <>
          <PieceDetail panel={selectedPanel} />
          <MovePanel
            panel={selectedPanel}
            busy={busy}
            onBusy={onBusy}
            onChanged={onChanged}
            onError={onError}
          />
        </>
      ) : null}

      <section className="group">
        <h2>
          Seams <span className="count">{document.counts.seams}</span>
        </h2>
        <ul className="seam-list">
          {document.seams.map((seam) => (
            <li key={seam.id}>
              <span className="seam-name">{seam.id}</span>
              <span className="seam-meta">
                {seam.a.edge ?? seam.a.side} ↔ {seam.b.edge ?? seam.b.side}
                {seam.allow_ease ? " · ease" : ""}
              </span>
            </li>
          ))}
        </ul>
      </section>
    </>
  );
}

function PieceDetail({ panel }: { panel: Panel }) {
  return (
    <section className="group detail">
      <h2>{panel.name}</h2>
      <dl>
        <dt>Fabric</dt>
        <dd>{panel.fabric}</dd>
        <dt>Grain</dt>
        <dd>{panel.grain_degrees}°</dd>
        <dt>Boundaries</dt>
        <dd>
          <ul className="tag-list">
            {panel.boundaries.map((boundary) => (
              <li key={boundary}>{boundary}</li>
            ))}
          </ul>
        </dd>
      </dl>
    </section>
  );
}

function EdgeCard({ edge }: { edge: BoardEdge }) {
  return (
    <section className="group detail">
      <h2>Boundary</h2>
      <dl>
        <dt>Piece</dt>
        <dd>{edge.panel}</dd>
        <dt>Name</dt>
        <dd>{edge.name}</dd>
        <dt>Rest length</dt>
        <dd>{(edge.length_m * 100).toFixed(2)} cm</dd>
      </dl>
    </section>
  );
}

function MovePanel({
  panel,
  busy,
  onBusy,
  onChanged,
  onError,
}: {
  panel: Panel;
  busy: boolean;
  onBusy: (value: boolean) => void;
  onChanged: () => void;
  onError: (message: string) => void;
}) {
  const [dx, setDx] = useState("0");
  const [dy, setDy] = useState("0");
  const [rotation, setRotation] = useState("0");

  async function send(kind: "layout" | "board_transform") {
    const x = Number.parseFloat(dx);
    const y = Number.parseFloat(dy);
    const degrees = Number.parseFloat(rotation);
    if (!Number.isFinite(x) || !Number.isFinite(y) || !Number.isFinite(degrees)) {
      onError("move values must be numbers (metres and degrees)");
      return;
    }
    onBusy(true);
    try {
      if (kind === "layout") {
        await api.command({ kind: "layout", panel: panel.id, origin: [x, y] });
      } else {
        await api.command({
          kind: "board_transform",
          panel: panel.id,
          origin: [x, y],
          rotation_degrees: degrees,
        });
      }
      await onChanged();
    } catch (problem) {
      onError(problem instanceof Error ? problem.message : String(problem));
    } finally {
      onBusy(false);
    }
  }

  const current = panel.layout_2d ?? [0, 0];
  return (
    <section className="group">
      <h2>Move piece</h2>
      <div className="field-row">
        <label>
          u (m)
          <input value={dx} onChange={(event) => setDx(event.target.value)} inputMode="decimal" />
        </label>
        <label>
          v (m)
          <input value={dy} onChange={(event) => setDy(event.target.value)} inputMode="decimal" />
        </label>
        <label>
          rot (°)
          <input
            value={rotation}
            onChange={(event) => setRotation(event.target.value)}
            inputMode="decimal"
          />
        </label>
      </div>
      <div className="field-row">
        <button type="button" disabled={busy} onClick={() => send("layout")}>
          Set position
        </button>
        <button type="button" disabled={busy} onClick={() => send("board_transform")}>
          Apply transform
        </button>
      </div>
      <p className="hint">
        board now ({current[0].toFixed(2)}, {current[1].toFixed(2)}) m · presentation only, rest
        geometry unchanged
      </p>
    </section>
  );
}
