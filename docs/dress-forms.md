# Dress forms as bodies for the preview

The studio's drape preview used to build its solver with no body (`VbdSolver::new(garment, None, …)`). It can now
drape on a body chosen from `avatar_catalog`: the bundled MakeHuman mannequin or a **dress form** installed under
`assets/dressforms/<id>/`.

## Use

1. Put the form's OBJ next to its `form.json` (see `assets/dressforms/dressform-scan-v01/` for the shape).
2. `cargo run --no-default-features --bin cloth-server` from the repository root (or set `CLOTH_ASSETS`).
3. In the studio, open a garment and choose the body in the drape toolbar. Press Step or Play.

## `form.json`

| Field | Meaning |
|---|---|
| `id` | lowercase letters, digits, hyphens; **must equal the directory name** |
| `title` | the picker's label |
| `mesh` | a bare `.obj` file name in the same directory (no separators, no `..`) |
| `height_m` | the form's height in metres; the mesh is checked against it |
| `shoulder_height_m` | where the garment's top edge is put; positive and not above `height_m` |
| `dimensions_cm` | optional named girths, for display |
| `source`, `license` | required. Say where the mesh came from and what lets it be used |
| `provenance` | optional free text: how the scale and dimensions were obtained |

Unknown fields are refused.

## What is checked, and what is deliberately not done

- **Units.** OBJ coordinates are read as metres and never rescaled. A mesh whose measured height differs from
  `height_m` by more than 2 % is **refused**, naming a likely unit error. A mesh that does not start within
  1 cm of y = 0 is refused.
- **Paths.** `form.json` cannot name a file outside its own directory, and the API never returns a filesystem path.
- **A malformed form is listed, with the reason**, instead of being skipped.
- **Placement.** The garment is lifted so its highest vertex sits on `shoulder_height_m`. It is **not scaled**:
  a template sized for the 1.75 m mannequin goes on a smaller form at its own size. The document's template is
  never changed; the lift is applied to a copy for the preview.
- **Not a fit.** The preview says so. Contact is the CPU VBD solver's, with no friction, no continuous collision and
  no self-collision, on a 24,000-triangle body.

## HTTP

| Route | |
|---|---|
| `GET /api/avatars` | `{selected, avatars[]}`: id, title, kind, `available`, `reason`, heights, dimensions, source, licence |
| `POST /api/avatar` | `{"id": "<id>"}` or `{"id": null}`; reads and checks the mesh now, resets the preview |
| `GET /api/avatar/mesh` | the selected body's surface in metres, for drawing |
| `POST /api/sim/step` | the snapshot now carries `avatar` and `garment_lift_m` |

## Cost *(measured on a cloud CPU, debug profile, T-shirt template)*

Per solver substep: no body 235 ms, the 26,700-triangle mannequin 357 ms, a 24,000-triangle form with the garment
resting on it 613 ms. None of these is interactive. A lighter collider for the CPU preview is the obvious next step.
