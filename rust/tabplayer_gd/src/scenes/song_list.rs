use super::shared::*;
use super::SongDisplay;

#[derive(GodotClass)]
#[class(base=VBoxContainer)]
pub struct SongList {
    #[base]
    base: Base<VBoxContainer>,
    rows: Vec<Row>,
    sort_key: Option<String>,
    filter: Option<String>,
    pub tuning_filter: String,
    song_display: Option<Gd<SongDisplay>>,
}

struct Row {
    song: SongFile,
    controls: Vec<Gd<Control>>,
    selected: bool,
}

#[godot_api]
impl IVBoxContainer for SongList {
    fn init(base: Base<VBoxContainer>) -> Self {
        Self {
            base,
            rows: Vec::new(),
            sort_key: None,
            filter: None,
            tuning_filter: String::new(),
            song_display: None,
        }
    }

    fn ready(&mut self) {
        let song_list = SongRepository::global().lock().song_files();
        self.rows = song_list
            .into_iter()
            .map(|song| Row {
                controls: vec![
                    create_label(&song.song_name, &song.id, self.base_mut().callable("on_row_input")),
                    create_label(&song.artist, &song.id, self.base_mut().callable("on_row_input")),
                    create_label(&song.album, &song.id, self.base_mut().callable("on_row_input")),
                    create_label(&song.year.map(|y| y.to_string()).unwrap_or_default(), &song.id, self.base_mut().callable("on_row_input")),
                    create_label(&to_min_sec(song.length as f64, false), &song.id, self.base_mut().callable("on_row_input")),
                    create_label(&song.instrument_chars(), &song.id, self.base_mut().callable("on_row_input")),
                ],
                song,
                selected: false,
            })
            .collect();

        if let Some(display) = &self.song_display {
            if let Some(mut split) = self
                .base_mut()
                .try_get_node_as::<VBoxContainer>("HSplitContainer/VBoxContainerDetails")
            {
                split.add_child(Some(&display.clone().upcast::<Node>()));
            }
        }

        let mut group = ButtonGroup::new_gd();
        group.connect("pressed", &self.base_mut().callable("Heading_Pressed"));
        if let Some(mut grid) = self.base_mut().try_get_node_as::<GridContainer>("%GridContainer") {
            let headings = ["Song Name", "Artist", "Album", "Year", "Length", "Parts"];
            grid.set_columns(headings.len() as i32);
            for heading in headings.iter() {
                let mut button = Button::new_alloc();
                button.set_text(heading);
                button.set_button_group(Some(&group));
                button.set_toggle_mode(true);
                grid.add_child(Some(&button.upcast::<Node>()));
            }
        }

        if let Some(mut tuning_select) = self.base_mut().try_get_node_as::<OptionButton>("HBoxContainer/TuningOptionButton") {
            tuning_select.add_item("");
            let tunings = self
                .rows
                .iter()
                .filter_map(|row| row.song.main_instrument())
                .map(|inst| Instrument::calc_tuning_name(inst.tuning, inst.capo_fret))
                .unique();
            for tuning in tunings {
                tuning_select.add_item(tuning.as_str());
            }
        }

        self.load_table_rows();
        self.load_table_filter();
    }
}

#[godot_api]
impl SongList {
    #[signal]
    fn song_selected(folder: GString);

    pub fn set_display(&mut self, display: Gd<SongDisplay>) {
        self.song_display = Some(display);
    }

    #[func]
    fn row_selected(&mut self, folder: GString) {
        self.base_mut().emit_signal("song_selected", &[folder.to_variant()]);
    }

    #[func]
    fn on_row_input(&mut self, event: Gd<InputEvent>, folder: GString) {
        if let Some(event) = event.try_cast::<InputEventMouseButton>() {
            if event.is_pressed() && event.get_button_index() == MouseButton::LEFT {
                self.base_mut().emit_signal("song_selected", &[folder.to_variant()]);
            }
        }
    }

