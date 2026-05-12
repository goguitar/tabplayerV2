#![allow(non_snake_case)]

use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use godot::classes::{
    AudioServer, AudioStreamOggVorbis, AudioStreamPlayer, AudioStreamWav, BoxMesh, Button, Camera3D, CheckBox, ConfirmationDialog, Engine,
    CompressedTexture2D, Control, DirectionalLight3D, FileDialog, IControl, INode, INode2D, INode3D,
    IRefCounted, IVBoxContainer, Image, ImageTexture, ItemList, Label, LineEdit, MenuButton,
    Label3D, Material, Mesh, MeshInstance3D, Node, Node2D, Node3D, OptionButton, Os, PlaneMesh, RefCounted,
    RichTextLabel, StandardMaterial3D, TextureRect, VBoxContainer,
};
use godot::prelude::*;
use once_cell::sync::Lazy;
use tabplayer_parser::index::{
    import_psarc_files as parser_import_psarc_files, read_song_file_list, song_root_folder,
};
use tabplayer_parser::models::{SongData, SongFile};
use tabplayer_parser::song_loader::load_song;
use tabplayer_sync::song_clock::SongClock;

static PENDING_SONG: Lazy<Mutex<Option<PendingSong>>> = Lazy::new(|| Mutex::new(None));

#[derive(Clone)]
struct PendingSong {
    folder: String,
    instrument: String,
}

fn read_song_data(folder: &str) -> Option<SongData> {
    load_song(folder).ok()
}

fn find_song_audio_file(folder: &str) -> Option<PathBuf> {
    let song_dir = song_root_folder().join(folder);
    let entries = fs::read_dir(song_dir).ok()?;

    let mut preferred_ogg: Option<PathBuf> = None;
    let mut first_wav: Option<PathBuf> = None;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        match path.extension().and_then(|x| x.to_str()) {
            Some("ogg") => {
                if path.file_name().and_then(|x| x.to_str()).is_some_and(|x| x.eq_ignore_ascii_case("song.ogg")) {
                    return Some(path);
                }
                if preferred_ogg.is_none() {
                    preferred_ogg = Some(path);
                }
            }
            Some("wav") if first_wav.is_none() => first_wav = Some(path),
            _ => {}
        }
    }

    preferred_ogg.or(first_wav)
}

fn load_dds_texture(path: &PathBuf) -> Option<Gd<godot::classes::Texture2D>> {
    let mut compressed = CompressedTexture2D::new_gd();
    if compressed.load(&path.to_string_lossy().to_string()) == godot::global::Error::OK {
        let tex2d: Gd<godot::classes::Texture2D> = compressed.upcast();
        return Some(tex2d);
    }

    let bytes = fs::read(path).ok()?;
    let packed: PackedByteArray = bytes.into_iter().collect();
    let mut image = Image::new_gd();
    if image.load_dds_from_buffer(&packed) != godot::global::Error::OK {
        return None;
    }
    let tex = ImageTexture::create_from_image(&image)?;
    Some(tex.upcast())
}

fn collect_psarc_files_recursive(dir: &PathBuf) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else {
        return files;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            files.extend(collect_psarc_files_recursive(&path));
            continue;
        }
        if path.extension().and_then(|x| x.to_str()) == Some("psarc") {
            files.push(path);
        }
    }

    files
}

fn set_pending_song(folder: String, instrument: String) {
    if let Ok(mut lock) = PENDING_SONG.lock() {
        *lock = Some(PendingSong { folder, instrument });
    }
}

fn take_pending_song() -> Option<PendingSong> {
    PENDING_SONG.lock().ok()?.take()
}

fn to_min_sec(value: f64) -> String {
    let min = (value / 60.0).floor() as i64;
    let sec = (value % 60.0).floor() as i64;
    format!("{min}m {sec:02}s")
}

