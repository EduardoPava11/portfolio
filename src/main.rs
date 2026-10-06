//! `portfolio`: builds the static site into `docs/` and runs the colour analysis into `analysis/`.
//!
//!   cargo run --release -- build          write the site
//!   cargo run --release -- red [opts]     analyse the reds in every photograph
//!   cargo run --release -- all [opts]     both
//!   cargo run --release -- lab            CIELAB reading of every pixel; writes the colour order
//!   cargo run --release -- frame [opts]   borders via matte into content/framed/
//!   cargo run --release -- export         submission JPEGs (5 MB, 01_LastName.jpeg)
//!
//! Red options:  --chroma-floor F    minimum OKLCh chroma for a pixel to count as coloured (default 0.04)
//!               --hue LO HI         red sector in OKLCh degrees (default: derived from the sRGB primaries)
//!               --side N            analysis resolution, longest side in pixels (default 1200)

mod color;
mod content;
mod export;
mod frame;
mod images;
mod lab;
mod red;
mod rename;
mod site;

use std::error::Error;
use std::path::Path;

pub type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cmd = args.first().map(String::as_str).unwrap_or("build");
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let outcome = match cmd {
        "build" => site::build(root),
        "red" => red::run(root, &args[1..]),
        "lab" => lab::run(root, &args[1..]),
        "frame" => frame::run(root, &args[1..]),
        "export" => export::run(root),
        "rename" => rename::run(root),
        "all" => site::build(root).and_then(|_| red::run(root, &args[1..])),
        _ => {
            eprintln!("usage: portfolio [build|lab|frame|export|red|all]");
            eprintln!("  lab      read every photograph in CIELAB, write the colour order and each picture's mat");
            eprintln!("           [--register dark|mid|light] [--intensity mute|balanced|statement]");
            eprintln!("  frame    borders via matte -> content/framed/ (pass matte options after the command)");
            eprintln!("  export   submission JPEGs from content/selection.txt -> export/");
            eprintln!("  rename   number the files 01-, 02-, ... in colour order (a file named end goes last)");
            eprintln!("  red      red analysis [--chroma-floor F] [--hue LO HI] [--side N]");
            std::process::exit(2);
        }
    };
    if let Err(e) = outcome {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
