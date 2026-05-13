use super::shared::*;
use super::{load_scene, SongDisplay, SongList};

#[derive(GodotClass)]
#[class(base=Control)]
pub struct SongPick {
    #[base]
    base: Base<Control>,
    song_list: Option<Gd<SongList>>,
    song_display: Option<Gd<SongDisplay>>,
    temp_song_for_confirm: Option<SongFile>,
    temp_instrument_for_confirm: Option<String>,
}

#[godot_api]
impl IControl for SongPick {
    fn init(base: Base<Control>) -> Self {
        Self {
            base,
            song_list: None,
            song_display: None,
            temp_song_for_confirm: None,
            temp_instrument_for_confirm: None,
        }
    }

    fn ready(&mut self) {
        let mut song_display = load_scene::<SongDisplay>("res://scenes/SongDisplay.tscn");
        song_display.connect(
            "song_selected",
            &self.base_mut().callable("on_song_selected"),
        );
        let mut song_list = load_scene::<SongList>("res://scenes/SongList.tscn");
        song_list.bind_mut().set_display(song_display.clone());
        song_list.connect("song_selected", &song_display.callable("song_changed"));
        self.song_list = Some(song_list.clone());
        self.song_display = Some(song_display);

        if let Some(mut vbox) = self.base_mut().try_get_node_as::<VBoxContainer>("MarginContainer/VBoxContainer") {
            vbox.add_child(Some(&song_list.clone().upcast::<Node>()));
        }
    }
}

#[godot_api]
impl SongPick {
    #[signal]
    fn closed();
    #[signal]
    fn opened_song(folder: GString, instrument: GString);

    #[func]
    fn on_song_selected(&mut self, folder: GString, instrument: GString) {
        let song_list = SongRepository::global().lock().song_files();
        let song = song_list.iter().find(|song| song.id == folder.to_string());
        if let Some(song) = song {
            self.choose_song(song.clone(), instrument.to_string());
        }
    }

    fn choose_song(&mut self, song: SongFile, instrument: String) {
        if let Some(song_list) = &self.song_list {
            let tuning_filter = song_list.bind().tuning_filter.clone();
            if tuning_filter.is_empty() {
                self.base_mut().emit_signal(
                    "opened_song",
                    &[song.id.to_variant(), instrument.to_variant()],
                );
                return;
            }
            if let Some(picked_instrument) =
                song.instruments.iter().find(|inst| inst.name == instrument)
            {
                let tuning_name = Instrument::calc_tuning_name(picked_instrument.tuning, picked_instrument.capo_fret);
                if tuning_name != tuning_filter {
                    if let Some(mut dialog) =
                        self.base_mut().try_get_node_as::<ConfirmationDialog>("TuningConfirmationDialog")
                    {
                        dialog.set_text(&format!(
                            "Instrument tuning ({tuning_name}) is different to song filter ({tuning_filter})\nAre you sure?"
                        ));
                        dialog.popup_centered();
                    }
                    self.temp_song_for_confirm = Some(song);
                    self.temp_instrument_for_confirm = Some(instrument);
                    return;
                }
            }
        }
        self.base_mut().emit_signal(
            "opened_song",
            &[song.id.to_variant(), instrument.to_variant()],
        );
    }

    #[func]
    #[allow(non_snake_case)]
    fn ConfirmedInstrumentTuningIsDiff(&mut self) {
        if let (Some(song), Some(instrument)) = (
            self.temp_song_for_confirm.clone(),
            self.temp_instrument_for_confirm.clone(),
        ) {
            self.base_mut().emit_signal(
                "opened_song",
                &[song.id.to_variant(), instrument.to_variant()],
            );
        }
    }

    #[func]
    #[allow(non_snake_case)]
    fn Back(&mut self) {
        self.base_mut().emit_signal("closed", &[]);
    }
}