fn calc_tuning_name(tuning: &[i16]) -> String {
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

fn is_hidden_instrument_name(name: &str) -> bool {
    let lowered = name.to_ascii_lowercase();
    lowered == "showlights" || lowered == "vocals"
}

fn instrument_order_key(name: &str) -> i32 {
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

type VariantDict = Dictionary<Variant, Variant>;

#[derive(GodotClass)]
#[class(base=Node)]
struct MainScene {
    #[base]
    base: Base<Node>,
}

#[godot_api]
impl INode for MainScene {
    fn init(base: Base<Node>) -> Self {
        Self { base }
    }

    fn ready(&mut self) {
        let mut tree = self.base().get_tree();
        let _ = tree.change_scene_to_file("res://scenes/StartMenu.tscn");
    }
}

#[derive(GodotClass)]
#[class(base=Control)]
struct StartMenu {
    #[base]
    base: Base<Control>,
    menu_animating: bool,
    menu_target_x: f32,
}

#[godot_api]
impl IControl for StartMenu {
    fn init(base: Base<Control>) -> Self {
        Self {
            base,
            menu_animating: false,
            menu_target_x: 80.0,
        }
    }

    fn ready(&mut self) {
        let song_count = read_song_file_list().data.len();
        let mut label = self.base().get_node_as::<Label>("%SongCountLabel");
        let text = format!("{song_count} songs");
        label.set_text(&text);

        let mut menu = self.base().get_node_as::<VBoxContainer>("VBoxContainer");
        let start_y = menu.get_position().y;
        let start_x = -menu.get_size().x.max(200.0);
        menu.set_position(Vector2::new(start_x, start_y));
        self.menu_target_x = 80.0;
        self.menu_animating = true;
    }

    fn process(&mut self, delta: f64) {
        if !self.menu_animating {
            return;
        }

        let mut menu = self.base().get_node_as::<VBoxContainer>("VBoxContainer");
        let pos = menu.get_position();
        let step = (delta as f32 * 8.0).clamp(0.0, 1.0);
        let new_x = pos.x + (self.menu_target_x - pos.x) * step;
        menu.set_position(Vector2::new(new_x, pos.y));

        if (new_x - self.menu_target_x).abs() < 0.5 {
            menu.set_position(Vector2::new(self.menu_target_x, pos.y));
            self.menu_animating = false;
        }
    }
}

#[godot_api]
impl StartMenu {
    #[func]
    fn PlayButton_Pressed(&mut self) {
        let mut tree = self.base().get_tree();
        let _ = tree.change_scene_to_file("res://scenes/SongPick.tscn");
    }

    #[func]
    fn InfoButton_Pressed(&mut self) {
        let mut tree = self.base().get_tree();
        let _ = tree.change_scene_to_file("res://scenes/InfoPage.tscn");
    }

    #[func]
    fn ConvertButton_Pressed(&mut self) {
        let mut tree = self.base().get_tree();
        let _ = tree.change_scene_to_file("res://scenes/ConvertMenu.tscn");
    }

    #[func]
    fn ReloadButton_Pressed(&mut self) {
        let song_count = read_song_file_list().data.len();
        let mut label = self.base().get_node_as::<Label>("%SongCountLabel");
        let text = format!("{song_count} songs");
        label.set_text(&text);
    }

    #[func]
    fn QuitButton_Pressed(&mut self) {
        let mut tree = self.base().get_tree();
        tree.quit();
    }

    #[func]
    fn SettingsButton_Pressed(&mut self) {
        let mut tree = self.base().get_tree();
        let _ = tree.change_scene_to_file("res://scenes/SettingsPage.tscn");
    }
}

#[derive(GodotClass)]
#[class(base=Control)]
struct SongPick {
    #[base]
    base: Base<Control>,
    songs: Vec<SongFile>,
    visible_indices: Vec<usize>,
    selected_index: Option<usize>,
    tuning_filter: String,
    search_filter: String,
    show_capo: bool,
    pending_confirm: Option<(String, String)>,
    display_instruments: Vec<String>,
}

#[godot_api]
impl IControl for SongPick {
    fn init(base: Base<Control>) -> Self {
        Self {
            base,
            songs: Vec::new(),
            visible_indices: Vec::new(),
            selected_index: None,
            tuning_filter: String::new(),
            search_filter: String::new(),
            show_capo: false,
            pending_confirm: None,
            display_instruments: Vec::new(),
        }
    }

    fn ready(&mut self) {
        self.songs = read_song_file_list().data;
        self.populate_tuning_filter();
        self.refresh_song_list();
    }
}

#[godot_api]
impl SongPick {
    fn get_main_instrument<'a>(&self, song: &'a SongFile) -> Option<&'a tabplayer_parser::models::SongFileInstrument> {
        song.instruments
            .iter()
            .filter(|x| !is_hidden_instrument_name(&x.name))
            .find(|x| x.is_main)
            .or_else(|| {
                song.instruments
                    .iter()
                    .find(|x| !is_hidden_instrument_name(&x.name))
            })
    }

    fn populate_tuning_filter(&mut self) {
        let mut tuning_option = self
            .base()
            .get_node_as::<OptionButton>("MarginContainer/VBoxContainer/FilterHBox/TuningOptionButton");
        tuning_option.clear();
        tuning_option.add_item("");

        let mut tunings = self
            .songs
            .iter()
            .filter_map(|song| self.get_main_instrument(song))
            .map(|inst| calc_tuning_name(&inst.tuning))
            .collect::<Vec<_>>();
        tunings.sort();
        tunings.dedup();
        for tuning in tunings {
            tuning_option.add_item(&tuning);
        }
        tuning_option.select(0);
    }

    fn refresh_song_list(&mut self) {
        self.visible_indices.clear();

        for (idx, song) in self.songs.iter().enumerate() {
            if self.song_visible(song) {
                self.visible_indices.push(idx);
            }
        }

        let mut list = self
            .base()
            .get_node_as::<ItemList>("MarginContainer/VBoxContainer/ContentSplit/SongsItemList");
        list.clear();

        for song_idx in &self.visible_indices {
            let song = &self.songs[*song_idx];
            let text = format!("{} - {}", song.artist, song.song_name);
            list.add_item(&text);
        }

        let mut shown = self
            .base()
            .get_node_as::<Label>("MarginContainer/VBoxContainer/FilterHBox/SongsLoadedLabel");
        shown.set_text(&format!("{} songs shown", self.visible_indices.len()));

        if self.visible_indices.is_empty() {
            self.selected_index = None;
            return;
        }

        let selected_song_idx = self
            .selected_index
            .and_then(|selected| self.visible_indices.iter().position(|x| *x == selected))
            .unwrap_or(0);
        list.select(selected_song_idx as i32);
        let song_idx = self.visible_indices[selected_song_idx];
        self.selected_index = Some(song_idx);
        self.update_selected_song_ui(song_idx);
    }

    fn song_visible(&self, song: &SongFile) -> bool {
        if !self.search_filter.is_empty() {
            let q = self.search_filter.to_lowercase();
            let hay = format!("{} {} {}", song.artist, song.song_name, song.album).to_lowercase();
            if !hay.contains(&q) {
                return false;
            }
        }

        let Some(main) = self.get_main_instrument(song) else {
            return false;
        };

        if !self.tuning_filter.is_empty() {
            let tuning = calc_tuning_name(&main.tuning);
            if tuning != self.tuning_filter {
                return false;
            }
        }

        if !self.show_capo && main.capo_fret > 0.0 {
            return false;
        }

        true
    }

    fn update_selected_song_ui(&mut self, index: usize) {
        let Some(song) = self.songs.get(index) else {
            return;
        };

        let mut artist = self
            .base()
            .get_node_as::<Label>("MarginContainer/VBoxContainer/ContentSplit/DetailsVBox/ArtistLabel");
        artist.set_text(&format!("Artist: {}", song.artist));

        let mut name = self
            .base()
            .get_node_as::<Label>("MarginContainer/VBoxContainer/ContentSplit/DetailsVBox/SongNameLabel");
        name.set_text(&format!("Name: {}", song.song_name));

        let mut album = self
            .base()
            .get_node_as::<Label>("MarginContainer/VBoxContainer/ContentSplit/DetailsVBox/AlbumLabel");
        album.set_text(&format!("Album: {}", song.album));

        let mut year = self
            .base()
            .get_node_as::<Label>("MarginContainer/VBoxContainer/ContentSplit/DetailsVBox/YearLabel");
        year.set_text(&format!(
            "Year: {}",
            song.year.map(|x| x.to_string()).unwrap_or_else(|| "?".to_string())
        ));

        let mut other = self
            .base()
            .get_node_as::<Label>("MarginContainer/VBoxContainer/ContentSplit/DetailsVBox/OtherLabel");
        other.set_text(&format!("Length: {}", to_min_sec(song.length)));

        let mut inst_count = self
            .base()
            .get_node_as::<Label>("MarginContainer/VBoxContainer/ContentSplit/DetailsVBox/InstrumentCountLabel");
        let playable_count = song
            .instruments
            .iter()
            .filter(|x| !is_hidden_instrument_name(&x.name))
            .count();
        inst_count.set_text(&format!("Instruments: {}", playable_count));

        let playable_instruments = song
            .instruments
            .iter()
            .filter(|x| !is_hidden_instrument_name(&x.name))
            .collect::<Vec<_>>();
        let mut playable_instruments = playable_instruments;
        playable_instruments.sort_by_key(|x| instrument_order_key(&x.name));

        self.display_instruments = playable_instruments
            .iter()
            .map(|x| x.name.clone())
            .collect();
        for i in 0..8 {
            let button_path = format!(
                "MarginContainer/VBoxContainer/ContentSplit/DetailsVBox/InstrumentGridContainer/InstrumentButton{}",
                i
            );
            let tuning_path = format!(
                "MarginContainer/VBoxContainer/ContentSplit/DetailsVBox/InstrumentGridContainer/InstrumentTuning{}",
                i
            );
            let count_path = format!(
                "MarginContainer/VBoxContainer/ContentSplit/DetailsVBox/InstrumentGridContainer/InstrumentCount{}",
                i
            );
            let density_path = format!(
                "MarginContainer/VBoxContainer/ContentSplit/DetailsVBox/InstrumentGridContainer/InstrumentDensity{}",
                i
            );

            let mut button = self.base().get_node_as::<Button>(&button_path);
            let mut tuning = self.base().get_node_as::<Label>(&tuning_path);
            let mut count = self.base().get_node_as::<Label>(&count_path);
            let mut density = self.base().get_node_as::<Label>(&density_path);

            if let Some(inst) = self.display_instruments.get(i) {
                let inst_data = playable_instruments[i];
                let note_count = inst_data.note_count as f64;
                let song_len = if song.length > 0.0 { song.length } else { 1.0 };
                let note_density = note_count / song_len;

                button.set_visible(true);
                button.set_text(&format!("Play {}", inst));
                tuning.set_visible(true);
                tuning.set_text(&calc_tuning_name(&inst_data.tuning));
                count.set_visible(true);
                count.set_text(&format!("{}", inst_data.note_count));
                density.set_visible(true);
                density.set_text(&format!("{:.2}", note_density));
            } else {
                button.set_visible(false);
                button.set_text("Play");
                tuning.set_visible(false);
                tuning.set_text("");
                count.set_visible(false);
                count.set_text("");
                density.set_visible(false);
                density.set_text("");
            }
        }

        let song_dir = song_root_folder().join(&song.folder_name);
        let album_art = ["album.dds"]
            .iter()
            .map(|name| song_dir.join(name))
            .find(|p| p.exists())
            .or_else(|| {
                fs::read_dir(&song_dir)
                    .ok()?
                    .flatten()
                    .map(|e| e.path())
                    .find(|p| {
                        p.extension()
                            .and_then(|x| x.to_str())
                            .is_some_and(|x| x.eq_ignore_ascii_case("dds"))
                    })
            });
        let mut album_tex = self.base().get_node_as::<TextureRect>(
            "MarginContainer/VBoxContainer/ContentSplit/DetailsVBox/AlbumArtTextureRect",
        );
        if let Some(album_art) = album_art {
            if let Some(tex2d) = load_dds_texture(&album_art) {
                let _ = album_tex.call("set_texture", &[tex2d.to_variant()]);
                return;
            }
        }
        let _ = album_tex.call("set_texture", &[Variant::nil()]);
    }

    #[func]
    fn Back(&mut self) {
        let mut tree = self.base().get_tree();
        let _ = tree.change_scene_to_file("res://scenes/StartMenu.tscn");
    }

    #[func]
    fn ConfirmedInstrumentTuningIsDiff(&mut self) {
        let Some((folder, instrument)) = self.pending_confirm.take() else {
            return;
        };
        set_pending_song(folder, instrument);
        let mut tree = self.base().get_tree();
        let _ = tree.change_scene_to_file("res://scenes/SongScene.tscn");
    }

    #[func]
    fn SongSelected(&mut self, index: i64) {
        if index < 0 {
            return;
        }
        let visible_idx = index as usize;
        let Some(song_idx) = self.visible_indices.get(visible_idx).copied() else {
            return;
        };
        self.selected_index = Some(song_idx);
        self.update_selected_song_ui(song_idx);
    }

    #[func]
    fn SongActivated(&mut self, index: i64) {
        if index < 0 {
            return;
        }

        let visible_idx = index as usize;
        let Some(song_idx) = self.visible_indices.get(visible_idx).copied() else {
            return;
        };
        self.selected_index = Some(song_idx);
        self.play_selected_instrument_inner();
    }

    #[func]
    fn SelectRandom(&mut self) {
        if self.visible_indices.is_empty() {
            return;
        }
        let tick = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|x| x.subsec_nanos() as usize)
            .unwrap_or(0);
        let pick = tick % self.visible_indices.len();

        let mut list = self
            .base()
            .get_node_as::<ItemList>("MarginContainer/VBoxContainer/ContentSplit/SongsItemList");
        list.select(pick as i32);

        let song_idx = self.visible_indices[pick];
        self.selected_index = Some(song_idx);
        self.update_selected_song_ui(song_idx);
    }

    #[func]
    fn UpdateFilter(&mut self, filter: GString) {
        self.search_filter = filter.to_string();
        self.refresh_song_list();
    }

    #[func]
    fn TuningSelected(&mut self, index: i64) {
        let tuning = self
            .base()
            .get_node_as::<OptionButton>("MarginContainer/VBoxContainer/FilterHBox/TuningOptionButton")
            .get_item_text(index as i32)
            .to_string();
        self.tuning_filter = tuning;
        self.refresh_song_list();
    }

    #[func]
    fn ShowCapo_Pressed(&mut self) {
        let checked = self
            .base()
            .get_node_as::<CheckBox>("MarginContainer/VBoxContainer/FilterHBox/CapoCheckBox")
            .is_pressed();
        self.show_capo = checked;
        self.refresh_song_list();
    }

    fn play_selected_instrument_inner(&mut self) {
        let Some(idx) = self.selected_index else {
            return;
        };

        let Some(song) = self.songs.get(idx) else {
            return;
        };

        let instrument = song
            .instruments
            .iter()
            .filter(|x| !is_hidden_instrument_name(&x.name))
            .find(|x| x.is_main)
            .or_else(|| {
                song.instruments
                    .iter()
                    .find(|x| !is_hidden_instrument_name(&x.name))
            })
            .map(|x| x.name.clone())
            .unwrap_or_else(|| "lead".to_string());

        if !self.tuning_filter.is_empty() {
            if let Some(inst) = song
                .instruments
                .iter()
                .find(|x| x.name == instrument && !is_hidden_instrument_name(&x.name))
            {
                let inst_tuning = calc_tuning_name(&inst.tuning);
                if inst_tuning != self.tuning_filter {
                    let mut dialog = self.base().get_node_as::<ConfirmationDialog>("TuningConfirmationDialog");
                    let text = format!(
                        "Instrument tuning ({}) is different to song filter ({})\nAre you sure?",
                        inst_tuning, self.tuning_filter
                    );
                    let _ = dialog.call("set_dialog_text", &[text.to_variant()]);
                    dialog.popup_centered();
                    self.pending_confirm = Some((song.folder_name.clone(), instrument));
                    return;
                }
            }
        }

        set_pending_song(song.folder_name.clone(), instrument);
        let mut tree = self.base().get_tree();
        let _ = tree.change_scene_to_file("res://scenes/SongScene.tscn");
    }

    fn play_instrument_by_button_index(&mut self, button_index: usize) {
        let Some(idx) = self.selected_index else {
            return;
        };
        let Some(song) = self.songs.get(idx) else {
            return;
        };
        let Some(instrument) = self.display_instruments.get(button_index).cloned() else {
            return;
        };

        if !self.tuning_filter.is_empty() {
            if let Some(inst) = song
                .instruments
                .iter()
                .find(|x| x.name == instrument && !is_hidden_instrument_name(&x.name))
            {
                let inst_tuning = calc_tuning_name(&inst.tuning);
                if inst_tuning != self.tuning_filter {
                    let mut dialog = self.base().get_node_as::<ConfirmationDialog>("TuningConfirmationDialog");
                    let text = format!(
                        "Instrument tuning ({}) is different to song filter ({})\nAre you sure?",
                        inst_tuning, self.tuning_filter
                    );
                    let _ = dialog.call("set_dialog_text", &[text.to_variant()]);
                    dialog.popup_centered();
                    self.pending_confirm = Some((song.folder_name.clone(), instrument));
                    return;
                }
            }
        }

        set_pending_song(song.folder_name.clone(), instrument);
        let mut tree = self.base().get_tree();
        let _ = tree.change_scene_to_file("res://scenes/SongScene.tscn");
    }

    #[func]
    fn InstrumentButton0_Pressed(&mut self) { self.play_instrument_by_button_index(0); }
    #[func]
    fn InstrumentButton1_Pressed(&mut self) { self.play_instrument_by_button_index(1); }
    #[func]
    fn InstrumentButton2_Pressed(&mut self) { self.play_instrument_by_button_index(2); }
    #[func]
    fn InstrumentButton3_Pressed(&mut self) { self.play_instrument_by_button_index(3); }
    #[func]
    fn InstrumentButton4_Pressed(&mut self) { self.play_instrument_by_button_index(4); }
    #[func]
    fn InstrumentButton5_Pressed(&mut self) { self.play_instrument_by_button_index(5); }
    #[func]
    fn InstrumentButton6_Pressed(&mut self) { self.play_instrument_by_button_index(6); }
    #[func]
    fn InstrumentButton7_Pressed(&mut self) { self.play_instrument_by_button_index(7); }
}

