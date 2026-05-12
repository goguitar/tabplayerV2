use crate::common::*;

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
    sort_column: i32,
    sort_desc: bool,
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
            sort_column: 0,
            sort_desc: false,
            pending_confirm: None,
            display_instruments: Vec::new(),
        }
    }

    fn ready(&mut self) {
        let _ = ensure_song_catalog_loaded();
        self.songs = catalog_list_song_files();
        self.configure_song_tree();
        self.populate_tuning_filter();
        self.refresh_song_list();
    }
}

#[godot_api]
impl SongPick {
    fn configure_song_tree(&mut self) {
        let mut tree = self
            .base()
            .get_node_as::<Tree>("MarginContainer/VBoxContainer/ContentSplit/SongsTree");
        tree.set_columns(6);
        tree.set_column_titles_visible(true);
        tree.set_hide_root(true);
        tree.set_column_title(0, "Song Name");
        tree.set_column_title(1, "Artist");
        tree.set_column_title(2, "Album");
        tree.set_column_title(3, "Year");
        tree.set_column_title(4, "Length");
        tree.set_column_title(5, "Parts");
    }

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

        let sort_column = self.sort_column;
        let sort_desc = self.sort_desc;
        let songs = &self.songs;
        self.visible_indices.sort_by(|a, b| {
            SongPick::compare_song_indices(songs, sort_column, sort_desc, *a, *b)
        });

        let mut list = self
            .base()
            .get_node_as::<Tree>("MarginContainer/VBoxContainer/ContentSplit/SongsTree");
        list.clear();

        let Some(root) = list.create_item() else {
            return;
        };

        for song_idx in &self.visible_indices {
            let song = &self.songs[*song_idx];
            let Some(mut row) = list.create_item_ex().parent(&root).done() else {
                continue;
            };

            row.set_text(0, &song.song_name);
            row.set_text(1, &song.artist);
            row.set_text(2, &song.album);
            row.set_text(3, &song.year.map(|x| x.to_string()).unwrap_or_default());
            row.set_text(4, &to_min_sec(song.length));
            row.set_text(5, &SongPick::song_parts_text(song));
            row.set_metadata(0, &(*song_idx as i64).to_variant());
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

        let song_idx = self.visible_indices[selected_song_idx];
        self.select_song_row(song_idx);
        self.selected_index = Some(song_idx);
        self.update_selected_song_ui(song_idx);
    }

    fn song_parts_text(song: &SongFile) -> String {
        let mut chars = [' '; 5];
        if song.instruments.iter().any(|x| {
            let name = x.name.to_ascii_lowercase();
            name == "lead" || name == "lead1" || name == "lead2"
        }) {
            chars[0] = 'L';
        }
        if song.instruments.iter().any(|x| {
            let name = x.name.to_ascii_lowercase();
            name == "rhythm" || name == "rhythm1" || name == "rhythm2"
        }) {
            chars[1] = 'R';
        }
        if song.instruments.iter().any(|x| {
            let name = x.name.to_ascii_lowercase();
            name == "bass" || name == "bass1" || name == "bass2"
        }) {
            chars[2] = 'B';
        }
        if song.lyrics.as_ref().is_some_and(|x| x.word_count > 0) {
            chars[3] = 'V';
        }

        let other = song
            .instruments
            .iter()
            .filter(|x| {
                let name = x.name.to_ascii_lowercase();
                !(name == "lead"
                    || name == "lead1"
                    || name == "lead2"
                    || name == "rhythm"
                    || name == "rhythm1"
                    || name == "rhythm2"
                    || name == "bass"
                    || name == "bass1"
                    || name == "bass2")
            })
            .count();
        if other > 0 {
            chars[4] = other.to_string().chars().next().unwrap_or(' ');
        }

        chars.iter().collect()
    }

    fn compare_song_indices(
        songs: &[SongFile],
        sort_column: i32,
        sort_desc: bool,
        a: usize,
        b: usize,
    ) -> Ordering {
        let sa = &songs[a];
        let sb = &songs[b];
        let ord = match sort_column {
            1 => sa.artist.to_lowercase().cmp(&sb.artist.to_lowercase()),
            2 => sa.album.to_lowercase().cmp(&sb.album.to_lowercase()),
            3 => sa.year.unwrap_or(0).cmp(&sb.year.unwrap_or(0)),
            4 => sa.length.partial_cmp(&sb.length).unwrap_or(Ordering::Equal),
            5 => SongPick::song_parts_text(sa).cmp(&SongPick::song_parts_text(sb)),
            _ => sa.song_name.to_lowercase().cmp(&sb.song_name.to_lowercase()),
        };

        if sort_desc {
            ord.reverse()
        } else {
            ord
        }
    }

    fn select_song_row(&mut self, song_idx: usize) {
        let mut tree = self
            .base()
            .get_node_as::<Tree>("MarginContainer/VBoxContainer/ContentSplit/SongsTree");

        let Some(root) = tree.get_root() else {
            return;
        };
        let mut current = root.get_first_child();
        while let Some(item) = current {
            if item.get_metadata(0).try_to::<i64>().ok() == Some(song_idx as i64) {
                tree.set_selected(&item, 0);
                return;
            }
            current = item.get_next();
        }
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

        let mut album_tex = self.base().get_node_as::<TextureRect>(
            "MarginContainer/VBoxContainer/ContentSplit/DetailsVBox/AlbumArtTextureRect",
        );
        if let Ok(Some(bytes)) = catalog_load_song_album_art(&song.folder_name) {
            if let Some(tex2d) = load_dds_texture_from_bytes(&bytes) {
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
    fn SongSelected(&mut self) {
        let tree = self
            .base()
            .get_node_as::<Tree>("MarginContainer/VBoxContainer/ContentSplit/SongsTree");
        let Some(item) = tree.get_selected() else {
            return;
        };
        let Some(song_idx) = item.get_metadata(0).try_to::<i64>().ok().map(|x| x as usize) else {
            return;
        };
        self.selected_index = Some(song_idx);
        self.update_selected_song_ui(song_idx);
    }

    #[func]
    fn SongActivated(&mut self) {
        let tree = self
            .base()
            .get_node_as::<Tree>("MarginContainer/VBoxContainer/ContentSplit/SongsTree");
        let Some(item) = tree.get_selected() else {
            return;
        };
        let Some(song_idx) = item.get_metadata(0).try_to::<i64>().ok().map(|x| x as usize) else {
            return;
        };
        self.selected_index = Some(song_idx);
        self.play_selected_instrument_inner();
    }

    #[func]
    fn SongColumnTitleClicked(&mut self, column: i64, _mouse_button_index: i64) {
        let column = column as i32;
        if self.sort_column == column {
            self.sort_desc = !self.sort_desc;
        } else {
            self.sort_column = column;
            self.sort_desc = false;
        }
        self.refresh_song_list();
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
            .get_node_as::<Tree>("MarginContainer/VBoxContainer/ContentSplit/SongsTree");
        let song_idx = self.visible_indices[pick];
        self.select_song_row(song_idx);
        let _ = list.call("ensure_cursor_is_visible", &[]);
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
