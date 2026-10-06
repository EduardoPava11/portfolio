//! `portfolio`: builds the static site into `docs/` and runs the colour analysis into `analysis/`.
//!
//!   cargo run --release -- build          write the site
//!   cargo run --release -- red [opts]     analyse the reds in every photograph
//!   cargo run --release -- all [opts]     both
//!
//! Red options:  --chroma-floor F    minimum OKLCh chroma for a pixel to count as coloured (default 0.04)
//!               --hue LO HI         red sector in OKLCh degrees (default: derived from the sRGB primaries)
//!               --side N            analysis resolution, longest side in pixels (default 1200)

mod color;
mod content;
mod images;
mod red;
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
        "all" => site::build(root).and_then(|_| red::run(root, &args[1..])),
        _ => {
            eprintln!("usage: portfolio [build|red|all] [--chroma-floor F] [--hue LO HI] [--side N]");
            std::process::exit(2);
        }
    };
    if let Err(e) = outcome {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