#[derive(GodotClass)]
#[class(base=Node)]
struct SongScene {
    #[base]
    base: Base<Node>,
    clock: SongClock,
    loop_a: Option<f64>,
    loop_b: Option<f64>,
    song_data: SongData,
    instrument_name: String,
    folder_name: String,
    has_audio_stream: bool,
    instrument_menu_names: Vec<String>,
    guitar_chart_root: Option<Gd<Node3D>>,
    guitar_camera: Option<Gd<Camera3D>>,
    song_chart_root: Option<Gd<Node3D>>,
    song_chart_nodes: Vec<Gd<Node3D>>,
    last_note_block_time: Option<f64>,
    last_note_block_node: Option<Gd<Node3D>>,
    last_chord_block: Option<tabplayer_parser::models::NoteBlock>,
    cached_song_position: Option<f64>,
}

#[godot_api]
impl INode for SongScene {
    fn init(base: Base<Node>) -> Self {
        Self {
            base,
            clock: SongClock::new(48_000),
            loop_a: None,
            loop_b: None,
            song_data: SongData::default(),
            instrument_name: String::new(),
            folder_name: String::new(),
            has_audio_stream: false,
            instrument_menu_names: Vec::new(),
            guitar_chart_root: None,
            guitar_camera: None,
            song_chart_root: None,
            song_chart_nodes: Vec::new(),
            last_note_block_time: None,
            last_note_block_node: None,
            last_chord_block: None,
            cached_song_position: None,
        }
    }

    fn ready(&mut self) {
        if let Some(pending) = take_pending_song() {
            self.folder_name = pending.folder.clone();
            self.instrument_name = pending.instrument;
            if let Some(data) = read_song_data(&self.folder_name) {
                self.song_data = data;
            }
        } else {
            let list = read_song_file_list();
            if let Some(first_song) = list.data.first() {
                self.folder_name = first_song.folder_name.clone();
                if let Some(data) = read_song_data(&self.folder_name) {
                    self.instrument_name = data
                        .instruments
                        .iter()
                        .find(|x| !is_hidden_instrument_name(&x.name))
                        .map(|x| x.name.clone())
                        .unwrap_or_else(|| "lead".to_string());
                    self.song_data = data;
                }
            }
        }

        self.clock.play();
        self.setup_audio_playback();
        self.setup_guitar_chart();
        self.load_instrument_from_state();
        self.populate_instrument_menu();
        self.update_labels();
    }

    fn process(&mut self, delta: f64) {
        self.cached_song_position = None;
        if self.has_audio_stream {
            let song_time = self.get_song_position();
            self.clock.seek_seconds(song_time);
        } else {
            self.clock.tick(delta);
        }
        let mut song_time = self.clock.song_time_seconds();

        if let (Some(a), Some(b)) = (self.loop_a, self.loop_b) {
            if b > a && a < song_time && delta + song_time > b {
                self.clock.seek_seconds(a);
                song_time = a;
                if self.has_audio_stream {
                    let mut player = self.base().get_node_as::<AudioStreamPlayer>("AudioStreamPlayer");
                    player.seek(a as f32);
                }
            }
        }

        if self.song_data.metadata.song_length > 0.0 && song_time > self.song_data.metadata.song_length {
            self.clock.seek_seconds(self.song_data.metadata.song_length);
            self.clock.pause();
            song_time = self.song_data.metadata.song_length;
        }

        self.update_guitar_chart(delta, song_time);

        let mut running = self.base().get_node_as::<Label>("RunningDetailsLabel");
        let running_text = format!("{}fps | {:05.1}ms\n{}", Engine::singleton().get_frames_per_second(), delta * 1000.0, to_min_sec_msec(song_time, true));
        running.set_text(&running_text);

        let mut pos = self
            .base()
            .get_node_as::<LineEdit>("GridContainer/PositionSetLineEdit");
        let pos_text = to_min_sec_msec(song_time, true);
        pos.set_text(&pos_text);

        let mut speed_label = self.base().get_node_as::<Label>("GridContainer/SongSpeedLabel");
        let speed_text = format!("{:.1}%", self.clock.speed * 100.0);
        speed_label.set_text(&speed_text);

        let mut a_label = self.base().get_node_as::<Label>("GridContainer/ABLabelStart");
        let a_text = self.loop_a.map(|x| to_min_sec_msec(x, true)).unwrap_or_default();
        a_label.set_text(&a_text);

        let mut b_label = self.base().get_node_as::<Label>("GridContainer/ABLabelEnd");
        let b_text = self.loop_b.map(|x| to_min_sec_msec(x, true)).unwrap_or_default();
        b_label.set_text(&b_text);

        self.update_details_label(song_time);
        self.update_lyrics(song_time);
    }
}

#[godot_api]
impl SongScene {
    fn current_instrument(&self) -> Option<&tabplayer_parser::models::SongInstrument> {
        self.song_data
            .instruments
            .iter()
            .filter(|x| !is_hidden_instrument_name(&x.name))
            .find(|x| x.name == self.instrument_name)
            .or_else(|| {
                self.song_data
                    .instruments
                    .iter()
                    .filter(|x| !is_hidden_instrument_name(&x.name))
                    .min_by_key(|x| instrument_order_key(&x.name))
            })
    }

