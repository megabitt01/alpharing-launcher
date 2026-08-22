# Tauri + React

This template should help get you started developing with Tauri and React in Vite.

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)

## Building

When building on Linux, use this command:
NO_STRIP=true GSTREAMER_HELPERS_DIR=/usr/lib/gstreamer-1.0 LD_LIBRARY_PATH="$PWD/src-tauri/libs:$LD_LIBRARY_PATH" npm run tauri build

When build with Docker on Linux, use these commands:
docker build -t alpharing-launcher-builder .

docker run --rm \
  -v "$PWD/src-tauri/target:/app/src-tauri/target" \
  -v alpharing-cargo-registry:/root/.cargo/registry \
  alpharing-launcher-builder
