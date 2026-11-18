import { useEffect, useRef } from "react";
import type { DrapeSnapshot } from "./api";

/**
 * A plain orthographic wireframe of the draped mesh. No lighting, no shadow,
 * no material: nothing in this view may imply a fitted garment. The solver
 * name and the not-a-fit claim live in the transport label above it.
 */
export function DrapeView({ snapshot }: { snapshot: DrapeSnapshot | null }) {
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
    if (!snapshot || snapshot.positions.length === 0) return;

    // Project world (x right, y up) to screen; z is ignored by an ortho side
    // view on purpose - a perspective camera would fake depth cues a preview
    // has not earned.
    let minX = Infinity, maxX = -Infinity, minY = Infinity, maxY = -Infinity;
    for (const [x, , y] of snapshot.positions) {
      minX = Math.min(minX, x); maxX = Math.max(maxX, x);
      minY = Math.min(minY, y); maxY = Math.max(maxY, y);
    }
    const span = Math.max(maxX - minX, maxY - minY, 0.05);
    const scale = (Math.min(width, height) * 0.85) / span;
    const ox = width / 2 - ((minX + maxX) / 2) * scale;
    const oy = height / 2 + ((minY + maxY) / 2) * scale;
    const px = ([x, , y]: [number, number, number]) => [ox + x * scale, oy - y * scale];

    const colours = ["#c8452b", "#2f6f6a", "#8a6b2f", "#4a5b8c", "#7a4a6a", "#3f6b3f"];
    snapshot.panels.forEach((panel, index) => {
      context.strokeStyle = colours[index % colours.length];
      context.lineWidth = 0.5;
      context.globalAlpha = 0.85;
      for (const [a, b, c] of panel.triangles) {
        const pa = px(snapshot.positions[a]);
        const pb = px(snapshot.positions[b]);
        const pc = px(snapshot.positions[c]);
        context.beginPath();
        context.moveTo(pa[0], pa[1]);
        context.lineTo(pb[0], pb[1]);
        context.lineTo(pc[0], pc[1]);
        context.closePath();
        context.stroke();
      }
    });
    context.globalAlpha = 1;
  }, [snapshot]);

  return <canvas ref={canvas} className="drape-canvas" aria-label="Drape preview wireframe" />;
}