    fn setup_guitar_chart(&mut self) {
        if self.guitar_chart_root.is_some() {
            return;
        }

        let mut root = Node3D::new_alloc();
        root.set_name("GuitarChartRoot");
        let root_node: Gd<Node> = root.clone().upcast();
        self.base_mut().add_child(&root_node);

        let mut material = StandardMaterial3D::new_gd();
        material.set_albedo(Color::from_rgb(0.82, 0.71, 0.55));
        let mut plane_mesh = PlaneMesh::new_gd();
        plane_mesh.set_size(Vector2::new(6.0, 6.0));
        plane_mesh.set_center_offset(Vector3::new(2.5, 0.0, -3.0));
        let plane_material: Gd<Material> = material.upcast();
        plane_mesh.set_material(&plane_material);
        let mut plane = MeshInstance3D::new_alloc();
        let plane_mesh_up: Gd<Mesh> = plane_mesh.upcast();
        plane.set_mesh(&plane_mesh_up);
        plane.set_transform(Transform3D::new(
            Basis::from_rows(
                Vector3::new(0.0, -1.0, 0.0),
                Vector3::new(1.0, 0.0, 0.0),
                Vector3::new(0.0, 0.0, 1.0),
            ),
            Vector3::ZERO,
        ));
        let plane_node: Gd<Node> = plane.upcast();
        root.add_child(&plane_node);

        let mut camera = Camera3D::new_alloc();
        camera.set_fov(60.0);
        camera.set_transform(Transform3D::new(
            Basis::from_rows(
                Vector3::new(0.0, 0.310809, -0.950472),
                Vector3::new(0.0, 0.950472, 0.310809),
                Vector3::new(1.0, 0.0, 0.0),
            ),
            Vector3::new(-12.0, 10.0, 8.0),
        ));
        let _ = camera.call("make_current", &[]);
        self.guitar_camera = Some(camera.clone());
        let camera_node: Gd<Node> = camera.upcast();
        root.add_child(&camera_node);

        let mut light = DirectionalLight3D::new_alloc();
        light.set_transform(Transform3D::new(
            Basis::from_rows(
                Vector3::new(-0.177838, 0.752991, -0.633544),
                Vector3::new(-0.317607, 0.565433, 0.761191),
                Vector3::new(0.931397, 0.336587, 0.1386),
            ),
            Vector3::ZERO,
        ));
        let light_node: Gd<Node> = light.upcast();
        root.add_child(&light_node);

        for string in 0..6 {
            let mut str_mesh = MeshInstance3D::new_alloc();
            let mut box_mesh = BoxMesh::new_gd();
            box_mesh.set_size(Vector3::new(0.08, 0.08, 50.0));
            let mut mat = StandardMaterial3D::new_gd();
            mat.set_albedo(string_color(string));
            let line_material: Gd<Material> = mat.upcast();
            box_mesh.set_material(&line_material);
            let string_mesh: Gd<Mesh> = box_mesh.upcast();
            str_mesh.set_mesh(&string_mesh);
            str_mesh.set_position(Vector3::new(0.0, calc_note_height_y(string), 25.0));
            let string_node: Gd<Node> = str_mesh.upcast();
            root.add_child(&string_node);
        }

        for fret in 0..25 {
            let mut fret_mesh = MeshInstance3D::new_alloc();
            let mut box_mesh = BoxMesh::new_gd();
            box_mesh.set_size(Vector3::new(0.03, 5.0 + calc_track_bottom_world().abs() * 2.0, 0.03));
            let mut mat = StandardMaterial3D::new_gd();
            mat.set_albedo(Color::from_rgb(0.82, 0.71, 0.55));
            let fret_material: Gd<Material> = mat.upcast();
            box_mesh.set_material(&fret_material);
            let fret_mesh_res: Gd<Mesh> = box_mesh.upcast();
            fret_mesh.set_mesh(&fret_mesh_res);
            fret_mesh.set_position(Vector3::new(0.0, 2.5, calc_fret_pos_z(fret)));
            let fret_node: Gd<Node> = fret_mesh.upcast();
            root.add_child(&fret_node);

            let mut path_mesh = MeshInstance3D::new_alloc();
            let mut path_box = BoxMesh::new_gd();
            path_box.set_size(Vector3::new(0.03, 6.0, 0.03));
            let mut path_mat = StandardMaterial3D::new_gd();
            path_mat.set_albedo(Color::from_rgb(0.33, 0.33, 0.33));
            let path_material: Gd<Material> = path_mat.upcast();
            path_box.set_material(&path_material);
            let path_mesh_res: Gd<Mesh> = path_box.upcast();
            path_mesh.set_mesh(&path_mesh_res);
            path_mesh.set_transform(Transform3D::new(
                Basis::from_rows(
                    Vector3::new(0.0, 5.0, 0.0),
                    Vector3::new(-1.0, 0.0, 0.0),
                    Vector3::new(0.0, 0.0, 1.0),
                ),
                Vector3::new(15.0, calc_track_bottom_world(), calc_fret_pos_z(fret)),
            ));
            let path_node: Gd<Node> = path_mesh.upcast();
            root.add_child(&path_node);
        }

        for fret_label in [3_i32, 5, 7, 9, 12, 15, 17, 19, 21, 24] {
            let mut label = Label3D::new_alloc();
            label.set_text(&fret_label.to_string());
            label.set_font_size(200);
            label.set_transform(Transform3D::new(
                Basis::from_rows(
                    Vector3::new(0.0, 1.0, 0.0),
                    Vector3::new(0.0, 0.0, 1.0),
                    Vector3::new(1.0, 0.0, 0.0),
                ),
                Vector3::new(-0.25, calc_track_bottom_world(), calc_in_fret_pos_z(fret_label)),
            ));
            root.add_child(&label.clone().upcast::<Node>());
        }

        self.guitar_chart_root = Some(root);
    }

    fn load_instrument_from_state(&mut self) {
        if let Some(mut last) = self.last_note_block_node.take() {
            last.queue_free();
        }
        self.last_note_block_time = None;
        self.rebuild_song_chart();
    }

    fn clear_song_chart(&mut self) {
        for mut node in self.song_chart_nodes.drain(..) {
            node.queue_free();
        }
        if let Some(mut node) = self.song_chart_root.take() {
            node.queue_free();
        }
    }

    fn rebuild_song_chart(&mut self) {
        self.clear_song_chart();
        self.last_chord_block = None;

        let Some(instrument) = self.current_instrument().cloned() else {
            return;
        };

        let mut chart_root = Node3D::new_alloc();
        chart_root.set_name("SongChartRoot");

        for block in &instrument.notes {
            if block.is_chord() {
                self.add_chord_block(&mut chart_root, &instrument, block);
                self.last_chord_block = Some(block.clone());
            } else {
                self.add_single_note_block(&mut chart_root, &instrument, block);
                self.last_chord_block = None;
            }
        }

        for node in generate_note_block_frets(&instrument) {
            chart_root.add_child(&node.clone().upcast::<Node>());
            self.song_chart_nodes.push(node);
        }

        let chart_node: Gd<Node> = chart_root.clone().upcast();
        self.base_mut().add_child(&chart_node);
        self.song_chart_root = Some(chart_root);
    }

    fn add_single_note_block(&mut self, chart_root: &mut Gd<Node3D>, instrument: &tabplayer_parser::models::SongInstrument, block: &tabplayer_parser::models::NoteBlock) {
        if block.notes.is_empty() {
            return;
        }
        let note = &block.notes[0];
        for node in make_note_geometry(note, &instrument.config, block) {
            chart_root.add_child(&node.clone().upcast::<Node>());
            self.song_chart_nodes.push(node);
        }

        if note.fret_num != 0 {
            let note_pos = Vector3::new(
                block.time as f32 * instrument.config.note_speed as f32,
                calc_note_height_y(note.string_num),
                calc_in_fret_pos_z(note.fret_num),
            );
            let start = Vector3::new(note_pos.x, calc_track_bottom_world(), note_pos.z);
            let vertical = box_line(string_color(note.string_num), start, Vector3::new(note_pos.x, note_pos.y, note_pos.z));
            chart_root.add_child(&vertical.clone().upcast::<Node>());
            self.song_chart_nodes.push(vertical);

            let pos = Vector3::new(note_pos.x, calc_track_bottom_world() + 0.01, calc_fret_pos_z(note.fret_num - 1));
            let horizontal = box_line(
                string_color(note.string_num),
                pos,
                pos + Vector3::new(0.0, 0.0, calc_fret_width_z(note.fret_num, 1)),
            );
            chart_root.add_child(&horizontal.clone().upcast::<Node>());
            self.song_chart_nodes.push(horizontal);
        }
    }

    fn add_chord_block(&mut self, chart_root: &mut Gd<Node3D>, instrument: &tabplayer_parser::models::SongInstrument, block: &tabplayer_parser::models::NoteBlock) {
        let is_same_chord = self
            .last_chord_block
            .as_ref()
            .is_some_and(|last| is_same_chord_as(block, last) && (block.time - last.time).abs() <= 1.2);

        let is_mute = block
            .chord_flags
            .iter()
            .any(|x| *x == tabplayer_parser::models::NoteBlockFlags::MUTE);

        if is_mute {
            let line_start_z = calc_fret_pos_z(block.fret_window_start - 1);
            let across = calc_fret_width_z(block.fret_window_start, block.fret_window_length);
            let bottom_left = Vector3::new(
                block.time as f32 * instrument.config.note_speed as f32,
                calc_track_bottom_world() + 0.01,
                line_start_z,
            );
            let chord_dir = Vector3::new(0.0, 6.0 * calc_string_distance() * 0.5, 0.0);
            for node in [
                box_line(Color::from_rgb(0.83, 0.83, 0.83), bottom_left, bottom_left + Vector3::new(0.0, 0.0, across) + chord_dir),
                box_line(Color::from_rgb(0.83, 0.83, 0.83), bottom_left + Vector3::new(0.0, 0.0, across), bottom_left + chord_dir),
            ] {
                chart_root.add_child(&node.clone().upcast::<Node>());
                self.song_chart_nodes.push(node);
            }
        } else if !is_same_chord {
            for note in &block.notes {
                for node in make_note_geometry(note, &instrument.config, block) {
                    chart_root.add_child(&node.clone().upcast::<Node>());
                    self.song_chart_nodes.push(node);
                }
            }

            if let Some(label) = &block.label {
                if !label.trim().is_empty() {
                    let node = text_vertical(
                        label,
                        Vector3::new(
                            block.time as f32 * instrument.config.note_speed as f32,
                            7.0 * calc_string_distance(),
                            calc_in_fret_pos_z(block.fret_window_start),
                        ),
                    );
                    chart_root.add_child(&node.clone().upcast::<Node>());
                    self.song_chart_nodes.push(node);
                }
            }
        }

        let line_start_z = calc_fret_pos_z(block.fret_window_start - 1);
        let across = calc_fret_width_z(block.fret_window_start, block.fret_window_length);
        let bottom_left = Vector3::new(
            block.time as f32 * instrument.config.note_speed as f32,
            calc_track_bottom_world() + 0.01,
            line_start_z,
        );
        let chord_dir = Vector3::new(0.0, 6.0 * calc_string_distance(), 0.0);
        for node in [
            box_line(Color::from_rgb(0.83, 0.83, 0.83), bottom_left + chord_dir, bottom_left),
            box_line(Color::from_rgb(0.83, 0.83, 0.83), bottom_left, bottom_left + Vector3::new(0.0, 0.0, across)),
            box_line(
                Color::from_rgb(0.83, 0.83, 0.83),
                bottom_left + Vector3::new(0.0, 0.0, across),
                bottom_left + Vector3::new(0.0, 0.0, across) + chord_dir,
            ),
        ] {
            chart_root.add_child(&node.clone().upcast::<Node>());
            self.song_chart_nodes.push(node);
        }
    }

