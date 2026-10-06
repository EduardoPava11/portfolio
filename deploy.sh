#!/bin/sh
# Build the site and publish it to GitHub Pages.
#
# docs/ is kept out of the source history (web sized copies of every photograph change
# with every build). It goes to the gh-pages branch as a single commit that replaces
# the one before it. Pages serves the root of gh-pages.
set -e
cd "$(dirname "$0")"
REMOTE=$(git remote get-url origin)

cargo run --release -- build

cd docs
rm -rf .git
git init -q -b gh-pages
git add -A
git -c commit.gpgsign=false commit -q -m "Site built $(date -u +%Y-%m-%dT%H:%MZ)"
git push -f -q "$REMOTE" gh-pages
rm -rf .git
echo "published"
