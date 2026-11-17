//! Print what the DXF reader makes of a file. CPU only.
use marvelous_cloth::garment::dxf_import::{self, Options};

fn main() {
    let path = std::env::args().nth(1).expect("usage: dxf_probe <file.dxf>");
    let text = std::fs::read_to_string(&path).expect("readable UTF-8 DXF");
    let options = Options {
        metres_per_unit: Some(0.001),
        ..Options::default()
    };
    match dxf_import::read_ascii(&text, options) {
        Ok(drawing) => {
            let closed = drawing.paths.iter().filter(|p| p.closed).count();
            let points: usize = drawing.paths.iter().map(|p| p.points.len()).sum();
            println!("paths {} closed {} points {} insunits {:?} m/unit {} override {}",
                drawing.paths.len(), closed, points, drawing.insunits, drawing.metres_per_unit, drawing.scale_overridden);
            println!("issues {}", drawing.issues.len());
            for issue in drawing.issues.iter().take(8) {
                println!("issue {} {} :: {}", issue.entity, issue.layer, issue.reason);
            }
            for path in drawing.paths.iter().take(5) {
                println!("path {} layer {} closed {} points {}", path.entity, path.layer, path.closed, path.points.len());
            }
        }
        Err(error) => println!("refused: {error}"),
    }
}