    fn update_guitar_chart(&mut self, delta: f64, song_time: f64) {
        if let Some(next) = self.next_note_block() {
            if let Some(mut cam) = self.guitar_camera.clone() {
                let cam_move_speed = 10.0_f32 / 50.0;
                let want_pos = calc_middle_window_z(next.fret_window_start, next.fret_window_length);
                let cur_pos = cam.get_position();
                let new_z = cur_pos.z * (1.0 - delta as f32 * cam_move_speed) + want_pos * delta as f32 * cam_move_speed;
                cam.set_position(Vector3::new(cur_pos.x, cur_pos.y, new_z));
            }
        }

        if let Some(mut root) = self.guitar_chart_root.clone() {
            if let Some(instrument) = self.current_instrument() {
                let root_pos = root.get_position();
                root.set_position(Vector3::new(
                    song_time as f32 * instrument.config.note_speed as f32,
                    root_pos.y,
                    root_pos.z,
                ));
            }

            let next_time = self.next_note_block().map(|x| x.time);
            if next_time != self.last_note_block_time {
                self.last_note_block_time = next_time;
                if let Some(mut n) = self.last_note_block_node.take() {
                    n.queue_free();
                }

                if let (Some(block), Some(instrument)) = (
                    self.next_note_block().cloned(),
                    self.current_instrument().cloned(),
                ) {
                    let mut node = Node3D::new_alloc();
                    for note in &block.notes {
                        let basic = basic_note(
                            note,
                            &instrument.config,
                            0.2_f32 / instrument.config.note_speed as f32,
                            block.fret_window_start,
                            block.fret_window_length,
                        );
                        node.add_child(&basic.clone().upcast::<Node>());
                    }
                    root.add_child(&node.clone().upcast::<Node>());
                    self.last_note_block_node = Some(node);
                }
            }
        }
    }

    fn setup_audio_playback(&mut self) {
        self.has_audio_stream = false;
        let mut player = self.base().get_node_as::<AudioStreamPlayer>("AudioStreamPlayer");

        let Some(audio_path) = find_song_audio_file(&self.folder_name) else {
            let wem_exists = song_root_folder().join(&self.folder_name).join("song.wem").exists();
            let mut details = self
                .base()
                .get_node_as::<Label>("DetailsVBoxContainer/SongDetailsLabel");
            let details_text = format!(
                "Length: {}\nInstrument: {}\nAudio: missing{}",
                to_min_sec(self.song_data.metadata.song_length),
                self.instrument_name,
                if wem_exists {
                    " (song.wem present; ensure libvgmstream shared library is available for wem->ogg 48k conversion)"
                } else {
                    ""
                }
            );
            details.set_text(&details_text);
            return;
        };

        let audio_path_str = audio_path.to_string_lossy().to_string();
        if audio_path
            .extension()
            .and_then(|x| x.to_str())
            .is_some_and(|x| x.eq_ignore_ascii_case("ogg"))
        {
            if let Some(stream) = AudioStreamOggVorbis::load_from_file(&audio_path_str) {
                let audio_stream: Gd<godot::classes::AudioStream> = stream.upcast();
                let _ = player.call("set_stream", &[audio_stream.to_variant()]);
                player.play();
                self.has_audio_stream = true;
            }
        } else if audio_path
            .extension()
            .and_then(|x| x.to_str())
            .is_some_and(|x| x.eq_ignore_ascii_case("wav"))
        {
            if let Some(stream) = AudioStreamWav::load_from_file(&audio_path_str) {
                let audio_stream: Gd<godot::classes::AudioStream> = stream.upcast();
                let _ = player.call("set_stream", &[audio_stream.to_variant()]);
                player.play();
                self.has_audio_stream = true;
            }
        }
    }

    fn populate_instrument_menu(&mut self) {
        let mut menu_button = self
            .base()
            .get_node_as::<MenuButton>("DetailsVBoxContainer/VBoxContainer/InstrumentMenuButton");
        menu_button.set_text(&self.instrument_name);

        let mut names = self
            .song_data
            .instruments
            .iter()
            .filter(|x| !is_hidden_instrument_name(&x.name))
            .map(|x| x.name.clone())
            .collect::<Vec<_>>();
        names.sort_by_key(|x| instrument_order_key(x));
        self.instrument_menu_names = names;

        if let Some(mut popup) = menu_button.get_popup() {
            popup.clear();
            for instrument in &self.instrument_menu_names {
                popup.add_item(instrument);
            }
            if let Some(index) = self
                .instrument_menu_names
                .iter()
                .position(|x| x.eq_ignore_ascii_case(&self.instrument_name))
            {
                popup.set_item_checked(index as i32, true);
            }
            let callable = self.base().callable("InstrumentChanged");
            let _ = popup.connect("id_pressed", &callable);
        }
    }

    fn update_labels(&mut self) {
        let year = self
            .song_data
            .metadata
            .year
            .map(|x| x.to_string())
            .unwrap_or_else(|| "?".to_string());

        let mut info = self
            .base()
            .get_node_as::<Label>("DetailsVBoxContainer/SongInfoLabel");
        let info_text = format!(
            "{} ({})\n{}\n{}",
            self.song_data.metadata.name, year, self.song_data.metadata.artist, self.song_data.metadata.album
        );
        info.set_text(&info_text);

        self.update_details_label(self.get_song_position());
        self.update_lyrics(self.get_song_position());
    }

    fn update_details_label(&mut self, song_position: f64) {
        let mut details = self
            .base()
            .get_node_as::<Label>("DetailsVBoxContainer/SongDetailsLabel");

        let Some(instrument) = self.current_instrument() else {
            details.set_text("No instrument");
            return;
        };

        let next_text = self
            .next_note_block()
            .map(|next| {
                let mut next_note = self.base().get_node_as::<Label>("GridContainer/SkipToNextLabel2");
                next_note.set_text(&format!("at {}", to_min_sec_msec(next.time, false)));
                format!(
                    "Next Note: {} in {:.1}",
                    to_min_sec_msec(next.time, false),
                    next.time - song_position
                )
            })
            .unwrap_or_else(|| "No note".to_string());

        let note_count = instrument.notes.iter().filter(|x| !x.is_chord()).count();
        let chord_count = instrument.notes.iter().filter(|x| x.is_chord()).count();
        let first = instrument.notes.first().map(|x| to_min_sec_msec(x.time, false)).unwrap_or_default();
        let last = instrument.notes.last().map(|x| to_min_sec_msec(x.time, false)).unwrap_or_default();

        details.set_text(&format!(
            "---------\nTuning: {}\nNotes: {}\nChords: {}\nFirst note @ {}\nLast note @ {}\n---------\n{}\n",
            calc_tuning_name(&instrument.config.tuning),
            note_count,
            chord_count,
            first,
            last,
            next_text,
        ));
    }

    fn update_lyrics(&mut self, song_position: f64) {
        let mut label = self.base().get_node_as::<RichTextLabel>("HBoxContainer/LyricsLabel");
        label.clear();
        label.push_font_size(40);

        let Some(lyrics) = &self.song_data.lyrics else {
            label.set_text("");
            return;
        };

        let mut idx = None;
        for (i, line) in lyrics.lines.iter().enumerate() {
            if line.end_time >= song_position {
                idx = Some(i);
                break;
            }
        }
        if idx.is_none() {
            if let Some(first) = lyrics.lines.first() {
                if first.start_time > song_position {
                    idx = Some(0);
                }
            }
        }
        let Some(i) = idx else {
            label.set_text("");
            return;
        };

        let cur = &lyrics.lines[i];
        let (part_a, part_b) = cur.parts_at(song_position);
        label.push_color(Color::from_rgb(1.0, 0.0, 0.0));
        label.add_text(&part_a);
        label.push_color(Color::from_rgb(1.0, 1.0, 0.0));
        label.add_text(&part_b);

        if let Some(next) = lyrics.lines.get(i + 1) {
            label.add_text(&format!("\n{}", next.to_line_text()));
        }
    }

    fn next_note_block(&self) -> Option<&tabplayer_parser::models::NoteBlock> {
        let instrument = self.current_instrument()?;
        let song_pos = self.get_song_position();
        instrument.notes.iter().find(|x| x.time > song_pos)
    }

