use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-env-changed=VGMSTREAM_STATIC_DIR");

    let mut candidates = Vec::<PathBuf>::new();
    if let Ok(dir) = env::var("VGMSTREAM_STATIC_DIR") {
        if !dir.trim().is_empty() {
            candidates.push(PathBuf::from(dir));
        }
    }

    candidates.push(PathBuf::from("/tmp/opencode/vgmstream/build/src"));
    candidates.push(PathBuf::from("/usr/local/lib"));
    candidates.push(PathBuf::from("/usr/lib"));

    let lib_dir = candidates
        .into_iter()
        .find(|dir| dir.join("libvgmstream.a").exists())
        .unwrap_or_else(|| {
            panic!(
                "libvgmstream.a not found. Set VGMSTREAM_STATIC_DIR to the directory containing libvgmstream.a"
            )
        });

    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    println!("cargo:rustc-link-lib=static=vgmstream");

    if let Some(build_root) = lib_dir.parent() {
        let vorbis_dir = build_root.join("dependencies/vorbis/lib");
        let ogg_dir = build_root.join("dependencies/ogg");
        if vorbis_dir.join("libvorbisfile.so").exists() {
            println!("cargo:rustc-link-search=native={}", vorbis_dir.display());
            println!("cargo:rustc-link-lib=dylib=vorbisfile");
            println!("cargo:rustc-link-lib=dylib=vorbis");
            if cfg!(target_os = "linux") {
                println!("cargo:rustc-link-arg=-Wl,-rpath,{}", vorbis_dir.display());
            }
        }
        if ogg_dir.join("libogg.so").exists() {
            println!("cargo:rustc-link-search=native={}", ogg_dir.display());
            println!("cargo:rustc-link-lib=dylib=ogg");
            if cfg!(target_os = "linux") {
                println!("cargo:rustc-link-arg=-Wl,-rpath,{}", ogg_dir.display());
            }
        }
    }

    if cfg!(target_os = "linux") {
        println!("cargo:rustc-link-lib=dylib=m");
        println!("cargo:rustc-link-lib=dylib=stdc++");
    } else if cfg!(target_os = "macos") {
        println!("cargo:rustc-link-lib=dylib=c++");
    }
}
