use super::shared::*;

#[derive(GodotClass)]
#[class(base=Control)]
pub struct StartMenu {
    #[base]
    base: Base<Control>,
    progress_text: String,
    tween: Option<TweenHelper>,
}

#[godot_api]
impl IControl for StartMenu {
    fn init(base: Base<Control>) -> Self {
        Self {
            base,
            progress_text: String::new(),
            tween: None,
        }
    }

    fn ready(&mut self) {
        let song_count = SongRepository::global().lock().song_files().len();
        if let Some(mut song_count_label) = self.base_mut().try_get_node_as::<Label>("%SongCountLabel") {
            song_count_label.set_text(&format!("{song_count} songs"));
        }

        let tree = self.base_mut().get_tree();
        let tween = if let Some(mut obj) = self.base_mut().try_get_node_as::<VBoxContainer>("VBoxContainer") {
            let initial_pos = Vector2::new(-obj.get_size().x, obj.get_position().y);
            let tween = TweenHelper::new(
                tree,
                obj.clone().upcast(),
                "position",
                initial_pos.to_variant(),
                Vector2::new(80.0, obj.get_position().y).to_variant(),
            );
            obj.set_position(initial_pos);
            tween.to_final();
            Some(tween)
        } else {
            None
        };
        self.tween = tween;
    }

    fn process(&mut self, _delta: f64) {
        let progress_text = self.progress_text.clone();
        if let Some(mut label) = self.base_mut().try_get_node_as::<Label>("ReloadProgressLabel") {
            label.set_text(progress_text.as_str());
        }
    }
}

#[godot_api]
impl StartMenu {
    #[signal]
    fn closed();
    #[signal]
    fn song_pick_opened();
    #[signal]
    fn convert_menu_opened();
    #[signal]
    fn info_menu_opened();
    #[signal]
    fn song_list_file_changed();
    #[signal]
    fn settings_opened();

    #[func]
    #[allow(non_snake_case)]
    fn PlayButton_Pressed(&mut self) {
        self.animate_out();
        self.base_mut().emit_signal("song_pick_opened", &[]);
    }

    #[func]
    #[allow(non_snake_case)]
    fn InfoButton_Pressed(&mut self) {
        self.animate_out();
        self.base_mut().emit_signal("info_menu_opened", &[]);
    }

    #[func]
    #[allow(non_snake_case)]
    fn ConvertButton_Pressed(&mut self) {
        self.animate_out();
        self.base_mut().emit_signal("convert_menu_opened", &[]);
    }

    #[func]
    #[allow(non_snake_case)]
    fn ReloadButton_Pressed(&mut self) {
        if let Some(mut play_button) = self.base_mut().try_get_node_as::<Button>("%PlayButton") {
            play_button.set_disabled(true);
        }
        if let Some(mut convert_button) = self.base_mut().try_get_node_as::<Button>("%ConvertButton") {
            convert_button.set_disabled(true);
        }
        if let Some(mut reload_button) = self.base_mut().try_get_node_as::<Button>("%ReloadButton") {
            reload_button.set_disabled(true);
        }

        let mut repo = SongRepository::global().lock();
        let _ = repo.reload_sources(|msg| {
            self.progress_text = msg;
        });
        self.base_mut().emit_signal("song_list_file_changed", &[]);

        if let Some(mut play_button) = self.base_mut().try_get_node_as::<Button>("%PlayButton") {
            play_button.set_disabled(false);
        }
        if let Some(mut convert_button) = self.base_mut().try_get_node_as::<Button>("%ConvertButton") {
            convert_button.set_disabled(false);
        }
        if let Some(mut reload_button) = self.base_mut().try_get_node_as::<Button>("%ReloadButton") {
            reload_button.set_disabled(false);
        }
    }

    #[func]
    #[allow(non_snake_case)]
    fn QuitButton_Pressed(&mut self) {
        self.base_mut().emit_signal("closed", &[]);
    }

    #[func]
    #[allow(non_snake_case)]
    fn SettingsButton_Pressed(&mut self) {
        self.animate_out();
        self.base_mut().emit_signal("settings_opened", &[]);
    }

    pub fn animate_in(&self) {
        if let Some(tween) = &self.tween {
            tween.to_final();
        }
    }

    pub fn animate_out(&self) {
        if let Some(tween) = &self.tween {
            tween.to_initial();
        }
    }
}