    fn get_song_position(&self) -> f64 {
        if self.has_audio_stream {
            let player = self.base().get_node_as::<AudioStreamPlayer>("AudioStreamPlayer");
            let mut time = player.get_playback_position() as f64;
            time += AudioServer::singleton().get_time_since_last_mix() as f64;
            time -= AudioServer::singleton().get_output_latency() as f64;
            return time.max(0.0);
        }
        self.clock.song_time_seconds()
    }

    #[func]
    fn _init(&mut self, _state: Variant) {}

    #[func]
    fn InstrumentChanged(&mut self, id: i64) {
        if let Some(instrument) = self.instrument_menu_names.get(id as usize) {
            self.instrument_name = instrument.clone();
            self.update_labels();
            self.load_instrument_from_state();
        }
    }

    #[func]
    fn SongFinished(&mut self) {
        self.clock.seek_seconds(0.0);
        self.clock.pause();
        if self.has_audio_stream {
            let mut player = self.base().get_node_as::<AudioStreamPlayer>("AudioStreamPlayer");
            player.play();
            player.seek(0.0);
            player.set_stream_paused(true);
        }
    }

    #[func]
    fn PauseButton_Pressed(&mut self) {
        if self.clock.playing {
            self.clock.pause();
            if self.has_audio_stream {
                let mut player = self.base().get_node_as::<AudioStreamPlayer>("AudioStreamPlayer");
                player.set_stream_paused(true);
            }
        } else {
            self.clock.play();
            if self.has_audio_stream {
                let mut player = self.base().get_node_as::<AudioStreamPlayer>("AudioStreamPlayer");
                player.set_stream_paused(false);
            }
        }
    }

    #[func]
    fn Pause(&mut self) {
        self.clock.pause();
        if self.has_audio_stream {
            let mut player = self.base().get_node_as::<AudioStreamPlayer>("AudioStreamPlayer");
            player.set_stream_paused(true);
        }
    }

    #[func]
    fn Resume(&mut self) {
        self.clock.play();
        if self.has_audio_stream {
            let mut player = self.base().get_node_as::<AudioStreamPlayer>("AudioStreamPlayer");
            player.set_stream_paused(false);
        }
    }

    #[func]
    fn Quit(&mut self) {
        let mut tree = self.base().get_tree();
        let _ = tree.change_scene_to_file("res://scenes/SongPick.tscn");
    }

    #[func]
    fn Skip10Sec(&mut self) {
        let target = self.clock.song_time_seconds() + 10.0;
        self.clock.seek_seconds(target);
        if self.has_audio_stream {
            let mut player = self.base().get_node_as::<AudioStreamPlayer>("AudioStreamPlayer");
            player.seek(target as f32);
        }
    }

    #[func]
    fn Back10Sec(&mut self) {
        let target = (self.clock.song_time_seconds() - 10.0).max(0.0);
        self.clock.seek_seconds(target);
        if self.has_audio_stream {
            let mut player = self.base().get_node_as::<AudioStreamPlayer>("AudioStreamPlayer");
            player.seek(target as f32);
        }
    }

    #[func]
    fn SkipToNext(&mut self) {
        let Some(next) = self.next_note_block() else {
            return;
        };
        let target = (next.time - 1.5).max(0.0);
        self.clock.seek_seconds(target);
        if self.has_audio_stream {
            let mut player = self.base().get_node_as::<AudioStreamPlayer>("AudioStreamPlayer");
            player.seek(target as f32);
        }
    }

    #[func]
    fn RestartSong(&mut self) {
        self.clock.seek_seconds(0.0);
        if self.has_audio_stream {
            let mut player = self.base().get_node_as::<AudioStreamPlayer>("AudioStreamPlayer");
            player.seek(0.0);
        }
    }

    #[func]
    fn SlowDownPlayback(&mut self) {
        self.clock.set_speed((self.clock.speed * 0.98).max(0.5));
        if self.has_audio_stream {
            let mut player = self.base().get_node_as::<AudioStreamPlayer>("AudioStreamPlayer");
            player.set_pitch_scale(self.clock.speed as f32);
        }
    }

    #[func]
    fn SpeedUpPlayback(&mut self) {
        self.clock.set_speed((self.clock.speed / 0.98).min(1.7));
        if self.has_audio_stream {
            let mut player = self.base().get_node_as::<AudioStreamPlayer>("AudioStreamPlayer");
            player.set_pitch_scale(self.clock.speed as f32);
        }
    }

    #[func]
    fn ResetSongSpeed(&mut self) {
        self.clock.set_speed(1.0);
        if self.has_audio_stream {
            let mut player = self.base().get_node_as::<AudioStreamPlayer>("AudioStreamPlayer");
            player.set_pitch_scale(1.0);
        }
    }

    #[func]
    fn PickA(&mut self) {
        self.loop_a = Some(self.clock.song_time_seconds());
    }

    #[func]
    fn PickB(&mut self) {
        self.loop_b = Some(self.clock.song_time_seconds());
    }

    #[func]
    fn ClearLoopTimes(&mut self) {
        self.loop_a = None;
        self.loop_b = None;
    }

    #[func]
    fn MoveSongPosition(&mut self) {
        let text = self
            .base()
            .get_node_as::<LineEdit>("GridContainer/PositionSetLineEdit")
            .get_text()
            .to_string();
        if let Some(pos) = parse_song_position(&text) {
            self.clock.seek_seconds(pos.max(0.0));
            if self.has_audio_stream {
                let mut player = self.base().get_node_as::<AudioStreamPlayer>("AudioStreamPlayer");
                player.seek(pos.max(0.0) as f32);
            }
        }
    }

    #[func]
    fn GetSongPosition(&self) -> f64 {
        self.clock.song_time_seconds()
    }
}

fn calc_track_bottom_world() -> f32 {
    -0.5
}

fn calc_string_distance() -> f32 {
    1.0
}

fn calc_note_height_y(string_num: i32) -> f32 {
    (5 - string_num.clamp(0, 5)) as f32 * calc_string_distance()
}

fn calc_fret_pos_z(fret: i32) -> f32 {
    fret as f32 * 1.5
}

fn calc_in_fret_pos_z(fret: i32) -> f32 {
    calc_fret_pos_z(fret - 1) + (calc_fret_pos_z(fret) - calc_fret_pos_z(fret - 1)) / 2.0
}

fn calc_fret_width_z(fret: i32, width: i32) -> f32 {
    calc_fret_pos_z(fret + width - 1) - calc_fret_pos_z(fret - 1)
}

fn calc_middle_window_z(fret_start: i32, window_length: i32) -> f32 {
    calc_fret_pos_z(fret_start - 1) + calc_fret_width_z(fret_start, window_length) / 2.0
}

fn string_color(string_num: i32) -> Color {
    match string_num {
        0 => Color::from_rgb(1.0, 0.0, 0.0),
        1 => Color::from_rgb(1.0, 1.0, 0.0),
        2 => Color::from_rgb(0.0, 0.0, 1.0),
        3 => Color::from_rgb(1.0, 0.65, 0.0),
        4 => Color::from_rgb(0.0, 1.0, 0.0),
        _ => Color::from_rgb(0.5, 0.0, 0.5),
    }
}

fn mesh_box(color: Color, pos: Vector3, scale: Vector3) -> Gd<Node3D> {
    let mut mat = StandardMaterial3D::new_gd();
    mat.set_albedo(color);
    let mut mesh = BoxMesh::new_gd();
    mesh.set_size(scale);
    let material: Gd<Material> = mat.upcast();
    mesh.set_material(&material);

    let mut node = MeshInstance3D::new_alloc();
    let mesh_up: Gd<Mesh> = mesh.upcast();
    node.set_mesh(&mesh_up);
    node.set_position(pos);
    node.upcast()
}

fn box_line(color: Color, start: Vector3, end: Vector3) -> Gd<Node3D> {
    let length = (end - start).length();
    let center = start.lerp(end, 0.5);
    let dx = (end.x - start.x).abs();
    let dy = (end.y - start.y).abs();
    let dz = (end.z - start.z).abs();
    mesh_box(
        color,
        center,
        Vector3::new(
            if dx > 0.001 { length } else { 0.1 },
            if dy > 0.001 { dy.max(0.16) } else { 0.16 },
            if dz > 0.001 { dz.max(0.16) } else { 0.16 },
        ),
    )
}

fn basic_note(
    note: &tabplayer_parser::models::SingleNote,
    config: &tabplayer_parser::models::InstrumentConfig,
    time: f32,
    fret_window_start: i32,
    fret_window_length: i32,
) -> Gd<Node3D> {
    let color = if note.string_num == 255 {
        Color::from_rgb(1.0, 0.41, 0.71)
    } else {
        string_color(note.string_num)
    };

    if note.fret_num == 0 {
        let line_start_z = calc_fret_pos_z(fret_window_start - 1);
        let across = calc_fret_width_z(fret_window_start, fret_window_length);
        let start = Vector3::new(
            time * config.note_speed as f32,
            calc_note_height_y(note.string_num),
            line_start_z,
        );
        return box_line(color, start, start + Vector3::new(0.0, 0.0, across));
    }

    mesh_box(
        color,
        Vector3::new(
            time * config.note_speed as f32,
            calc_note_height_y(note.string_num),
            calc_in_fret_pos_z(note.fret_num),
        ),
        Vector3::new(1.25, 1.25, 1.25),
    )
}

