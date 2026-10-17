#!/bin/sh
# Render the application icon PNGs from assets/icon.svg; needs rsvg-convert (librsvg).
set -eu

shell_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
svg="$shell_dir/assets/icon.svg"
icons="$shell_dir/packaging/linux/icons"
mkdir -p -- "$icons"
for size in 16 22 24 32 48 64 128 256 512; do
    rsvg-convert -w "$size" -h "$size" -o "$icons/$size.png" -- "$svg"
done
# The tray icon is decoded at runtime from this embedded copy.
rsvg-convert -w 64 -h 64 -o "$shell_dir/assets/icon.png" -- "$svg"
