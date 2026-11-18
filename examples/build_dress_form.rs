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
    println!("wrote {out}: {} vertices", loaded.surface.vertices.len());
}
