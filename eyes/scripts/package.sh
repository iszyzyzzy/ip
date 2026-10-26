#!/usr/bin/env sh
set -eu
cd "$(dirname "$0")/.."
cargo build --locked --release --bin eyes --example controller
bundle=dist/eyes-linux
mkdir -p "$bundle/assets" "$bundle/proto" "$bundle/examples"
cp target/release/eyes "$bundle/eyes"
cp target/release/examples/controller "$bundle/controller"
# Linux 包使用独立目录，不会覆盖 Windows 发布包或用户的现有配置。
cp -R assets/. "$bundle/assets/"
cp proto/eyes.proto "$bundle/proto/"
cp examples/simulator.json "$bundle/examples/"
cp scripts/run-pi5.sh "$bundle/run-pi5.sh"
cp README.md q-eyes-preview.png "$bundle/"
printf 'Linux 发布目录：%s\n' "$bundle"
