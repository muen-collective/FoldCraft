
import { useState } from "react";
import { api, type ImportPreview } from "./api";

type Props = {
  fabricId: string | null;
  onApplied: () => void;
  onError: (message: string) => void;
};

/** Review a DXF before it touches the document: contours, names, node counts
 *  with and without fitting, eligibility reasons and the left-out ledger. */
export function ImportPanel({ fabricId, onApplied, onError }: Props) {
  const [path, setPath] = useState("");
  const [preview, setPreview] = useState<ImportPreview | null>(null);
  const [selected, setSelected] = useState<Set<number>>(new Set());
  const [busy, setBusy] = useState(false);

  async function inspect() {
    if (!path.trim()) return;
    setBusy(true);
    try {
      const result = await api.importPreview(path.trim(), 0.001);
      setPreview(result);
      setSelected(new Set(result.contours.filter((c) => c.eligible).map((c) => c.index)));
    } catch (problem) {
      onError(problem instanceof Error ? problem.message : String(problem));
      setPreview(null);
    } finally {
      setBusy(false);
    }
  }

  async function apply() {
    if (!fabricId || selected.size === 0) return;
    setBusy(true);
    try {
      await api.importApply(path.trim(), [...selected].sort((a, b) => a - b), fabricId, 0.001);
      setPreview(null);
      setSelected(new Set());
      onApplied();
    } catch (problem) {
      onError(problem instanceof Error ? problem.message : String(problem));
    } finally {
      setBusy(false);
    }
  }

  function toggle(index: number) {
    setSelected((current) => {
      const next = new Set(current);
      if (next.has(index)) next.delete(index);
      else next.add(index);
      return next;
    });
  }

  return (
    <section className="group" aria-label="Import DXF">
      <h2>Import DXF</h2>
      <div className="import-row">
        <input
          className="import-path"
          placeholder="/absolute/path/to/pattern.dxf"
          value={path}
          onChange={(event) => setPath(event.target.value)}
          onKeyDown={(event) => event.key === "Enter" && inspect()}
        />
        <button type="button" onClick={inspect} disabled={busy || !path.trim()}>
          Preview
        </button>
      </div>
      {preview ? (
        <>
          <p className="import-meta">
            {(preview.units.metresPerUnit * 1000).toFixed(0)} mm/unit
            {preview.units.overridden ? " override" : ""} · {preview.leftOutTotal} left out
            {preview.sampleSize ? ` · sample ${preview.sampleSize}` : ""}
          </p>
          <ul className="contour-list">
            {preview.contours
              .filter((c) => c.closed)
              .map((c) => (
                <li key={c.index}>
                  <label className={c.eligible ? undefined : "contour-off"}>
                    <input
                      type="checkbox"
                      disabled={!c.eligible}
                      checked={selected.has(c.index)}
                      onChange={() => toggle(c.index)}
                    />
                    <span className="contour-name">{c.piece ?? `(layer ${c.layer})`}</span>
                    <span className="contour-meta">
                      {c.sourceNodes} nodes
                      {c.fittedNodes != null ? ` -> ${c.fittedNodes}, ${c.curvedEdges} curved` : ""}
                      {c.reason ? ` · ${c.reason}` : ""}
                    </span>
                  </label>
                </li>
              ))}
          </ul>
          <div className="import-row">
            <button
              type="button"
              className="import-apply"
              onClick={apply}
              disabled={busy || selected.size === 0 || !fabricId}
            >
              Add {selected.size} piece{selected.size === 1 ? "" : "s"}
            </button>
            <button type="button" onClick={() => setPreview(null)} disabled={busy}>
              Cancel
            </button>
          </div>
        </>
      ) : null}
    </section>
  );
}