fn make_note_geometry(
    note: &tabplayer_parser::models::SingleNote,
    config: &tabplayer_parser::models::InstrumentConfig,
    note_block: &tabplayer_parser::models::NoteBlock,
) -> Vec<Gd<Node3D>> {
    let mut out = Vec::new();

    let color = if note.string_num == 255 {
        Color::from_rgb(1.0, 0.41, 0.71)
    } else {
        string_color(note.string_num)
    };

    if !note.note_type.contains(&tabplayer_parser::models::NoteType::CHILD) {
        if note.fret_num == 0 {
            let line_start_z = calc_fret_pos_z(note_block.fret_window_start - 1);
            let across = calc_fret_width_z(note_block.fret_window_start, note_block.fret_window_length);
            let start = Vector3::new(
                note_block.time as f32 * config.note_speed as f32,
                calc_note_height_y(note.string_num),
                line_start_z,
            );
            out.push(box_line(color, start, start + Vector3::new(0.0, 0.0, across)));
        } else {
            out.push(mesh_box(
                color,
                Vector3::new(
                    note_block.time as f32 * config.note_speed as f32,
                    calc_note_height_y(note.string_num),
                    calc_in_fret_pos_z(note.fret_num),
                ),
                Vector3::new(1.25, 1.25, 1.25),
            ));
        }
    }

    if note.note_type.contains(&tabplayer_parser::models::NoteType::SUSTAIN) && note.length > 0.0 {
        let start = Vector3::new(
            note_block.time as f32 * config.note_speed as f32,
            calc_note_height_y(note.string_num),
            if note.fret_num == 0 {
                calc_middle_window_z(note_block.fret_window_start, note_block.fret_window_length)
            } else {
                calc_in_fret_pos_z(note.fret_num)
            },
        );
        let mut end = start + Vector3::new(config.note_speed as f32 * note.length as f32, 0.0, 0.0);
        if let Some(slide) = &note.slide {
            end = Vector3::new(
                end.x,
                end.y,
                calc_in_fret_pos_z(slide.to_fret),
            );
        }
        out.push(box_line(color, start, end));
    }

    let symbols = note_symbols(note);
    if !symbols.is_empty() {
        let mut text_pos = Vector3::new(
            note_block.time as f32 * config.note_speed as f32,
            calc_note_height_y(note.string_num),
            if note.fret_num == 0 {
                calc_middle_window_z(note_block.fret_window_start, note_block.fret_window_length)
            } else {
                calc_in_fret_pos_z(note.fret_num)
            },
        );
        text_pos.x -= 0.6;
        out.push(text_vertical(&symbols, text_pos));
    }

    out
}

fn note_symbols(note: &tabplayer_parser::models::SingleNote) -> String {
    let mut out = String::new();
    let t = &note.note_type;

    if t.contains(&tabplayer_parser::models::NoteType::HAMMERON) {
        out.push('h');
    }
    if t.contains(&tabplayer_parser::models::NoteType::PULLOFF) {
        out.push('p');
    }
    if t.contains(&tabplayer_parser::models::NoteType::BEND) {
        let step = note.bends.iter().map(|x| x.step as i32).max().unwrap_or(0);
        out.push('b');
        out.push_str(&step.to_string());
    }
    if t.contains(&tabplayer_parser::models::NoteType::LEFTHAND) {
        out.push('L');
    }
    if t.contains(&tabplayer_parser::models::NoteType::MUTE)
        || t.contains(&tabplayer_parser::models::NoteType::PALMMUTE)
    {
        out.push('x');
    }
    if t.contains(&tabplayer_parser::models::NoteType::TAP) {
        out.push('T');
    }
    if t.contains(&tabplayer_parser::models::NoteType::HARMONIC) {
        out.push('H');
    }
    if t.contains(&tabplayer_parser::models::NoteType::PINCHHARMONIC) {
        out.push('o');
    }
    if t.contains(&tabplayer_parser::models::NoteType::FRETHANDMUTE) {
        out.push('.');
    }

    out
}

fn text_vertical(text: &str, pos: Vector3) -> Gd<Node3D> {
    let mut label = Label3D::new_alloc();
    label.set_text(text);
    label.set_font_size(200);
    label.set_position(pos);
    let _ = label.call("rotate_y", &[std::f32::consts::FRAC_PI_2.to_variant()]);
    label.upcast()
}

fn is_same_chord_as(a: &tabplayer_parser::models::NoteBlock, b: &tabplayer_parser::models::NoteBlock) -> bool {
    if a.notes.len() != b.notes.len() {
        return false;
    }
    if a.chord_flags != b.chord_flags {
        return false;
    }

    for (na, nb) in a.notes.iter().zip(b.notes.iter()) {
        if na.fret_num != nb.fret_num || na.string_num != nb.string_num {
            return false;
        }
        if !na.bends.is_empty() || na.slide.is_some() || !nb.bends.is_empty() || nb.slide.is_some() {
            return false;
        }

        let mut ta = na
            .note_type
            .iter()
            .copied()
            .filter(|x| {
                !matches!(
                    x,
                    tabplayer_parser::models::NoteType::UNDEFINED
                        | tabplayer_parser::models::NoteType::MISSING
                        | tabplayer_parser::models::NoteType::CHORD
                        | tabplayer_parser::models::NoteType::OPEN
                        | tabplayer_parser::models::NoteType::IGNORE
                        | tabplayer_parser::models::NoteType::HIGHDENSITY
                        | tabplayer_parser::models::NoteType::SINGLE
                        | tabplayer_parser::models::NoteType::CHORDNOTES
                        | tabplayer_parser::models::NoteType::DOUBLESTOP
                        | tabplayer_parser::models::NoteType::MISSING2
                        | tabplayer_parser::models::NoteType::STRUM
                        | tabplayer_parser::models::NoteType::ACCENT
                )
            })
            .collect::<Vec<_>>();
        let mut tb = nb
            .note_type
            .iter()
            .copied()
            .filter(|x| {
                !matches!(
                    x,
                    tabplayer_parser::models::NoteType::UNDEFINED
                        | tabplayer_parser::models::NoteType::MISSING
                        | tabplayer_parser::models::NoteType::CHORD
                        | tabplayer_parser::models::NoteType::OPEN
                        | tabplayer_parser::models::NoteType::IGNORE
                        | tabplayer_parser::models::NoteType::HIGHDENSITY
                        | tabplayer_parser::models::NoteType::SINGLE
                        | tabplayer_parser::models::NoteType::CHORDNOTES
                        | tabplayer_parser::models::NoteType::DOUBLESTOP
                        | tabplayer_parser::models::NoteType::MISSING2
                        | tabplayer_parser::models::NoteType::STRUM
                        | tabplayer_parser::models::NoteType::ACCENT
                )
            })
            .collect::<Vec<_>>();
        ta.sort_by_key(|x| *x as i32);
        tb.sort_by_key(|x| *x as i32);
        if ta != tb {
            return false;
        }
    }

    true
}

fn generate_note_block_frets(instrument: &tabplayer_parser::models::SongInstrument) -> Vec<Gd<Node3D>> {
    let mut out = Vec::new();
    let mut start_of_section = -10.0_f64;
    let mut cur_start = -1;
    let mut cur_length = -1;

    for note in &instrument.notes {
        if cur_start != note.fret_window_start || cur_length != note.fret_window_length {
            if cur_start > 0 {
                while start_of_section < note.time {
                    let length = (note.time - start_of_section).min(10.0);
                    out.push(create_window_piece(
                        cur_start,
                        cur_length,
                        (start_of_section + length) as f32,
                        start_of_section as f32,
                        instrument,
                    ));
                    start_of_section += length;
                }
            }
            cur_start = note.fret_window_start;
            cur_length = note.fret_window_length;
            start_of_section = note.time;
        }
    }

    if cur_start > 0 {
        if let Some(last) = instrument.notes.last() {
            while start_of_section < last.time + 1.0 {
                let length = ((last.time + 1.0) - start_of_section).min(10.0);
                out.push(create_window_piece(
                    cur_start,
                    cur_length,
                    (start_of_section + length) as f32,
                    start_of_section as f32,
                    instrument,
                ));
                start_of_section += length;
            }
        }
    }

    out
}

fn create_window_piece(
    fret: i32,
    length: i32,
    end_time: f32,
    start_time: f32,
    instrument: &tabplayer_parser::models::SongInstrument,
) -> Gd<Node3D> {
    let across = calc_fret_width_z(fret, length);
    let pos = Vector3::new(
        ((end_time + start_time) / 2.0) * instrument.config.note_speed as f32 - 0.5,
        calc_track_bottom_world() - 0.01,
        calc_fret_pos_z(fret - 1) + across / 2.0,
    );

    let mut mat = StandardMaterial3D::new_gd();
    mat.set_albedo(Color::from_rgb(0.28, 0.24, 0.55));
    let mut plane = PlaneMesh::new_gd();
    plane.set_size(Vector2::new(
        instrument.config.note_speed as f32 * (end_time - start_time),
        across,
    ));
    let material: Gd<Material> = mat.upcast();
    plane.set_material(&material);
    let mut node = MeshInstance3D::new_alloc();
    let plane_mesh: Gd<Mesh> = plane.upcast();
    node.set_mesh(&plane_mesh);
    node.set_position(pos);
    node.upcast()
}

fn to_min_sec_msec(value: f64, frac: bool) -> String {
    let min = (value / 60.0).floor() as i64;
    let sec = (value % 60.0).floor() as i64;
    if frac {
        let ms = ((value.fract().abs()) * 1000.0).floor() as i64;
        format!("{min}m {sec:02}s {ms:03}ms")
    } else {
        format!("{min}m {sec:02}s")
    }
}