    fn load_table_rows(&mut self) {
        if let Some(mut grid) = self.base_mut().try_get_node_as::<GridContainer>("%GridContainer") {
            for row in &self.rows {
                for control in &row.controls {
                    if control.get_parent().is_some() {
                        grid.remove_child(Some(&control.clone().upcast::<Node>()));
                    }
                    grid.add_child(Some(&control.clone().upcast::<Node>()));
                }
            }
        }
    }

    fn load_table_filter(&mut self) {
        let capo_shown = self
            .base_mut()
            .try_get_node_as::<CheckBox>("HBoxContainer/CapoCheckBox")
            .map(|checkbox| checkbox.is_pressed())
            .unwrap_or(false);
        let mut count_shown = 0;
        for row in &mut self.rows {
            let tuning_enabled = row.song.main_instrument().map(|instrument| {
                let tuning_name = Instrument::calc_tuning_name(instrument.tuning, instrument.capo_fret);
                (self.tuning_filter.is_empty() || tuning_name == self.tuning_filter)
                    && (instrument.capo_fret == 0.0 || (capo_shown && instrument.capo_fret != 0.0))
            }).unwrap_or(true);
            let enabled = self.filter.as_ref().map(|filter| {
                row.song.artist.to_lowercase().contains(&filter.to_lowercase())
                    || row.song.song_name.to_lowercase().contains(&filter.to_lowercase())
                    || row.song.album.to_lowercase().contains(&filter.to_lowercase())
            }).unwrap_or(true);
            for control in &row.controls {
                control.set_visible(enabled && tuning_enabled);
            }
            if enabled && tuning_enabled {
                count_shown += 1;
            }
        }
        if let Some(mut label) = self.base_mut().try_get_node_as::<Label>("HBoxContainer/SongsLoadedLabel") {
            label.set_text(&format!("{count_shown} songs shown"));
        }
    }

    #[func]
    #[allow(non_snake_case)]
    fn TuningSelected(&mut self, index: i64) {
        if let Some(tuning_select) = self.base_mut().try_get_node_as::<OptionButton>("HBoxContainer/TuningOptionButton") {
            let record = tuning_select.get_item_text(index as i32);
            self.tuning_filter = record.to_string();
            self.load_table_filter();
        }
    }

    #[func]
    #[allow(non_snake_case)]
    fn Heading_Pressed(&mut self, button: Gd<Button>) {
        self.sort_key = Some(button.get_text().to_string());
        self.rows.sort_by(|a, b| match self.sort_key.as_deref() {
            Some("Artist") => a.song.artist.cmp(&b.song.artist),
            Some("Album") => a.song.album.cmp(&b.song.album),
            Some("Year") => a.song.year.cmp(&b.song.year),
            Some("Length") => a.song.length.partial_cmp(&b.song.length).unwrap_or(Ordering::Equal),
            _ => a.song.song_name.cmp(&b.song.song_name),
        });
        self.load_table_rows();
    }

    #[func]
    #[allow(non_snake_case)]
    fn UpdateFilter(&mut self, filter: GString) {
        let filter = filter.to_string();
        if filter.is_empty() {
            self.filter = None;
        } else {
            self.filter = Some(filter);
        }
        self.load_table_filter();
    }

    #[func]
    #[allow(non_snake_case)]
    fn SelectRandom(&mut self) {
        let valid_songs = self
            .rows
            .iter()
            .filter(|row| row.controls.iter().any(|c| c.is_visible()))
            .collect::<Vec<_>>();
        if valid_songs.is_empty() {
            return;
        }
        let index = (rand::random::<f32>() * valid_songs.len() as f32) as usize;
        let song = &valid_songs[index];
        self.base_mut().emit_signal("song_selected", &[song.song.id.to_variant()]);
    }

    #[func]
    #[allow(non_snake_case)]
    fn ShowCapo_Pressed(&mut self) {
        self.load_table_filter();
    }
}

 

fn create_label(text: &str, folder: &str, callable: Callable) -> Gd<Control> {
    let mut label = Label::new_alloc();
    label.set_text(&fixed_width_string(text, 30));
    label.set_mouse_filter(Control::MouseFilter::Stop);
    let args = [folder.to_variant()];
    label.connect("gui_input", &callable.bind(&args));
    label.upcast()
}
