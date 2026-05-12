use super::shared::*;

#[derive(GodotClass)]
#[class(base=VBoxContainer)]
pub struct SongDisplay {
    #[base]
    base: Base<VBoxContainer>,
    folder_name: Option<String>,
}

#[godot_api]
impl IVBoxContainer for SongDisplay {
    fn init(base: Base<VBoxContainer>) -> Self {
        Self { base, folder_name: None }
    }
}

#[godot_api]
impl SongDisplay {
    #[signal]
    fn song_selected(folder: GString, instrument: GString);

    #[func]
    fn song_changed(&mut self, folder_name: GString) {
        self.folder_name = Some(folder_name.to_string());
        self.load_song();
    }

    fn load_song(&mut self) {
        let folder = match &self.folder_name {
            Some(folder) => folder.clone(),
            None => return,
        };
        let song_info = SongRepository::global()
            .lock()
            .song_files()
            .into_iter()
            .find(|song| song.id == folder);
        let song_info = match song_info {
            Some(song) => song,
            None => return,
        };

            if let Some(mut album_rect) = self.base_mut().try_get_node_as::<TextureRect>("AlbumArtTextureRect") {
                album_rect.set_texture(Some(&GradientTexture2D::new_gd().upcast::<Texture2D>()));
            }
        if let Some(mut label) = self.base_mut().try_get_node_as::<Label>("ArtistLabel") {
            label.set_text(&format!("Artist: {}", song_info.artist));
        }
        if let Some(mut label) = self.base_mut().try_get_node_as::<Label>("SongNameLabel") {
            label.set_text(&format!("Name: {}", song_info.song_name));
        }
        if let Some(mut label) = self.base_mut().try_get_node_as::<Label>("AlbumLabel") {
            label.set_text(&format!("Album: {}", song_info.album));
        }
        if let Some(mut label) = self.base_mut().try_get_node_as::<Label>("YearLabel") {
            label.set_text(&format!("Year: {:?}", song_info.year));
        }
        if let Some(mut label) = self.base_mut().try_get_node_as::<Label>("OtherLabel") {
            label.set_text(&format!("Length: {}", to_min_sec(song_info.length as f64, false)));
        }

        if let Some(mut grid) = self.base_mut().try_get_node_as::<GridContainer>("InstrumentGridContainer") {
            for child in grid.get_children().iter_shared() {
                grid.remove_child(Some(&child));
            }
            grid.add_child(Some(&Label::new_alloc().upcast::<Node>()));
            let mut tuning_label = Label::new_alloc();
            tuning_label.set_text("Tuning");
            grid.add_child(Some(&tuning_label.upcast::<Node>()));
            let mut note_label = Label::new_alloc();
            note_label.set_text("Note Counts");
            grid.add_child(Some(&note_label.upcast::<Node>()));
            let mut density_label = Label::new_alloc();
            density_label.set_text("Note Density");
            grid.add_child(Some(&density_label.upcast::<Node>()));
            grid.set_columns(grid.get_child_count() as i32);

            let mut instruments = song_info.instruments.clone();
            instruments.sort_by(|a, b| {
                let a_order = SongInfo::STANDARD_INSTRUMENT_TYPES
                    .iter()
                    .position(|name| name == &a.name.as_str())
                    .unwrap_or(999);
                let b_order = SongInfo::STANDARD_INSTRUMENT_TYPES
                    .iter()
                    .position(|name| name == &b.name.as_str())
                    .unwrap_or(999);
                a_order.cmp(&b_order)
            });
            for instrument in instruments {
                let mut button = Button::new_alloc();
                button.set_text(&format!("Play {}", instrument.name));
                let folder_name = self.folder_name.clone().unwrap_or_default();
                let instrument_name = instrument.name.clone();
                let callable = self.base_mut().callable("emit_song_selected").bind(
                    folder_name.to_variant(),
                    instrument_name.to_variant(),
                );
                button.connect("pressed", &callable);
                grid.add_child(Some(&button.upcast::<Node>()));
                let mut label = Label::new_alloc();
                label.set_text(&Instrument::calc_tuning_name(instrument.tuning, instrument.capo_fret));
                grid.add_child(Some(&label.upcast::<Node>()));
                let mut label = Label::new_alloc();
                label.set_text(&format!("{}", instrument.note_count));
                grid.add_child(Some(&label.upcast::<Node>()));
                let mut label = Label::new_alloc();
                label.set_text(&to_fixed_places(instrument.note_density(&song_info) as f64, 2, false));
                grid.add_child(Some(&label.upcast::<Node>()));
            }
        }
    }

    #[func]
    fn emit_song_selected(&mut self, folder: Variant, instrument: Variant) {
        self.base_mut().emit_signal("song_selected", &[folder, instrument]);
    }
}
