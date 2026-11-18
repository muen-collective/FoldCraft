//! Write a bundled form to an OBJ file: `cargo run --example build_dress_form -- <id> <out.obj>`.
use marvelous_cloth::avatar_catalog::{assets_root, discover};
use std::fmt::Write;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (id, out) = match args.as_slice() {
        [id, out] => (id, out),
        _ => return eprintln!("usage: build_dress_form <id> <out.obj>"),
    };
    let entries = discover(&assets_root());
    let Some(entry) = entries.iter().find(|e| &e.id == id) else {
        return eprintln!("no form {id}");
    };
    let loaded = match entry.load() {
        Ok(l) => l,
        Err(e) => return eprintln!("{e}"),
    };
    let mut s = String::new();
    for v in &loaded.surface.vertices {
        let _ = writeln!(s, "v {:.6} {:.6} {:.6}", v.x, v.y, v.z);
    }
    for t in &loaded.surface.triangles {
        let _ = writeln!(s, "f {} {} {}", t[0] + 1, t[1] + 1, t[2] + 1);
    }
    std::fs::write(out, s).expect("write");
    if let Some(sizer) = &loaded.sizer {
        // The landmark heights as built (a form built to a chart row has moved them), and what each measures.
        let side = serde_json::json!({
            "knot_y_m": sizer.knot_heights(),
            "girth_cm": sizer.base.girth_cm,
        });
        std::fs::write(format!("{out}.landmarks.json"), side.to_string()).expect("write landmarks");
    }
    // Separate meshes (removable arms) next to the body: <out>.<name>.obj
    for (name, part) in &loaded.parts {
        let mut text = String::new();
        for v in &part.vertices {
            let _ = writeln!(text, "v {:.6} {:.6} {:.6}", v.x, v.y, v.z);
        }
        for t in &part.triangles {
            let _ = writeln!(text, "f {} {} {}", t[0] + 1, t[1] + 1, t[2] + 1);
        }
        std::fs::write(format!("{out}.{name}.obj"), text).expect("write part");
        println!("  part {name}: {} vertices", part.vertices.len());
    }
    let lines = loaded.guide_lines();
    std::fs::write(
        format!("{out}.lines.json"),
        serde_json::to_string(&lines).expect("json"),
    )
    .expect("write lines");
    for l in &lines {
        println!(
            "  {:?} {} ({} points{})",
            l.kind,
            l.name,
            l.points.len(),
            if l.closed { ", closed" } else { "" }
        );
    }
    println!("wrote {out}: {} vertices", loaded.surface.vertices.len());
}
