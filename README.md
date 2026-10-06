# Portfolio

A static website for a photography contest entry: the connected body of work, the
artist statement and the bio, each with a copy button. One Rust binary builds the
site and analyses the colour of the photographs. No Node, no Python, no framework.

Live: https://eduardopava11.github.io/portfolio/

## Layout

```
content/
  site.toml       name, title, tagline, contact
  bio.txt         the bio, plain text, blank line between paragraphs
  statement.txt   the artist statement, same format
  photos.toml     viewing order and captions
  photos/         the photographs themselves (jpg, jpeg, png)
assets/           stylesheet and page script, copied into the site as is
src/              the generator
docs/             the built site (ignored by git, published by deploy.sh)
analysis/red/     the red analysis (ignored by git)
```

## Working on it

```sh
cargo run --release -- build     # write docs/, open docs/index.html in a browser
cargo run --release -- red       # analyse the reds, open analysis/red/index.html
cargo run --release -- all       # both
./deploy.sh                      # build and publish to GitHub Pages
```

Photographs: drop files into `content/photos/`. Every file there is included. List the
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
