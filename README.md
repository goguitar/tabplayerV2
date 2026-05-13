# TabPlayer (Godot + Rust)

TabPlayer is a Rocksmith-style practice player built with Godot and Rust.

It lets you load songs from DLC `.psarc` files, browse/filter your library, and play with skip/loop/speed controls while viewing note/chord charts.

This app does not do real-time guitar input detection or scoring.

## Current Runtime

- Godot controllers run through Rust GDExtension (`godot-rust` `v0.5.2`)
- Runtime song source is PSARC-first (no converted song-folder dependency)
- Audio path is WEM decode via `vgmstream` and in-memory WAV stream load
- Song catalog is rescanned from DLC at startup

## How to play

1. Download a release from [GitHub Releases](https://github.com/Murph9/tabplayerV2/releases)
2. Put your `.psarc` DLC files under your DLC folder (default Linux path used by this repo: `/home/csantz/Music/DLC`)
3. Launch the game
4. Open Song Pick and play

## Build (Linux)

Build and copy the extension library into `godot/bin/`:

```bash
tools/build_godot_bridge.sh
```

Equivalent manual commands:

```bash
cargo build -p godot_bridge --manifest-path rust/Cargo.toml
cp rust/target/debug/libgodot_bridge.so godot/bin/libgodot_bridge.so
```

Expected library names:

- Linux: `libgodot_bridge.so`
- Windows: `godot_bridge.dll`
- macOS: `libgodot_bridge.dylib`
