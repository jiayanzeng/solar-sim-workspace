#!/bin/sh
set -eu

cargo run --quiet -p xtask -- check-texture-metadata --dir assets/textures
