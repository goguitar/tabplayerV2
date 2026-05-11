# TabPlayer using Godot and Rust

This application lets you play rocksmith cldc and others by importing them in app forever.

It allows stopping, rewinding and skipping through songs much faster than rocksmith.
And allows you to play along with the notes as you would TAB or sheet music.

This doesn't listen to a plugged in guitar or tell you notes hit

See the MiiChannel song, note the strings and note preview at: https://www.murph9.com/mygames

## How to play

1. Download the latest release from [Github Releases](https://github.com/Murph9/tabplayerV2/releases)

1. Download some songs from various sources like [CustomsForge](https://customsforge.com/index.php) (requires a free account)

1. Go to the convert page and select the downloaded songs
1. Run the reload song list feature
1. Play Songs

## Included Rust Dependencies

-   [godot-rust/gdext](https://github.com/godot-rust/gdext) for Godot scripting
-   [Rocksmith2014.rs](https://github.com/santzit/rocksmith2014.rs) for PSARC/SNG/XML parsing
-   [vgmstream r2083](https://github.com/vgmstream/vgmstream/releases/tag/r2083) shared library for WEM → WAV decoding

### Building the GDExtension

```
cd rust
cargo build -p tabplayer_gd
```

The resulting shared library is loaded from:

- `res://rust/target/debug/libtabplayer_gd.so` (Linux)
- `res://rust/target/debug/tabplayer_gd.dll` (Windows)
- `res://rust/target/debug/libtabplayer_gd.dylib` (macOS)

### vgmstream shared library

The Linux `libvgmstream.so` (r2083) is vendored under `third_party/vgmstream/linux/`.
For other platforms, build and drop the corresponding shared library into:

- `third_party/vgmstream/windows/libvgmstream.dll`
- `third_party/vgmstream/macos/libvgmstream.dylib`
