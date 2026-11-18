import { useEffect, useRef } from "react";
import type { AvatarMesh, DrapeSnapshot } from "./api";

type Point = [number, number, number];

/**
 * A plain orthographic front view: world x to the right, world y up, z ignored.
 * No lighting, no shadow, no material: nothing in this view may imply a fitted
 * garment. The solver name and the not-a-fit claim live in the caption.
 *
 * The body, when one is selected, is drawn as its silhouette (the extent of its
 * surface along x at each height) plus a dashed shoulder line, behind the
 * garment. It is the collider the solver uses, not a render of it.
 */
export function DrapeView({
  snapshot,
  avatar,
}: {
  snapshot: DrapeSnapshot | null;
  avatar: AvatarMesh | null;
}) {
  const canvas = useRef<HTMLCanvasElement | null>(null);

  useEffect(() => {
    const element = canvas.current;
    if (!element) return;
    const context = element.getContext("2d");
    if (!context) return;
    const width = element.clientWidth;
    const height = element.clientHeight;
    const ratio = window.devicePixelRatio || 1;
    element.width = Math.max(1, Math.floor(width * ratio));
    element.height = Math.max(1, Math.floor(height * ratio));
    context.setTransform(ratio, 0, 0, ratio, 0, 0);
    context.clearRect(0, 0, width, height);
    const garment: Point[] = snapshot?.positions ?? [];
    const body: Point[] = avatar?.positions ?? [];
    if (garment.length === 0 && body.length === 0) return;

    let minX = Infinity, maxX = -Infinity, minY = Infinity, maxY = -Infinity;
    for (const [x, y] of [...garment, ...body]) {
      minX = Math.min(minX, x); maxX = Math.max(maxX, x);
      minY = Math.min(minY, y); maxY = Math.max(maxY, y);
    }
    const span = Math.max(maxX - minX, maxY - minY, 0.05);
    const scale = (Math.min(width, height) * 0.85) / span;
    const ox = width / 2 - ((minX + maxX) / 2) * scale;
    const oy = height / 2 + ((minY + maxY) / 2) * scale;
    const px = ([x, y]: Point): [number, number] => [ox + x * scale, oy - y * scale];

    if (avatar && body.length > 0) {
      // Silhouette: at each height band, the leftmost and rightmost vertex.
      const bands = 200;
      const lo = new Array<number>(bands).fill(Infinity);
      const hi = new Array<number>(bands).fill(-Infinity);
      const bottom = Math.min(...body.map((p) => p[1]));
      const top = Math.max(...body.map((p) => p[1]));
      for (const [x, y] of body) {
        const band = Math.min(bands - 1, Math.floor(((y - bottom) / (top - bottom || 1)) * bands));
        lo[band] = Math.min(lo[band], x);
        hi[band] = Math.max(hi[band], x);
      }
      const rows = lo.map((_, band) => band).filter((band) => Number.isFinite(lo[band]));
      context.beginPath();
      rows.forEach((band, index) => {
        const [sx, sy] = px([lo[band], bottom + ((band + 0.5) / bands) * (top - bottom), 0]);
        if (index === 0) context.moveTo(sx, sy); else context.lineTo(sx, sy);
      });
      [...rows].reverse().forEach((band) => {
        const [sx, sy] = px([hi[band], bottom + ((band + 0.5) / bands) * (top - bottom), 0]);
        context.lineTo(sx, sy);
      });
      context.closePath();
      context.fillStyle = "rgba(150, 150, 160, 0.18)";
      context.fill();
      context.strokeStyle = "rgba(150, 150, 160, 0.7)";
      context.lineWidth = 1;
      context.stroke();
      if (avatar.shoulder_height_m !== null) {
        const [, sy] = px([0, avatar.shoulder_height_m, 0]);
        context.setLineDash([4, 4]);
        context.beginPath();
        context.moveTo(ox + minX * scale, sy);
        context.lineTo(ox + maxX * scale, sy);
        context.strokeStyle = "rgba(150, 150, 160, 0.5)";
        context.stroke();
        context.setLineDash([]);
      }
    }

    const colours = ["#c8452b", "#2f6f6a", "#8a6b2f", "#4a5b8c", "#7a4a6a", "#3f6b3f"];
    snapshot?.panels.forEach((panel, index) => {
      context.strokeStyle = colours[index % colours.length];
      context.lineWidth = 0.5;
      context.globalAlpha = 0.85;
      for (const [a, b, c] of panel.triangles) {
        const pa = px(garment[a]);
        const pb = px(garment[b]);
        const pc = px(garment[c]);
        context.beginPath();
        context.moveTo(pa[0], pa[1]);
        context.lineTo(pb[0], pb[1]);
        context.lineTo(pc[0], pc[1]);
        context.closePath();
        context.stroke();
      }
    });
    context.globalAlpha = 1;
  }, [snapshot, avatar]);

  return <canvas ref={canvas} className="drape-canvas" aria-label="Drape preview wireframe" />;
}
