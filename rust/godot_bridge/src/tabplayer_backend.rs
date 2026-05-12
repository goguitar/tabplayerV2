use crate::common::*;

#[derive(GodotClass)]
#[class(base=RefCounted)]
struct TabPlayerBackend {
    #[base]
    _base: Base<RefCounted>,
}

#[godot_api]
impl IRefCounted for TabPlayerBackend {
    fn init(base: Base<RefCounted>) -> Self {
        Self { _base: base }
    }
}

#[godot_api]
impl TabPlayerBackend {
    #[func]
    fn import_psarc_dir(&self, dir: GString) -> VariantDict {
        self.rescan_internal(Some(PathBuf::from(dir.to_string())))
    }

    #[func]
    fn import_default_dlc(&self) -> VariantDict {
        self.rescan_internal(None)
    }

    #[func]
    fn import_psarc_files(&self, paths: PackedStringArray) -> VariantDict {
        if let Some(first) = paths.as_slice().first() {
            return self.import_psarc_dir(first.clone());
        }
        self.rescan_internal(None)
    }

    #[func]
    fn song_count(&self) -> i64 {
        let _ = ensure_song_catalog_loaded();
        catalog_list_song_files().len() as i64
    }

    #[func]
    fn list_song_folders(&self) -> PackedStringArray {
        let _ = ensure_song_catalog_loaded();
        let mut arr = PackedStringArray::new();
        for song in catalog_list_song_files() {
            arr.push(song.folder_name.as_str());
        }
        arr
    }

    #[func]
    fn load_song_summary(&self, folder: GString) -> VariantDict {
        let mut dict = VariantDict::new();
        if let Ok(song) = catalog_load_song_data(&folder.to_string()) {
            dict.set("ok", true);
            dict.set("name", song.metadata.name);
            dict.set("artist", song.metadata.artist);
            dict.set("instrument_count", song.instruments.len() as i64);
            let note_count: usize = song.instruments.iter().map(|x| x.notes.len()).sum();
            dict.set("note_count", note_count as i64);
            return dict;
        }

        dict.set("ok", false);
        dict.set("name", "");
        dict.set("artist", "");
        dict.set("instrument_count", 0_i64);
        dict.set("note_count", 0_i64);
        dict
    }

    #[func]
    fn song_audio_status(&self, folder: GString) -> VariantDict {
        let mut dict = VariantDict::new();
        let song_id = folder.to_string();
        match load_song_audio_stream(&song_id) {
            Ok(_) => {
                dict.set("ok", true);
                dict.set("error", "");
            }
            Err(err) => {
                dict.set("ok", false);
                dict.set("error", err.to_string());
            }
        }
        dict
    }

    #[func]
    fn song_art_status(&self, folder: GString) -> VariantDict {
        let mut dict = VariantDict::new();
        match catalog_load_song_album_art(&folder.to_string()) {
            Ok(Some(bytes)) => {
                let loadable = load_dds_texture_from_bytes(&bytes).is_some();
                dict.set("exists", true);
                dict.set("path", "psarc://album.dds");
                dict.set("compressed_loadable", false);
                dict.set("loadable", loadable);
                dict.set(
                    "load_error",
                    if loadable {
                        ""
                    } else {
                        "dds decode failed"
                    },
                );
            }
            Ok(None) => {
                dict.set("exists", false);
                dict.set("path", "");
                dict.set("compressed_loadable", false);
                dict.set("loadable", false);
                dict.set("load_error", "missing");
            }
            Err(err) => {
                dict.set("exists", false);
                dict.set("path", "");
                dict.set("compressed_loadable", false);
                dict.set("loadable", false);
                dict.set("load_error", err.to_string());
            }
        }

        dict
    }

    fn rescan_internal(&self, dir: Option<PathBuf>) -> VariantDict {
        let mut dict = VariantDict::new();
        let result = if let Some(dir) = dir {
            catalog_rescan_dlc_dir(&dir)
        } else {
            catalog_rescan_default_dlc()
        };
        match result {
            Ok(count) => {
                dict.set("ok", true);
                dict.set("completed", count as i64);
                dict.set("failed", 0_i64);
            }
            Err(err) => {
                dict.set("ok", false);
                dict.set("completed", 0_i64);
                dict.set("failed", 1_i64);
                dict.set("error", err.to_string());
            }
        }
        dict
    }
}
