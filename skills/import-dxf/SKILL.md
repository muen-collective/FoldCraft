---
name: import-dxf
description: Open a real DXF pattern export (CLO, Gerber, Optitex, Lectra, Seamly2D or DrapeXD) as editable panels — preview the contours and their names first, apply an unambiguous selection, never guess holes or sewing lines.
---

# Import a DXF pattern

An imported block is legitimate once its edges are named. The importer expands
blocks, reads AAMA-style records (`PIECE NAME`, `SAMPLE SIZE`, `UNITS`) and refits
dense samples into cubic edges with a node budget. It refuses to invent
relationships: nested outlines, holes and cut-vs-sew pairs are **rejected**, not
interpreted.

## Steps

1. Preview first. This never touches the document:
   ```json
   document_import_preview {
     "path": "/abs/path/pattern.dxf",
     "metres_per_unit": 0.001
   }
   ```
   - Pass `metres_per_unit` when the file has no unit header: 0.001 mm, 0.01 cm,
     0.0254 in, 1.0 m. Old R10/R12 exports often carry none — the parka needs it.
   - Each contour reports `piece` (its record name, if any), `closed`,
     `sourceNodes`, `fittedNodes`, `curvedEdges`, `eligible` and a refusal `reason`.
   - `leftOut` is the ledger of dropped entities; read it before promising the
     user a complete import.
2. Choose an existing fabric id from `document_inspect` (e.g. `woven-reference`).
3. Apply one undoable entry:
   ```json
   document_import_apply {
     "path": "/abs/path/pattern.dxf",
     "selected": [0, 7],
     "fabric": "woven-reference",
     "metres_per_unit": 0.001
   }
   ```
4. Re-read with `document_inspect`: new panel ids start with `dxf_`; each piece's
   boundaries are named `edge_1..n`, so operators have targets.

## Rules

- Select **outer outlines only**. Exports commonly nest a cut line around a
  sewing line for the same piece; selecting both is refused rather than guessed
  as a hole. One outline per physical piece.
- Open paths, non-finite points and contours outside 3..4096 points are
  ineligible — say so instead of forcing them in.
- Fitting is fidelity-gated: node counts drop toward the budget (default 32)
  only while every edge stays within 0.5 mm of its source samples. A reported
  `fittedNodes` above the budget means the shape needed them.
- Imported geometry is geometric truth from someone else's CAD. Everything you
  add afterwards (seams, roles, allowances) is recorded intent; nothing may
  claim the import itself was drafted parametrically.

## Refusals and what they mean

| code | meaning |
|---|---|
| `invalid_dxf` | the file does not parse as bounded ASCII DXF (binary is refused) |
| `no_units` / scale errors inside the preview | pass an explicit metres_per_unit |
| `invalid_geometry` + "nested/overlapping" | your selection mixes cut and sewing lines, or two pieces overlap — pick outer outlines |
| `no_document` | open a document first; import appends, it never replaces |
| `stale_revision` | re-inspect; something changed the document since your guard |

## Verify

- Panel count grew by exactly the number selected.
- Every new panel carries a boundary list; sew against those names.
- `garment_undo` removes the whole import in one step.
- Quote the left-out ledger honestly: sizes other than the sample size are not
  imported, and blocks deeper than four levels are refused.
