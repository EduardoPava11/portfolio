# Portfolio

A static website for a photography contest entry: the connected body of work, the
artist statement and the bio, each with a copy button. One Rust binary builds the
site and analyses the colour of the photographs. No Node, no Python, no framework.

Live: https://eduardopava11.github.io/portfolio/

## The design

Form follows function. The function is a juror or a visitor looking at the pictures
in sequence, on a phone or a wall sized screen, and finding the words when they want
them. So the page is a reel: one picture per screen, scroll snapped, swipe or arrow
keys to move. The only other things on screen are the name (top left), two words
(Statement, Bio, top right) and the sequence itself, drawn along the bottom edge as
a strip of each picture's CIELAB centroid. The strip is progress bar, navigation and
the colour thesis in one element; the current picture's cell is lit. Statement and
bio open in a panel over the pictures, each with a copy button. The last screen
carries the name, the words and the contact.

Every picture is delivered as AVIF with a JPEG fallback, at 1200 and 2000 px on the
long side, chosen by the browser from `srcset`. Each `img` carries a 24 px JPEG
placeholder inline, so the frame shows its colour before the file arrives. On this
material the AVIF is about a fifth of the JPEG's bytes.

Crates: `image` (decode, resize, JPEG), `ravif` (AVIF, pure Rust AV1), `kamadak-exif`,
`serde` + `toml`, `rayon`. No JavaScript framework and no WebAssembly: the page is
HTML, one stylesheet and one small script, because shipping a framework runtime to
show 55 photographs would be form without function.

## Layout

```
content/
  site.toml       name, title, tagline, contact
  bio.txt         the bio, plain text, blank line between paragraphs
  statement.txt   the artist statement, same format
  photos.toml     captions (and viewing order when there is no order.txt)
  order.txt       colour order written by `lab`
  selection.txt   the pictures to submit, in order, read by `export`
  <photos_dir>/   the photographs (jpg, jpeg, png, tiff); set in site.toml
  framed/         matte's framed copies (ignored by git)
assets/           stylesheet and page script, copied into the site as is
src/              the generator
docs/             the built site (ignored by git, published by deploy.sh)
analysis/red/     the red analysis (ignored by git)
```

## Working on it

```sh
cargo run --release -- lab       # CIELAB reading of every pixel; writes content/order.txt (colour order)
cargo run --release -- frame     # borders via matte -> content/framed/ (matte options may follow)
cargo run --release -- export    # submission JPEGs from content/selection.txt -> export/
cargo run --release -- build     # write docs/, open docs/index.html in a browser
cargo run --release -- red       # analyse the reds, open analysis/red/index.html
./deploy.sh                      # build and publish to GitHub Pages
```

The order of work for a submission: `lab` to read and order the body by colour,
`frame` to put the mat on, prune `content/selection.txt` to the pictures you are
sending, `export` to write the numbered JPEGs, `build` and `./deploy.sh` for the site.

## The Exposure 2027 calls (read 2026-10-05)

Both close 11:59pm, October 5, 2026, through Wufoo forms linked from
exposurephotofestival.com. Exhibition at TRUCK Contemporary Art, Calgary,
February 1 to 28, 2027. Artist fee $456 plus $50 production.

| | North West Showcase | International Open Call |
| --- | --- | --- |
| who | emerging artists in AB, BC, SK, MB, YT, NT | anyone |
| images | 8 to 15 from a connected series | 5 to 10 from a connected series |
| files | JPEG, 5 MB max, `01_LastName.jpeg` | same |
| words | short statement about the project | same, no bio |
| fee | $35 for 10, $5 each after | $45 for 5, $5 each after |
| jury | Monika Szewczyk (Polygon Gallery) + 4 | Shana Lopes (SFMOMA) + 2 |

Jurors score Artistic Strength ("technical and presentation choices feel
deliberate", "a considered, individual approach") and Relevance and Resonance
("engages with ideas, experiences, or questions that matter right now") on a
four point scale. `export` enforces the file rules and reports whether the count
fits each call.

## CIELAB reading and colour order

`cargo run --release -- lab` converts every pixel of every photograph, at full
resolution, from sRGB to CIELAB (D65) and records per photograph the L* a* b*
centroid, median L*, mean and 90th percentile chroma, the chroma weighted mean hue
and how much the picture's colour agrees on that hue, plus a chroma weighted hue
histogram. It then sorts the body by hue and cuts the circle at the widest empty
gap between neighbouring photographs, so the sequence never splits a colour family.
The result is `content/order.txt`, which `build` and `export` follow, and a contact
sheet at `analysis/lab/index.html` with the numbers in `lab.csv`.

## Borders

`cargo run --release -- frame` hands the originals to `matte` (`~/MATTE`), which
measures the set in CAM16-UCS, proposes mat colours from the set's own statistics,
and grows the canvas around each photograph without resampling. The top ranked
candidate is used unless `--pick <id>` or `--color "#RRGGBB"` follows the command;
`matte analyze <folder>` lists the candidates. Framed copies go to `content/framed/`
and, with `framed = true` in `site.toml`, are what the site and the export show.

Photographs: drop files into the folder named by `photos_dir` in `site.toml`. Every file there is included. List the
ones you want first in `content/photos.toml` with a title, place and caption; the rest
follow in alphabetical order. The season and year under each photograph come from the
EXIF capture date (meteorological seasons: Dec to Feb winter, Mar to May spring, Jun to
Aug summer, Sep to Nov autumn). EXIF orientation is applied, so phone photos come out
upright. Web copies are 2000 px on the long side for viewing and 800 px for the grid;
the originals never leave this folder.

Words: edit `content/bio.txt` and `content/statement.txt`, rebuild. The copy button on
the site copies exactly those paragraphs, separated by blank lines.

## Red analysis

`cargo run --release -- red` reads every photograph at 1200 px on the long side,
converts each pixel to OKLab and measures it in OKLCh terms: lightness L, chroma C
(distance from grey) and hue angle h. A pixel counts as red when its chroma clears the
floor (default 0.04, so greys never vote) and its hue falls in the red sector. By default
the sector is derived from the display primaries: from the hue halfway between sRGB
magenta and red, to the hue halfway between red and yellow (358.8 to 69.5 degrees).
That is wide enough to include orange; narrow it with `--hue LO HI`.

```sh
cargo run --release -- red --hue 10 50 --chroma-floor 0.06 --side 1600
```

Output in `analysis/red/`:

- `index.html`: a contact sheet in viewing order. For each photograph: the thumbnail,
  the red mask (red pixels in colour, everything else as grey of the same lightness),
  a hue histogram over the whole wheel with the red sector marked, the red fraction,
  the red share of all colour, the mean red in L, C and h, and small histograms of
  red lightness and red chroma. The whole body is summed at the top.
- `red.csv`: the same numbers, one row per photograph, for a spreadsheet.
- `<file>.png`: the masks.

The mean hue is a circular mean (unit vectors averaged, not angles), so reds that
straddle 0 degrees do not average to cyan.

## Publishing

`./deploy.sh` builds and force pushes `docs/` to the `gh-pages` branch, which GitHub
Pages serves. The `main` branch holds only sources and content.
