pub(crate) use std::path::PathBuf;
use std::sync::Mutex;
pub(crate) use std::{cmp::Ordering};

use anyhow::anyhow;
pub(crate) use godot::classes::{
    AudioServer, AudioStreamPlayer, AudioStreamWav, BoxMesh, Button, Camera3D, CheckBox, ConfirmationDialog, Engine, Control,
    DirectionalLight3D, FileDialog, IControl, INode, INode2D, INode3D,
    IRefCounted, IVBoxContainer, Image, ImageTexture, Input, Label, LineEdit, MenuButton,
    Label3D, Material, Mesh, MeshInstance3D, Node, Node2D, Node3D, OptionButton, Os, PlaneMesh, RefCounted,
    RichTextLabel, StandardMaterial3D, TextureRect, Tree, VBoxContainer,
};
pub(crate) use godot::prelude::*;
use once_cell::sync::Lazy;
pub(crate) use tabplayer_parser::index::{
    song_root_folder,
};
pub(crate) use tabplayer_parser::models::{SongData, SongFile};
pub(crate) use tabplayer_parser::song_catalog::{
    ensure_default_loaded as ensure_song_catalog_loaded, list_song_files as catalog_list_song_files,
    load_song_album_art as catalog_load_song_album_art,
    load_song_audio_wav as catalog_load_song_audio_wav, load_song_data as catalog_load_song_data,
    rescan_default_dlc as catalog_rescan_default_dlc, rescan_dlc_dir as catalog_rescan_dlc_dir,
};
pub(crate) use tabplayer_sync::song_clock::SongClock;

static PENDING_SONG: Lazy<Mutex<Option<PendingSong>>> = Lazy::new(|| Mutex::new(None));

#[derive(Clone)]
pub(crate) struct PendingSong {
    pub(crate) song_id: String,
    pub(crate) instrument: String,
}

pub(crate) fn read_song_data(song_id: &str) -> Option<SongData> {
    catalog_load_song_data(song_id).ok()
}

pub(crate) fn load_song_audio_stream(song_id: &str) -> anyhow::Result<Gd<godot::classes::AudioStream>> {
    let wav_bytes = catalog_load_song_audio_wav(song_id)
        .map_err(|e| anyhow!("failed decoding audio from psarc: {e}"))?;
    let packed: PackedByteArray = wav_bytes.into_iter().collect();
    let stream = AudioStreamWav::load_from_buffer(&packed)
        .ok_or_else(|| anyhow!("godot failed loading wav from buffer"))?;
    Ok(stream.upcast())
}

pub(crate) fn load_dds_texture_from_bytes(bytes: &[u8]) -> Option<Gd<godot::classes::Texture2D>> {
    let packed: PackedByteArray = bytes.iter().copied().collect();
    let mut image = Image::new_gd();
    if image.load_dds_from_buffer(&packed) != godot::global::Error::OK {
        return None;
    }
    let tex = ImageTexture::create_from_image(&image)?;
    Some(tex.upcast())
}

pub(crate) fn set_pending_song(song_id: String, instrument: String) {
    if let Ok(mut lock) = PENDING_SONG.lock() {
        *lock = Some(PendingSong { song_id, instrument });
    }
}

pub(crate) fn take_pending_song() -> Option<PendingSong> {
    PENDING_SONG.lock().ok()?.take()
}

pub(crate) fn to_min_sec(value: f64) -> String {
    let min = (value / 60.0).floor() as i64;
    let sec = (value % 60.0).floor() as i64;
    format!("{min}m {sec:02}s")
}

pub(crate) fn calc_tuning_name(tuning: &[i16]) -> String {
    match tuning {
        [1, 1, 1, 1, 1, 1] => "F Standard".to_string(),
        [0, 0, 0, 0, 0, 0] => "E Standard".to_string(),
        [-2, 0, 0, 0, 0, 0] => "Drop D".to_string(),
        [-1, -1, -1, -1, -1, -1] => "Eb Standard".to_string(),
        [-3, -1, -1, -1, -1, -1] => "Eb Drop Db".to_string(),
        [-2, -2, -2, -2, -2, -2] => "D Standard".to_string(),
        [-4, -2, -2, -2, -2, -2] => "D Drop C".to_string(),
        [-3, -3, -3, -3, -3, -3] => "Db Standard".to_string(),
        [-5, -3, -3, -3, -3, -3] => "Db Drop B".to_string(),
        [-4, -4, -4, -4, -4, -4] => "C Standard".to_string(),
        [-6, -4, -4, -4, -4, -4] => "C Drop Bb".to_string(),
        [-5, -5, -5, -5, -5, -5] => "B Standard".to_string(),
        _ => {
            const NOTE_OFFSET: [i32; 6] = [0, 5, 10, 3, 7, 0];
            const NOTE_LIST: [&str; 12] = ["E", "F", "Gb", "G", "Ab", "A", "Bb", "B", "C", "Db", "D", "Eb"];
            tuning
                .iter()
                .enumerate()
                .map(|(i, offset)| {
                    let mut pos = NOTE_OFFSET.get(i).copied().unwrap_or(0) + *offset as i32;
                    while pos < 0 {
                        pos += NOTE_LIST.len() as i32;
                    }
                    let note = NOTE_LIST[pos as usize % NOTE_LIST.len()];
                    if i == 5 {
                        note.to_ascii_lowercase()
                    } else {
                        note.to_string()
                    }
                })
                .collect::<Vec<_>>()
                .join(",")
        }
    }
}

pub(crate) fn is_hidden_instrument_name(name: &str) -> bool {
    let lowered = name.to_ascii_lowercase();
    lowered == "showlights" || lowered == "vocals"
}

pub(crate) fn instrument_order_key(name: &str) -> i32 {
    match name.to_ascii_lowercase().as_str() {
        "lead" => 0,
        "lead1" => 1,
        "lead2" => 2,
        "rhythm" => 8,
        "rhythm1" => 9,
        "rhythm2" => 10,
        "bass" => 11,
        "bass1" => 12,
        "bass2" => 13,
        _ => 999,
    }
}

pub(crate) type VariantDict = Dictionary<Variant, Variant>;