fn parse_song_position(text: &str) -> Option<f64> {
    if let Ok(v) = text.trim().parse::<f64>() {
        return Some(v);
    }

    let cleaned = text
        .replace("ms", " ")
        .replace('m', " ")
        .replace('s', " ")
        .replace(':', " ");
    let nums = cleaned
        .split_whitespace()
        .filter_map(|x| x.parse::<f64>().ok())
        .collect::<Vec<_>>();

    match nums.as_slice() {
        [m, s, ms] => Some(m * 60.0 + s + ms / 1000.0),
        [m, s] => Some(m * 60.0 + s),
        [s] => Some(*s),
        _ => None,
    }
}

#[derive(GodotClass)]
#[class(base=VBoxContainer)]
struct ConvertMenu {
    #[base]
    base: Base<VBoxContainer>,
}

#[godot_api]
impl IVBoxContainer for ConvertMenu {
    fn init(base: Base<VBoxContainer>) -> Self {
        Self { base }
    }
}

#[godot_api]
impl ConvertMenu {
    #[func]
    fn ChoseButton_Pressed(&mut self) {
        let mut dialog = self.base().get_node_as::<FileDialog>("FileDialog");
        dialog.popup_centered();
    }

    #[func]
    fn FromDownloadsButton_Pressed(&mut self) {
        let downloads = Os::singleton().get_system_dir(godot::classes::os::SystemDir::DOWNLOADS);
        let files = collect_psarc_files_recursive(&PathBuf::from(downloads.to_string()));
        self.import_psarc_files(files);
    }

    #[func]
    fn Dir_Selected(&mut self, dir: GString) {
        let files = collect_psarc_files_recursive(&PathBuf::from(dir.to_string()));
        self.import_psarc_files(files);
    }

    #[func]
    fn File_Selected(&mut self, path: GString) {
        self.import_psarc_files(vec![PathBuf::from(path.to_string())]);
    }

    #[func]
    fn Files_Selected(&mut self, paths: PackedStringArray) {
        let mut files = Vec::new();
        for path in paths.as_slice() {
            files.push(PathBuf::from(path.to_string()));
        }
        self.import_psarc_files(files);
    }

    #[func]
    fn BackButton_Pressed(&mut self) {
        let mut tree = self.base().get_tree();
        let _ = tree.change_scene_to_file("res://scenes/StartMenu.tscn");
    }

    #[func]
    fn AnimateIn(&mut self) {}

    #[func]
    fn AnimateOut(&mut self) {}

    fn import_psarc_files(&mut self, files: Vec<PathBuf>) {
        let mut info = self.base().get_node_as::<Label>("InfoLabel");
        if files.is_empty() {
            info.set_text("No valid .psarc files found");
            return;
        }

        match parser_import_psarc_files(&files) {
            Ok(report) => {
                let msg = format!("Imported: {}, failed: {}", report.completed, report.failed);
                info.set_text(&msg);
            }
            Err(err) => {
                let msg = format!("Import failed: {err}");
                info.set_text(&msg);
            }
        }
    }
}

#[derive(GodotClass)]
#[class(base=VBoxContainer)]
struct InfoPage {
    #[base]
    base: Base<VBoxContainer>,
}

#[godot_api]
impl IVBoxContainer for InfoPage {
    fn init(base: Base<VBoxContainer>) -> Self {
        Self { base }
    }
}

#[godot_api]
impl InfoPage {
    #[func]
    fn BackButton_Pressed(&mut self) {
        let mut tree = self.base().get_tree();
        let _ = tree.change_scene_to_file("res://scenes/StartMenu.tscn");
    }

    #[func]
    fn ProjectSourceButton_Pressed(&mut self) {
        let _ = Os::singleton().shell_open("https://github.com/Murph9/tabplayerV2");
    }

    #[func]
    fn OpenConfigFolder_Pressed(&mut self) {
        let path = song_root_folder().to_string_lossy().to_string();
        let _ = Os::singleton().shell_open(&path);
    }

    #[func]
    fn AnimateIn(&mut self) {}

    #[func]
    fn AnimateOut(&mut self) {}
}

#[derive(GodotClass)]
#[class(base=VBoxContainer)]
struct SettingsPage {
    #[base]
    base: Base<VBoxContainer>,
}

#[godot_api]
impl IVBoxContainer for SettingsPage {
    fn init(base: Base<VBoxContainer>) -> Self {
        Self { base }
    }
}

#[godot_api]
impl SettingsPage {
    #[func]
    fn BackButton_Pressed(&mut self) {
        let mut tree = self.base().get_tree();
        let _ = tree.change_scene_to_file("res://scenes/StartMenu.tscn");
    }

    #[func]
    fn AnimateIn(&mut self) {}

    #[func]
    fn AnimateOut(&mut self) {}
}

#[derive(GodotClass)]
#[class(base=VBoxContainer)]
struct SongList {
    #[base]
    _base: Base<VBoxContainer>,
}

#[godot_api]
impl IVBoxContainer for SongList {
    fn init(base: Base<VBoxContainer>) -> Self {
        Self { _base: base }
    }
}

#[godot_api]
impl SongList {
    #[func]
    fn SelectRandom(&mut self) {}

    #[func]
    fn UpdateFilter(&mut self, _filter: GString) {}

    #[func]
    fn TuningSelected(&mut self, _index: i64) {}

    #[func]
    fn ShowCapo_Pressed(&mut self) {}
}

#[derive(GodotClass)]
#[class(base=VBoxContainer)]
struct SongDisplay {
    #[base]
    _base: Base<VBoxContainer>,
}

#[godot_api]
impl IVBoxContainer for SongDisplay {
    fn init(base: Base<VBoxContainer>) -> Self {
        Self { _base: base }
    }
}

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
        let files = collect_psarc_files_recursive(&PathBuf::from(dir.to_string()));
        self.import_files_internal(files)
    }

    #[func]
    fn import_default_dlc(&self) -> VariantDict {
        self.import_psarc_dir("/home/csantz/Music/DLC".into())
    }

    #[func]
    fn import_psarc_files(&self, paths: PackedStringArray) -> VariantDict {
        let mut files = Vec::new();
        for path in paths.as_slice() {
            files.push(PathBuf::from(path.to_string()));
        }
        self.import_files_internal(files)
    }

    #[func]
    fn song_count(&self) -> i64 {
        read_song_file_list().data.len() as i64
    }

    #[func]
    fn list_song_folders(&self) -> PackedStringArray {
        let mut arr = PackedStringArray::new();
        for song in read_song_file_list().data {
            arr.push(song.folder_name.as_str());
        }
        arr
    }

    #[func]
    fn load_song_summary(&self, folder: GString) -> VariantDict {
        let mut dict = VariantDict::new();
        if let Ok(song) = load_song(&folder.to_string()) {
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
    fn song_art_status(&self, folder: GString) -> VariantDict {
        let mut dict = VariantDict::new();
        let song_dir = song_root_folder().join(folder.to_string());
        let art_path = song_dir.join("album.dds");
        dict.set("exists", art_path.exists());
        dict.set("path", art_path.to_string_lossy().to_string());

        if art_path.exists() {
            let mut compressed = CompressedTexture2D::new_gd();
            let compressed_ok = compressed.load(&art_path.to_string_lossy().to_string()) == godot::global::Error::OK;
            let loadable = load_dds_texture(&art_path).is_some();
            dict.set("compressed_loadable", compressed_ok);
            dict.set("loadable", loadable);
            dict.set(
                "load_error",
                if loadable {
                    ""
                } else {
                    "dds decode failed"
                },
            );
        } else {
            dict.set("compressed_loadable", false);
            dict.set("loadable", false);
            dict.set("load_error", "missing");
        }

        dict
    }

    fn import_files_internal(&self, files: Vec<PathBuf>) -> VariantDict {
        let mut dict = VariantDict::new();
        match parser_import_psarc_files(&files) {
            Ok(report) => {
                dict.set("ok", true);
                dict.set("completed", report.completed as i64);
                dict.set("failed", report.failed as i64);
            }
            Err(err) => {
                dict.set("ok", false);
                dict.set("completed", 0_i64);
                dict.set("failed", files.len() as i64);
                dict.set("error", err.to_string());
            }
        }
        dict
    }
}

#[derive(GodotClass)]
#[class(base=Node3D)]
struct GuitarChart {
    #[base]
    _base: Base<Node3D>,
}

#[godot_api]
impl INode3D for GuitarChart {
    fn init(base: Base<Node3D>) -> Self {
        Self { _base: base }
    }
}

#[derive(GodotClass)]
#[class(base=Node3D)]
struct SongChart {
    #[base]
    _base: Base<Node3D>,
}

#[godot_api]
impl INode3D for SongChart {
    fn init(base: Base<Node3D>) -> Self {
        Self { _base: base }
    }
}

#[derive(GodotClass)]
#[class(base=Node2D)]
struct NoteMiniGraph {
    #[base]
    _base: Base<Node2D>,
}

#[godot_api]
impl INode2D for NoteMiniGraph {
    fn init(base: Base<Node2D>) -> Self {
        Self { _base: base }
    }
}

#[derive(GodotClass)]
#[class(base=Node)]
struct NoteBucketGraph {
    #[base]
    _base: Base<Node>,
}

#[godot_api]
impl INode for NoteBucketGraph {
    fn init(base: Base<Node>) -> Self {
        Self { _base: base }
    }
}

struct TabPlayerExtension;

#[gdextension]
unsafe impl ExtensionLibrary for TabPlayerExtension {}
