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

The order of work for a submission: `lab` to read and order the body by colour and
derive the mats, `rename` to number the files, `frame` to put the mats on, prune
`content/selection.txt` to the pictures you are sending, `export` to write the
numbered JPEGs, `build` and `./deploy.sh` for the site.

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

## Borders: a mat for every picture

Daniel's rule: the borders reflect the colours of the pictures, so they change from
picture to picture; everything else on the site is constant black and white.

`portfolio lab` derives each picture's mat from the CIELAB reading. The hue is the
picture's own chroma weighted hue. The lightness is one value for the whole set, so
the surround never jumps from frame to frame. The chroma is paced over the sequence:
a raised cosine envelope that sits at the `--floor` level on the first and last
picture and reaches the `--intensity` level at the centre, so the body opens and
closes quietly and peaks in the middle. Both levels come from the set's own
statistics in matte's vocabulary:

| | dark | mid (default) | light |
| --- | --- | --- | --- |
| L* | 15th percentile of the pictures' median L* | 50th | 85th |

| | mute (default floor) | balanced | statement (default centre) |
| --- | --- | --- | --- |
| C* | half the pictures' mean chroma | the mean | 90th percentile |

Out of gamut mats are mapped by holding L* and hue and bisecting on chroma. The
result is the `mat_hex` column of `analysis/lab/lab.csv` and the second swatch on the
contact sheet.

`portfolio frame` then runs `matte frame` (`~/MATTE`) once per picture with
`--color <mat_hex>`; matte grows the canvas without resampling. The mats are cut with
`--bottom-weight 1.15`, the optical centring of traditional mat cutting. About 4.5 s
per picture. Only missing or stale frames are redone; `--force` redoes all. Other matte
options may follow the command, for example `portfolio frame -- --border 0.08`.

```sh
cargo run --release -- lab --register light --intensity balanced --floor mute   # a different mat law
cargo run --release -- frame --force -- --bottom-weight 1.15                     # reframe all
```

## Publishing

`./deploy.sh` builds and force pushes `docs/` to the `gh-pages` branch, which GitHub
Pages serves. The `main` branch holds only sources and content.
