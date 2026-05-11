use crate::models::*;
use crate::services::*;
use crate::song_repository::SongRepository;
use godot::classes::{
    AudioEffectPitchShift, AudioServer, AudioStreamPlayer, AudioStreamWav, BoxMesh, Button,
    ButtonGroup, Camera3D, CheckBox, CheckButton, ColorPickerButton, ConfirmationDialog, Control,
    DirectionalLight3D, Engine, FileDialog, GradientTexture2D, GridContainer, HBoxContainer, Image,
    ImageTexture, InputEvent, InputEventMouseButton, ItemList, Label, Label3D, LineEdit, Line2D,
    MenuButton, MouseButton, OptionButton, OS, PackedScene, PlaneMesh, RichTextLabel, SceneTree,
    SpinBox, Texture2D, TextureRect, VBoxContainer,
};
use godot::classes::os::SystemDir;
use godot::classes::{IControl, INode, INode2D, INode3D, IVBoxContainer};
use godot::prelude::*;
use itertools::Itertools;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::f64;
use std::path::PathBuf;

fn load_scene<T: GodotClass + Inherits<Node>>(path: &str) -> Gd<T> {
    let packed = load::<PackedScene>(path);
    packed.instantiate_as::<T>()
}

#[derive(GodotClass)]
#[class(base=Node)]
pub struct MainScene {
    #[base]
    base: Base<Node>,
    start_menu: Option<Gd<StartMenu>>,
    song_pick: Option<Gd<SongPick>>,
    convert_menu: Option<Gd<ConvertMenu>>,
    info_page: Option<Gd<InfoPage>>,
    settings_page: Option<Gd<SettingsPage>>,
}

#[godot_api]
impl INode for MainScene {
    fn init(base: Base<Node>) -> Self {
        Self {
            base,
            start_menu: None,
            song_pick: None,
            convert_menu: None,
            info_page: None,
            settings_page: None,
        }
    }

    fn ready(&mut self) {
        self.load_menus();
        self.load_song_pick();
    }
}

#[godot_api]
impl MainScene {
    fn load_menus(&mut self) {
        self.load_start_menu();

        let mut convert_menu = load_scene::<ConvertMenu>("res://scenes/ConvertMenu.tscn");
        self.base.add_child(convert_menu.clone().upcast());
        convert_menu.connect(
            "closed".into(),
            self.base.callable("on_convert_closed"),
        );
        self.convert_menu = Some(convert_menu);

        let mut info_page = load_scene::<InfoPage>("res://scenes/InfoPage.tscn");
        self.base.add_child(info_page.clone().upcast());
        info_page.connect("closed".into(), self.base.callable("on_info_closed"));
        self.info_page = Some(info_page);

        let mut settings_page = load_scene::<SettingsPage>("res://scenes/SettingsPage.tscn");
        self.base
            .add_child(settings_page.clone().upcast());
        settings_page.connect(
            "closed".into(),
            self.base.callable("on_settings_closed"),
        );
        self.settings_page = Some(settings_page);
    }

    fn load_start_menu(&mut self) {
        let mut start_menu = load_scene::<StartMenu>("res://scenes/StartMenu.tscn");
        self.base.add_child(start_menu.clone().upcast());
        start_menu.connect("closed".into(), self.base.callable("on_start_closed"));
        start_menu.connect(
            "song_pick_opened".into(),
            self.base.callable("on_song_pick_opened"),
        );
        start_menu.connect(
            "song_list_file_changed".into(),
            self.base.callable("reload_song_list"),
        );
        start_menu.connect(
            "convert_menu_opened".into(),
            self.base.callable("on_convert_opened"),
        );
        start_menu.connect(
            "info_menu_opened".into(),
            self.base.callable("on_info_opened"),
        );
        start_menu.connect(
            "settings_opened".into(),
            self.base.callable("on_settings_opened"),
        );
        self.start_menu = Some(start_menu);
    }

    #[func]
    fn on_start_closed(&mut self) {
        self.base.get_tree().map(|mut tree| tree.quit());
    }

    #[func]
    fn on_song_pick_opened(&mut self) {
        if let Some(mut start_menu) = self.start_menu.take() {
            self.base.remove_child(start_menu.clone().upcast());
        }
        if let Some(mut convert_menu) = self.convert_menu.as_mut() {
            self.base.remove_child(convert_menu.clone().upcast());
        }
        if let Some(mut settings_page) = self.settings_page.as_mut() {
            self.base.remove_child(settings_page.clone().upcast());
        }
        if let Some(mut info_page) = self.info_page.as_mut() {
            self.base.remove_child(info_page.clone().upcast());
        }
        if let Some(song_pick) = &self.song_pick {
            self.base.add_child(song_pick.clone().upcast());
        }
    }

    #[func]
    fn on_convert_opened(&mut self) {
        if let Some(convert_menu) = &self.convert_menu {
            convert_menu.bind().animate_in();
        }
        if let Some(start_menu) = &self.start_menu {
            start_menu.bind().animate_out();
        }
    }

    #[func]
    fn on_info_opened(&mut self) {
        if let Some(info_page) = &self.info_page {
            info_page.bind().animate_in();
        }
        if let Some(start_menu) = &self.start_menu {
            start_menu.bind().animate_out();
        }
    }

    #[func]
    fn on_settings_opened(&mut self) {
        if let Some(settings_page) = &self.settings_page {
            settings_page.bind().animate_in();
        }
        if let Some(start_menu) = &self.start_menu {
            start_menu.bind().animate_out();
        }
    }

    #[func]
    fn on_convert_closed(&mut self) {
        if let Some(convert_menu) = &self.convert_menu {
            convert_menu.bind().animate_out();
        }
        if let Some(start_menu) = &self.start_menu {
            start_menu.bind().animate_in();
        }
        self.reload_song_list();
    }

    #[func]
    fn on_info_closed(&mut self) {
        if let Some(info_page) = &self.info_page {
            info_page.bind().animate_out();
        }
        if let Some(start_menu) = &self.start_menu {
            start_menu.bind().animate_in();
        }
    }

    #[func]
    fn on_settings_closed(&mut self) {
        if let Some(settings_page) = &self.settings_page {
            settings_page.bind().animate_out();
        }
        if let Some(start_menu) = &self.start_menu {
            start_menu.bind().animate_in();
        }
    }

    #[func]
    fn reload_song_list(&mut self) {
        let loaded = self.song_pick.as_ref().map(|s| s.is_visible_in_tree()).unwrap_or(false);
        if loaded {
            if let Some(song_pick) = &self.song_pick {
                song_pick.queue_free();
            }
        }
        self.load_song_pick();
        if loaded {
            if let Some(song_pick) = &self.song_pick {
                self.base.add_child(song_pick.clone().upcast());
            }
        }
    }

    fn load_song_pick(&mut self) {
        let mut song_pick = load_scene::<SongPick>("res://scenes/SongPick.tscn");
        song_pick.connect("closed".into(), self.base.callable("on_song_pick_closed"));
        song_pick.connect("opened_song".into(), self.base.callable("on_song_opened"));
        self.song_pick = Some(song_pick);
    }

    #[func]
    fn on_song_pick_closed(&mut self) {
        if let Some(song_pick) = &self.song_pick {
            self.base.remove_child(song_pick.clone().upcast());
        }
        if let Some(start_menu) = &self.start_menu {
            self.base.add_child(start_menu.clone().upcast());
            start_menu.bind().animate_in();
        }
        if let Some(convert_menu) = &self.convert_menu {
            self.base.add_child(convert_menu.clone().upcast());
        }
        if let Some(settings_page) = &self.settings_page {
            self.base.add_child(settings_page.clone().upcast());
        }
        if let Some(info_page) = &self.info_page {
            self.base.add_child(info_page.clone().upcast());
        }
    }

    #[func]
    fn on_song_opened(&mut self, folder: GString, instrument: GString) {
        if let Some(song_pick) = &self.song_pick {
            self.base.remove_child(song_pick.clone().upcast());
        }
        let mut scene = load_scene::<SongScene>("res://scenes/SongScene.tscn");
        let state = SongRepository::global()
            .lock()
            .get_song_state(&folder.to_string(), &instrument.to_string());
        if let Some(state) = state {
            scene.bind_mut().init(state);
        }
        self.base.add_child(scene.clone().upcast());
        scene.connect("closed".into(), self.base.callable("on_song_scene_closed"));
    }

    #[func]
    fn on_song_scene_closed(&mut self) {
        if let Some(song_pick) = &self.song_pick {
            self.base.add_child(song_pick.clone().upcast());
        }
    }
}

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
        if let Some(mut song_count_label) = self.base.get_node_as::<Label>("%SongCountLabel") {
            song_count_label.set_text(format!("{song_count} songs").into());
        }

        if let Some(obj) = self.base.get_node_as::<VBoxContainer>("VBoxContainer") {
            let initial_pos = Vector2::new(-obj.get_size().x, obj.get_position().y);
            let tween = TweenHelper::new(
                self.base.get_tree().unwrap(),
                obj.clone().upcast(),
                "position",
                initial_pos.to_variant(),
                Vector2::new(80.0, obj.get_position().y).to_variant(),
            );
            obj.set_position(initial_pos);
            tween.to_final();
            self.tween = Some(tween);
        }
    }

    fn process(&mut self, _delta: f64) {
        if let Some(mut label) = self.base.get_node_as::<Label>("ReloadProgressLabel") {
            label.set_text(self.progress_text.clone().into());
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
        self.base.emit_signal("song_pick_opened".into(), &[]);
    }

    #[func]
    #[allow(non_snake_case)]
    fn InfoButton_Pressed(&mut self) {
        self.animate_out();
        self.base.emit_signal("info_menu_opened".into(), &[]);
    }

    #[func]
    #[allow(non_snake_case)]
    fn ConvertButton_Pressed(&mut self) {
        self.animate_out();
        self.base.emit_signal("convert_menu_opened".into(), &[]);
    }

    #[func]
    #[allow(non_snake_case)]
    fn ReloadButton_Pressed(&mut self) {
        if let Some(mut play_button) = self.base.get_node_as::<Button>("%PlayButton") {
            play_button.set_disabled(true);
        }
        if let Some(mut convert_button) = self.base.get_node_as::<Button>("%ConvertButton") {
            convert_button.set_disabled(true);
        }
        if let Some(mut reload_button) = self.base.get_node_as::<Button>("%ReloadButton") {
            reload_button.set_disabled(true);
        }

        let mut repo = SongRepository::global().lock();
        let _ = repo.reload_sources(|msg| {
            self.progress_text = msg;
        });
        self.base.emit_signal("song_list_file_changed".into(), &[]);

        if let Some(mut play_button) = self.base.get_node_as::<Button>("%PlayButton") {
            play_button.set_disabled(false);
        }
        if let Some(mut convert_button) = self.base.get_node_as::<Button>("%ConvertButton") {
            convert_button.set_disabled(false);
        }
        if let Some(mut reload_button) = self.base.get_node_as::<Button>("%ReloadButton") {
            reload_button.set_disabled(false);
        }
    }

    #[func]
    #[allow(non_snake_case)]
    fn QuitButton_Pressed(&mut self) {
        self.base.emit_signal("closed".into(), &[]);
    }

    #[func]
    #[allow(non_snake_case)]
    fn SettingsButton_Pressed(&mut self) {
        self.animate_out();
        self.base.emit_signal("settings_opened".into(), &[]);
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

#[derive(GodotClass)]
#[class(base=VBoxContainer)]
pub struct ConvertMenu {
    #[base]
    base: Base<VBoxContainer>,
    tween: Option<TweenHelper>,
}

#[godot_api]
impl IVBoxContainer for ConvertMenu {
    fn init(base: Base<VBoxContainer>) -> Self {
        Self { base, tween: None }
    }

    fn ready(&mut self) {
        let initial_pos = Vector2::new(-self.base.get_size().x, self.base.get_position().y);
        let tween = TweenHelper::new(
            self.base.get_tree().unwrap(),
            self.base.clone().upcast(),
            "position",
            initial_pos.to_variant(),
            self.base.get_position().to_variant(),
        );
        self.base.set_position(initial_pos);
        self.tween = Some(tween);
    }
}

#[godot_api]
impl ConvertMenu {
    #[signal]
    fn closed();

    #[func]
    #[allow(non_snake_case)]
    fn ChoseButton_Pressed(&mut self) {
        if let Some(mut info_label) = self.base.get_node_as::<Label>("InfoLabel") {
            info_label.set_text("".into());
        }
        if let Some(mut dialog) = self.base.get_node_as::<FileDialog>("FileDialog") {
            let window_size = self.base.get_window().map(|w| w.get_size()).unwrap_or(Vector2i::new(1280, 720));
            dialog.set_size(Vector2i::new((window_size.x as f64 * 0.8) as i32, (window_size.y as f64 * 0.8) as i32));
            dialog.popup_centered();
        }
    }

    #[func]
    #[allow(non_snake_case)]
    fn FromDownloadsButton_Pressed(&mut self) {
        let downloads = OS::singleton().get_system_dir(SystemDir::Downloads);
        if let Some(mut info_label) = self.base.get_node_as::<Label>("InfoLabel") {
            info_label.set_text(format!("Loading from {}", downloads).into());
        }
        let files = std::fs::read_dir(downloads.to_string())
            .map(|entries| {
                entries
                    .flatten()
                    .map(|entry| entry.path())
                    .filter(|path| path.extension().map(|ext| ext == "psarc").unwrap_or(false))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        self.convert_files(files);
    }

    #[func]
    #[allow(non_snake_case)]
    fn Dir_Selected(&mut self, dir: GString) {
        let files = std::fs::read_dir(dir.to_string())
            .map(|entries| {
                entries
                    .flatten()
                    .map(|entry| entry.path())
                    .filter(|path| path.extension().map(|ext| ext == "psarc").unwrap_or(false))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        self.convert_files(files);
    }

    #[func]
    #[allow(non_snake_case)]
    fn File_Selected(&mut self, path: GString) {
        self.convert_files(vec![PathBuf::from(path.to_string())]);
    }

    #[func]
    #[allow(non_snake_case)]
    fn Files_Selected(&mut self, paths: PackedStringArray) {
        let files = paths
            .iter_shared()
            .map(|p| PathBuf::from(p.to_string()))
            .collect::<Vec<_>>();
        self.convert_files(files);
    }

    fn convert_files(&mut self, files: Vec<PathBuf>) {
        if let Some(mut info_label) = self.base.get_node_as::<Label>("InfoLabel") {
            let psarc_files: Vec<PathBuf> = files
                .into_iter()
                .filter(|path| path.extension().map(|ext| ext == "psarc").unwrap_or(false))
                .collect();
            if psarc_files.is_empty() {
                info_label.set_text("No valid .psarc files found".into());
                return;
            }
            let mut completed = 0;
            let mut failed = 0;
            for psarc in psarc_files {
                match SongRepository::global().lock().add_source(&psarc) {
                    Ok(_) => completed += 1,
                    Err(_) => failed += 1,
                }
            }
            info_label.set_text(format!("Completed: {completed}, failed: {failed}").into());
        }
    }

    #[func]
    #[allow(non_snake_case)]
    fn BackButton_Pressed(&mut self) {
        if let Some(tween) = &self.tween {
            tween.to_initial();
        }
        self.base.emit_signal("closed".into(), &[]);
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

#[derive(GodotClass)]
#[class(base=VBoxContainer)]
pub struct InfoPage {
    #[base]
    base: Base<VBoxContainer>,
    tween: Option<TweenHelper>,
}

#[godot_api]
impl IVBoxContainer for InfoPage {
    fn init(base: Base<VBoxContainer>) -> Self {
        Self { base, tween: None }
    }

    fn ready(&mut self) {
        let tween = TweenHelper::new(
            self.base.get_tree().unwrap(),
            self.base.clone().upcast(),
            "position",
            Vector2::new(-500.0, self.base.get_position().y).to_variant(),
            self.base.get_position().to_variant(),
        );
        self.base.set_position(Vector2::new(-500.0, self.base.get_position().y));
        self.tween = Some(tween);
    }
}

#[godot_api]
impl InfoPage {
    #[signal]
    fn closed();

    #[func]
    #[allow(non_snake_case)]
    fn BackButton_Pressed(&mut self) {
        self.base.emit_signal("closed".into(), &[]);
    }

    #[func]
    #[allow(non_snake_case)]
    fn ProjectSourceButton_Pressed(&mut self) {
        OS::singleton().shell_open("https://github.com/Murph9/tabplayerV2".into());
    }

    #[func]
    #[allow(non_snake_case)]
    fn OpenConfigFolder_Pressed(&mut self) {
        let folder = OS::singleton().get_user_data_dir();
        OS::singleton().shell_open(format!("file://{folder}").into());
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

#[derive(GodotClass)]
#[class(base=VBoxContainer)]
pub struct SettingsPage {
    #[base]
    base: Base<VBoxContainer>,
    settings: Settings,
    tween: Option<TweenHelper>,
    color_pickers: Vec<Gd<ColorPickerButton>>,
}

#[godot_api]
impl IVBoxContainer for SettingsPage {
    fn init(base: Base<VBoxContainer>) -> Self {
        Self {
            base,
            settings: SettingsService::settings(),
            tween: None,
            color_pickers: Vec::new(),
        }
    }

    fn ready(&mut self) {
        self.settings = SettingsService::settings();
        self.color_pickers = Vec::new();
        self.build_ui();
        let tween = TweenHelper::new(
            self.base.get_tree().unwrap(),
            self.base.clone().upcast(),
            "position",
            Vector2::new(-500.0, self.base.get_position().y).to_variant(),
            self.base.get_position().to_variant(),
        );
        self.base.set_position(Vector2::new(-500.0, self.base.get_position().y));
        self.tween = Some(tween);
    }
}

#[godot_api]
impl SettingsPage {
    #[signal]
    fn closed();

    fn build_ui(&mut self) {
        let mut header = HBoxContainer::new_alloc();
        let mut title = Label::new();
        title.set_text("Settings".into());
        header.add_child(title.upcast());
        let mut exit_button = Button::new();
        exit_button.set_text("Save and Close".into());
        exit_button.connect("pressed".into(), self.base.callable("on_close_pressed"));
        header.add_child(exit_button.upcast());
        self.base.add_child(header.upcast());

        let mut label = Label::new();
        label.set_text("Set String Colours (low to high):".into());
        self.base.add_child(label.upcast());

        for i in 0..6 {
            let string_char = DisplayConst::STRING_LABELS[i];
            let mut box_container = HBoxContainer::new_alloc();
            let mut string_label = Label::new();
            string_label.set_text(format!("{string_char} String").into());
            box_container.add_child(string_label.upcast());
            let mut picker = ColorPickerButton::new();
            picker.set_color(self.settings.string_colours[i]);
            picker.set_edit_alpha(false);
            let callable = self.base.callable("on_color_changed").bind(i as i64);
            picker.connect("popup_closed".into(), callable);
            self.color_pickers.push(picker.clone());
            box_container.add_child(picker.upcast());
            self.base.add_child(box_container.upcast());
        }

        let mut other_label = Label::new();
        other_label.set_text("Other Settings".into());
        self.base.add_child(other_label.upcast());

        let mut low_is_low = CheckBox::new();
        low_is_low.set_text("Low String at the bottom".into());
        low_is_low.set_button_pressed(self.settings.low_string_is_low);
        low_is_low.connect("pressed".into(), self.base.callable("on_low_is_low_toggled"));
        self.base.add_child(low_is_low.upcast());

        let mut camera_box = HBoxContainer::new_alloc();
        let mut camera_label = Label::new();
        camera_label.set_text(format!("Change Camera Aim Speed: {}", self.settings.camera_aim_speed).into());
        camera_box.add_child(camera_label.upcast());
        let mut camera_up = Button::new();
        camera_up.set_text("(+)".into());
        camera_up.connect("pressed".into(), self.base.callable("on_camera_speed_up"));
        camera_box.add_child(camera_up.upcast());
        let mut camera_down = Button::new();
        camera_down.set_text("(-)".into());
        camera_down.connect("pressed".into(), self.base.callable("on_camera_speed_down"));
        camera_box.add_child(camera_down.upcast());
        self.base.add_child(camera_box.upcast());

        let mut audio_box = HBoxContainer::new_alloc();
        let mut audio_label = Label::new();
        audio_label.set_text("AudioOffset (in Ms): ".into());
        audio_box.add_child(audio_label.upcast());
        let mut audio_offset = SpinBox::new();
        audio_offset.set_min(0.0);
        audio_offset.set_max(5000.0);
        audio_offset.set_step(1.0);
        audio_offset.set_value_no_signal(self.settings.audio_position_offset_ms);
        audio_offset.connect("value_changed".into(), self.base.callable("on_audio_offset_changed"));
        audio_box.add_child(audio_offset.upcast());
        self.base.add_child(audio_box.upcast());
    }

    #[func]
    fn on_close_pressed(&mut self) {
        self.base.emit_signal("closed".into(), &[]);
    }

    #[func]
    fn on_low_is_low_toggled(&mut self) {
        self.settings.low_string_is_low = !self.settings.low_string_is_low;
        SettingsService::update_settings(self.settings.clone());
    }

    #[func]
    fn on_camera_speed_up(&mut self) {
        self.adjust_camera_speed(1.0);
    }

    #[func]
    fn on_camera_speed_down(&mut self) {
        self.adjust_camera_speed(-1.0);
    }

    fn adjust_camera_speed(&mut self, delta: f32) {
        let new_speed = self.settings.camera_aim_speed + delta;
        if new_speed < 1.0 || new_speed > 30.0 {
            return;
        }
        self.settings.camera_aim_speed = new_speed;
        SettingsService::update_settings(self.settings.clone());
    }

    #[func]
    fn on_audio_offset_changed(&mut self, value: f64) {
        self.settings.audio_position_offset_ms = value;
        SettingsService::update_settings(self.settings.clone());
    }

    #[func]
    fn on_color_changed(&mut self, index: i64) {
        if let Some(picker) = self.color_pickers.get(index as usize) {
            let color = picker.get_color();
            if let Some(entry) = self.settings.string_colours.get_mut(index as usize) {
                *entry = color;
                SettingsService::update_settings(self.settings.clone());
            }
        }
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
            "song_selected".into(),
            self.base.callable("on_song_selected"),
        );
        let mut song_list = load_scene::<SongList>("res://scenes/SongList.tscn");
        song_list.bind_mut().set_display(song_display.clone());
        song_list.connect("song_selected".into(), song_display.callable("song_changed"));
        self.song_list = Some(song_list.clone());
        self.song_display = Some(song_display);

        if let Some(mut vbox) = self.base.get_node_as::<VBoxContainer>("MarginContainer/VBoxContainer") {
            vbox.add_child(song_list.clone().upcast());
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
                self.base.emit_signal(
                    "opened_song".into(),
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
                        self.base.get_node_as::<ConfirmationDialog>("TuningConfirmationDialog")
                    {
                        dialog.set_dialog_text(format!(
                            "Instrument tuning ({tuning_name}) is different to song filter ({tuning_filter})\nAre you sure?"
                        ).into());
                        dialog.popup_centered();
                    }
                    self.temp_song_for_confirm = Some(song);
                    self.temp_instrument_for_confirm = Some(instrument);
                    return;
                }
            }
        }
        self.base.emit_signal(
            "opened_song".into(),
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
            self.base.emit_signal(
                "opened_song".into(),
                &[song.id.to_variant(), instrument.to_variant()],
            );
        }
    }

    #[func]
    #[allow(non_snake_case)]
    fn Back(&mut self) {
        self.base.emit_signal("closed".into(), &[]);
    }
}

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

            if let Some(mut album_rect) = self.base.get_node_as::<TextureRect>("AlbumArtTextureRect") {
                album_rect.set_texture(GradientTexture2D::new().upcast());
            }
        if let Some(mut label) = self.base.get_node_as::<Label>("ArtistLabel") {
            label.set_text(format!("Artist: {}", song_info.artist).into());
        }
        if let Some(mut label) = self.base.get_node_as::<Label>("SongNameLabel") {
            label.set_text(format!("Name: {}", song_info.song_name).into());
        }
        if let Some(mut label) = self.base.get_node_as::<Label>("AlbumLabel") {
            label.set_text(format!("Album: {}", song_info.album).into());
        }
        if let Some(mut label) = self.base.get_node_as::<Label>("YearLabel") {
            label.set_text(format!("Year: {:?}", song_info.year).into());
        }
        if let Some(mut label) = self.base.get_node_as::<Label>("OtherLabel") {
            label.set_text(format!("Length: {}", to_min_sec(song_info.length as f64, false)).into());
        }

        if let Some(mut grid) = self.base.get_node_as::<GridContainer>("InstrumentGridContainer") {
            for child in grid.get_children() {
                grid.remove_child(child);
            }
            grid.add_child(Label::new().upcast());
            let mut tuning_label = Label::new();
            tuning_label.set_text("Tuning".into());
            grid.add_child(tuning_label.upcast());
            let mut note_label = Label::new();
            note_label.set_text("Note Counts".into());
            grid.add_child(note_label.upcast());
            let mut density_label = Label::new();
            density_label.set_text("Note Density".into());
            grid.add_child(density_label.upcast());
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
                let mut button = Button::new();
                button.set_text(format!("Play {}", instrument.name).into());
                let folder_name = self.folder_name.clone().unwrap_or_default();
                let instrument_name = instrument.name.clone();
                let callable = self.base.callable("emit_song_selected").bind(
                    folder_name.to_variant(),
                    instrument_name.to_variant(),
                );
                button.connect("pressed".into(), callable);
                grid.add_child(button.upcast());
                let mut label = Label::new();
                label.set_text(Instrument::calc_tuning_name(instrument.tuning, instrument.capo_fret).into());
                grid.add_child(label.upcast());
                let mut label = Label::new();
                label.set_text(format!("{}", instrument.note_count).into());
                grid.add_child(label.upcast());
                let mut label = Label::new();
                label.set_text(to_fixed_places(instrument.note_density(&song_info) as f64, 2, false).into());
                grid.add_child(label.upcast());
            }
        }
    }

    #[func]
    fn emit_song_selected(&mut self, folder: Variant, instrument: Variant) {
        self.base.emit_signal("song_selected".into(), &[folder, instrument]);
    }
}

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
                    create_label(&song.song_name, &song.id, self.base.callable("on_row_input")),
                    create_label(&song.artist, &song.id, self.base.callable("on_row_input")),
                    create_label(&song.album, &song.id, self.base.callable("on_row_input")),
                    create_label(&song.year.map(|y| y.to_string()).unwrap_or_default(), &song.id, self.base.callable("on_row_input")),
                    create_label(&to_min_sec(song.length as f64, false), &song.id, self.base.callable("on_row_input")),
                    create_label(&song.instrument_chars(), &song.id, self.base.callable("on_row_input")),
                ],
                song,
                selected: false,
            })
            .collect();

        if let Some(display) = &self.song_display {
            if let Some(mut split) = self
                .base
                .get_node_as::<VBoxContainer>("HSplitContainer/VBoxContainerDetails")
            {
                split.add_child(display.clone().upcast());
            }
        }

        let mut group = ButtonGroup::new();
        group.connect("pressed".into(), self.base.callable("Heading_Pressed"));
        if let Some(mut grid) = self.base.get_node_as::<GridContainer>("%GridContainer") {
            let headings = ["Song Name", "Artist", "Album", "Year", "Length", "Parts"];
            grid.set_columns(headings.len() as i32);
            for heading in headings.iter() {
                let mut button = Button::new();
                button.set_text(heading.to_string().into());
                button.set_button_group(group.clone());
                button.set_toggle_mode(true);
                grid.add_child(button.upcast());
            }
        }

        if let Some(mut tuning_select) = self.base.get_node_as::<OptionButton>("HBoxContainer/TuningOptionButton") {
            tuning_select.add_item("".into());
            let tunings = self
                .rows
                .iter()
                .filter_map(|row| row.song.main_instrument())
                .map(|inst| Instrument::calc_tuning_name(inst.tuning, inst.capo_fret))
                .unique();
            for tuning in tunings {
                tuning_select.add_item(tuning.into());
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
        self.base.emit_signal("song_selected".into(), &[folder.to_variant()]);
    }

    #[func]
    fn on_row_input(&mut self, event: Gd<InputEvent>, folder: GString) {
        if let Some(event) = event.try_cast::<InputEventMouseButton>() {
            if event.is_pressed() && event.get_button_index() == MouseButton::LEFT {
                self.base.emit_signal("song_selected".into(), &[folder.to_variant()]);
            }
        }
    }

    fn load_table_rows(&mut self) {
        if let Some(mut grid) = self.base.get_node_as::<GridContainer>("%GridContainer") {
            for row in &self.rows {
                for control in &row.controls {
                    if control.get_parent().is_some() {
                        grid.remove_child(control.clone().upcast());
                    }
                    grid.add_child(control.clone().upcast());
                }
            }
        }
    }

    fn load_table_filter(&mut self) {
        let capo_shown = self
            .base
            .get_node_as::<CheckBox>("HBoxContainer/CapoCheckBox")
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
        if let Some(mut label) = self.base.get_node_as::<Label>("HBoxContainer/SongsLoadedLabel") {
            label.set_text(format!("{count_shown} songs shown").into());
        }
    }

    #[func]
    #[allow(non_snake_case)]
    fn TuningSelected(&mut self, index: i64) {
        if let Some(tuning_select) = self.base.get_node_as::<OptionButton>("HBoxContainer/TuningOptionButton") {
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
        self.base.emit_signal("song_selected".into(), &[song.song.id.to_variant()]);
    }

    #[func]
    #[allow(non_snake_case)]
    fn ShowCapo_Pressed(&mut self) {
        self.load_table_filter();
    }
}

 

fn create_label(text: &str, folder: &str, callable: Callable) -> Gd<Control> {
    let mut label = Label::new();
    label.set_text(fixed_width_string(text, 30).into());
    label.set_mouse_filter(Control::MouseFilter::Stop);
    label.connect("gui_input".into(), callable.bind(folder.to_variant()));
    label.upcast()
}

#[derive(GodotClass)]
#[class(base=Node)]
pub struct SongScene {
    #[base]
    base: Base<Node>,
    state: Option<SongState>,
    audio_stream: Option<Gd<AudioStreamWav>>,
    player: Option<Gd<AudioStreamPlayer>>,
    cached_song_position: Option<f64>,
    note_graph_scene: Option<Gd<NoteMiniGraph>>,
    song_chart_scene: Option<Gd<SongChart>>,
    a_position: f64,
    b_position: f64,
}

#[godot_api]
impl INode for SongScene {
    fn init(base: Base<Node>) -> Self {
        Self {
            base,
            state: None,
            audio_stream: None,
            player: None,
            cached_song_position: None,
            note_graph_scene: None,
            song_chart_scene: None,
            a_position: 0.0,
            b_position: 0.0,
        }
    }

    fn ready(&mut self) {
        if let Some(state) = &self.state {
            self.set_ui_labels(state);
            if let Some(mut player) = self.base.get_node_as::<AudioStreamPlayer>("AudioStreamPlayer") {
                if let Some(stream) = &self.audio_stream {
                    player.set_stream(stream.clone().upcast());
                }
                player.play();
                self.player = Some(player.clone());
            }

            let player = self.player.clone();
            let mut guitar_chart = Gd::<GuitarChart>::from_init_fn(|mut chart| {
                chart.state = Some(state.clone());
                chart.audio_player = player.clone();
            });
            self.base.add_child(guitar_chart.clone().upcast());

            self.load_instrument_from_state();
        }
    }

    fn process(&mut self, delta: f64) {
        self.cached_song_position = None;
        let song_position = self.get_song_position();
        if let Some(mut label) = self.base.get_node_as::<Label>("GridContainer/ABLabelStart") {
            label.set_text(if self.a_position == 0.0 { "".into() } else { to_min_sec(self.a_position, true).into() });
        }
        if let Some(mut label) = self.base.get_node_as::<Label>("GridContainer/ABLabelEnd") {
            label.set_text(if self.b_position == 0.0 { "".into() } else { to_min_sec(self.b_position, true).into() });
        }
        if self.a_position != 0.0 && self.b_position != 0.0 {
            if self.a_position < song_position && delta + song_position > self.b_position {
                if let Some(player) = &self.player {
                    player.set_stream_paused(false);
                    player.seek(self.a_position as f32);
                }
            }
        }
        if let Some(player) = &self.player {
            if !player.is_playing() {
                return;
            }
        }
        if let Some(state) = &self.state {
            if let Some(next_note) = self.next_note_block(state, song_position) {
                if let Some(mut label) = self.base.get_node_as::<Label>("GridContainer/SkipToNextLabel2") {
                    label.set_text(format!("at {}", to_min_sec(next_note.time as f64, false)).into());
                }
            }
            if let Some(mut details) = self.base.get_node_as::<Label>("DetailsVBoxContainer/SongDetailsLabel") {
                let instrument = state.instrument();
                let note_text = if let Some(next_note) = self.next_note_block(state, song_position) {
                    format!(
                        "Next Note: {} in {:.1}",
                        to_min_sec(next_note.time as f64, false),
                        next_note.time as f64 - song_position
                    )
                } else {
                    "No note".to_string()
                };
                details.set_text(format!(
                    "---------\nTuning: {}\nNotes: {}\nChords: {}\nFirst note @ {}\nLast note @ {}\n---------\n{note_text}\n",
                    Instrument::calc_tuning_name(instrument.config.tuning, instrument.config.capo_fret),
                    instrument.single_note_count(),
                    instrument.chord_count(),
                    to_min_sec(instrument.notes.first().map(|n| n.time as f64).unwrap_or(0.0), false),
                    to_min_sec(instrument.notes.last().map(|n| n.time as f64).unwrap_or(0.0), false),
                ).into());
            }
            if let Some(mut label) = self.base.get_node_as::<Label>("RunningDetailsLabel") {
                label.set_text(format!(
                    "{}fps | {:03.1}ms\n{}",
                    Engine::get_frames_per_second(),
                    delta * 1000.0,
                    to_min_sec(song_position, true)
                ).into());
            }
            if let Some(mut lyrics_label) = self.base.get_node_as::<RichTextLabel>("HBoxContainer/LyricsLabel") {
                self.update_lyrics(state, song_position, &mut lyrics_label);
            }
            if let Some(mut pos_line) = self.base.get_node_as::<LineEdit>("GridContainer/PositionSetLineEdit") {
                pos_line.set_text(to_min_sec(song_position, true).into());
            }
            if let Some(mut speed_label) = self.base.get_node_as::<Label>("GridContainer/SongSpeedLabel") {
                if let Some(player) = &self.player {
                    speed_label.set_text(format!("{:.1}%", player.get_pitch_scale() * 100.0).into());
                }
            }
        }
    }
}

#[godot_api]
impl SongScene {
    #[signal]
    fn closed();

    pub fn init(&mut self, state: SongState) {
        let mut stream = AudioStreamWav::new();
        stream.set_format(AudioStreamWav::FORMAT_16_BITS);
        stream.set_mix_rate(state.audio_sample_rate);
        stream.set_stereo(state.audio_channels >= 2);
        stream.set_data(PackedByteArray::from(state.audio.clone()));
        self.audio_stream = Some(stream);
        self.state = Some(state);
    }

    #[func]
    #[allow(non_snake_case)]
    fn InstrumentChanged(&mut self, id: i64) {
        if let Some(state) = &mut self.state {
            let instrument = state
                .song_info
                .instruments
                .get(id as usize)
                .map(|inst| inst.name.clone());
            if let Some(name) = instrument {
                state.instrument_name = name.clone();
                if let Some(mut list) = self.base.get_node_as::<MenuButton>("DetailsVBoxContainer/VBoxContainer/InstrumentMenuButton") {
                    list.set_text(name.into());
                }
                self.load_instrument_from_state();
            }
        }
    }

    fn load_instrument_from_state(&mut self) {
        if let Some(mut note_graph) = self.note_graph_scene.take() {
            self.base.remove_child(note_graph.clone().upcast());
        }
        if let Some(state) = &self.state {
            let player = self.player.clone();
            let mut note_graph = Gd::<NoteMiniGraph>::from_init_fn(|mut graph| {
                graph.song_state = Some(state.clone());
                graph.audio_player = player.clone();
            });
            self.base.add_child(note_graph.clone().upcast());
            self.note_graph_scene = Some(note_graph);

            if let Some(mut song_chart) = self.song_chart_scene.take() {
                self.base.remove_child(song_chart.clone().upcast());
            }
            let mut song_chart = Gd::<SongChart>::from_init_fn(|mut chart| {
                chart.instrument = Some(state.instrument().clone());
            });
            self.base.add_child(song_chart.clone().upcast());
            self.song_chart_scene = Some(song_chart);
        }
    }

    fn set_ui_labels(&mut self, state: &SongState) {
        if let Some(mut info_label) = self.base.get_node_as::<Label>("DetailsVBoxContainer/SongInfoLabel") {
            info_label.set_text(format!(
                "{} ({:?})\n{}\n{}",
                state.song_info.metadata.name,
                state.song_info.metadata.year,
                state.song_info.metadata.artist,
                state.song_info.metadata.album
            ).into());
        }
        if let Some(mut instrument_list) = self.base.get_node_as::<MenuButton>("DetailsVBoxContainer/VBoxContainer/InstrumentMenuButton") {
            instrument_list.set_text(state.instrument().name.clone().into());
            if let Some(mut popup) = instrument_list.get_popup() {
                popup.clear();
                for instrument in &state.song_info.instruments {
                    popup.add_item(instrument.name.clone().into());
                }
                popup.set_item_checked(state.song_info.main_instrument_index as i32, true);
                popup.connect("id_pressed".into(), self.base.callable("InstrumentChanged"));
            }
        }
    }

    #[func]
    #[allow(non_snake_case)]
    fn PauseButton_Pressed(&mut self) {
        if let Some(player) = &self.player {
            if player.is_stream_paused() {
                self.resume();
            } else {
                self.pause();
            }
        }
    }

    fn pause(&mut self) {
        if let Some(player) = &self.player {
            player.set_stream_paused(true);
        }
    }

    fn resume(&mut self) {
        if let Some(player) = &self.player {
            player.set_stream_paused(false);
        }
    }

    #[func]
    #[allow(non_snake_case)]
    fn Quit(&mut self) {
        self.pause();
        if let Some(player) = &self.player {
            player.stop();
        }
        self.base.emit_signal("closed".into(), &[]);
    }

    #[func]
    #[allow(non_snake_case)]
    fn Skip10Sec(&mut self) {
        self.skip_10_sec();
    }

    #[func]
    #[allow(non_snake_case)]
    fn Back10Sec(&mut self) {
        self.back_10_sec();
    }

    #[func]
    #[allow(non_snake_case)]
    fn SkipToNext(&mut self) {
        self.skip_to_next();
    }

    #[func]
    #[allow(non_snake_case)]
    fn RestartSong(&mut self) {
        self.restart_song();
    }

    #[func]
    #[allow(non_snake_case)]
    fn SlowDownPlayback(&mut self) {
        self.slow_down();
    }

    #[func]
    #[allow(non_snake_case)]
    fn SpeedUpPlayback(&mut self) {
        self.speed_up();
    }

    #[func]
    #[allow(non_snake_case)]
    fn ResetSongSpeed(&mut self) {
        self.reset_song_speed();
    }

    #[func]
    #[allow(non_snake_case)]
    fn PickA(&mut self) {
        self.pick_a();
    }

    #[func]
    #[allow(non_snake_case)]
    fn PickB(&mut self) {
        self.pick_b();
    }

    #[func]
    #[allow(non_snake_case)]
    fn ClearLoopTimes(&mut self) {
        self.clear_loop_times();
    }

    #[func]
    #[allow(non_snake_case)]
    fn SongFinished(&mut self) {
        if let Some(player) = &self.player {
            player.play();
            player.seek(0.0);
            player.set_stream_paused(true);
            self.pause();
        }
    }

    #[func]
    fn _input(&mut self, event: Gd<InputEvent>) {
        if event.is_action_pressed("ui_cancel".into()) {
            self.Quit();
        } else if event.is_action_pressed("song_pause".into()) {
            self.PauseButton_Pressed();
        } else if event.is_action_pressed("song_skip_forward_10".into()) {
            self.skip_10_sec();
        } else if event.is_action_pressed("song_skip_backward_10".into()) {
            self.back_10_sec();
        } else if event.is_action_pressed("song_skip_to_next".into()) {
            self.skip_to_next();
        } else if event.is_action_pressed("song_restart".into()) {
            self.restart_song();
        } else if event.is_action_pressed("song_speed_down".into()) {
            self.slow_down();
        } else if event.is_action_pressed("song_speed_up".into()) {
            self.speed_up();
        } else if event.is_action_pressed("song_set_loop_start".into()) {
            self.pick_a();
        } else if event.is_action_pressed("song_set_loop_end".into()) {
            self.pick_b();
        } else if event.is_action_pressed("song_reset_loop".into()) {
            self.clear_loop_times();
        } else if event.is_action_pressed("song_reset_speed".into()) {
            self.reset_song_speed();
        }
    }

    fn skip_10_sec(&mut self) {
        if let Some(player) = &self.player {
            player.seek((self.get_song_position() + 10.0) as f32);
        }
    }

    fn back_10_sec(&mut self) {
        if let Some(player) = &self.player {
            player.seek((self.get_song_position() - 10.0) as f32);
        }
    }

    fn skip_to_next(&mut self) {
        if let Some(state) = &self.state {
            if let Some(next_note) = self.next_note_block(state, self.get_song_position()) {
                if let Some(player) = &self.player {
                    player.seek(next_note.time - 1.5);
                }
            }
        }
    }

    fn restart_song(&mut self) {
        if let Some(player) = &self.player {
            player.seek(0.0);
        }
    }

    fn slow_down(&mut self) {
        self.adjust_pitch(0.98);
    }

    fn speed_up(&mut self) {
        self.adjust_pitch(1.0 / 0.98);
    }

    fn adjust_pitch(&mut self, amount: f32) {
        if let Some(player) = &self.player {
            if player.get_pitch_scale() < 0.5 && amount < 1.0 {
                return;
            }
            if player.get_pitch_scale() > 1.7 && amount > 1.0 {
                return;
            }
            self.set_song_speed(player.get_pitch_scale() * amount);
        }
    }

    fn reset_song_speed(&mut self) {
        self.set_song_speed(1.0);
    }

    fn set_song_speed(&mut self, fraction: f32) {
        if let Some(player) = &self.player {
            player.set_pitch_scale(fraction);
            if fraction == 1.0 {
                player.set_bus("Master".into());
            } else {
                player.set_bus("SongPlayback".into());
            }
            let bus_id = AudioServer::singleton().get_bus_index("SongPlayback".into());
            if let Some(mut effect) = AudioServer::singleton()
                .get_bus_effect(bus_id, 0)
                .and_then(|e| e.try_cast::<AudioEffectPitchShift>())
            {
                effect.set_pitch_scale(1.0 / player.get_pitch_scale());
            }
        }
    }

    fn pick_a(&mut self) {
        self.a_position = self.get_song_position();
    }

    fn pick_b(&mut self) {
        self.b_position = self.get_song_position();
    }

    fn clear_loop_times(&mut self) {
        self.a_position = 0.0;
        self.b_position = 0.0;
    }

    #[func]
    #[allow(non_snake_case)]
    fn MoveSongPosition(&mut self) {
        if let Some(mut line_edit) = self.base.get_node_as::<LineEdit>("GridContainer/PositionSetLineEdit") {
            let text = line_edit.get_text().to_string();
            let mut pos = text.parse::<f32>().unwrap_or(0.0);
            if pos == 0.0 {
                if let Some(parts) = parse_min_sec(&text) {
                    pos = parts;
                }
            }
            if pos <= 0.0 {
                return;
            }
            if let Some(player) = &self.player {
                if pos > player.get_stream().map(|s| s.get_length()).unwrap_or(0.0) {
                    return;
                }
                player.set_stream_paused(false);
                player.seek(pos);
                self.pause();
            }
        }
    }

    pub fn get_song_position(&mut self) -> f64 {
        if let Some(cached) = self.cached_song_position {
            return cached;
        }
        if let Some(player) = &self.player {
            let mut time = calculate_song_position(player);
            time -= SettingsService::settings().audio_position_offset_ms / 1000.0;
            self.cached_song_position = Some(time);
            return time;
        }
        0.0
    }

    fn next_note_block(&self, state: &SongState, song_pos: f64) -> Option<&NoteBlock> {
        state.instrument().notes.iter().find(|n| n.time as f64 > song_pos)
    }

    fn update_lyrics(&self, state: &SongState, song_pos: f64, label: &mut Gd<RichTextLabel>) {
        label.clear();
        label.push_font_size(40);
        let lines = current_lines(state, song_pos);
        if lines.is_empty() {
            label.set_text("".into());
            return;
        }
        if let Some(line) = lines.get(0) {
            let (part_a, part_b) = line.get_parts(song_pos);
            label.push_color(Color::from_rgb(1.0, 0.0, 0.0));
            label.add_text(part_a.into());
            label.push_color(Color::from_rgb(1.0, 1.0, 0.0));
            label.add_text(part_b.into());
        }
        if let Some(line) = lines.get(1) {
            label.add_text(format!("\n{}", line.text()).into());
        }
    }
}

fn parse_min_sec(text: &str) -> Option<f32> {
    let parts: Vec<&str> = text.split_whitespace().collect();
    if parts.len() < 3 {
        return None;
    }
    let minutes = parts.get(0)?.trim_end_matches('m').parse::<f32>().ok()?;
    let seconds = parts.get(1)?.trim_end_matches('s').parse::<f32>().ok()?;
    let millis = parts.get(2)?.trim_end_matches("ms").parse::<f32>().ok()?;
    Some(minutes * 60.0 + seconds + millis / 1000.0)
}

fn calculate_song_position(player: &Gd<AudioStreamPlayer>) -> f64 {
    let mut time = player.get_playback_position() as f64 + AudioServer::singleton().get_time_since_last_mix() as f64;
    time -= AudioServer::singleton().get_output_latency() as f64;
    time
}

fn current_lines(state: &SongState, song_pos: f64) -> Vec<LyricLine> {
    if state.song_info.lyrics.lines.is_empty() {
        return Vec::new();
    }
    if state.song_info.lyrics.lines.first().map(|line| line.start_time as f64 > song_pos).unwrap_or(false) {
        return state.song_info.lyrics.lines.iter().take(2).cloned().collect();
    }
    for line in &state.song_info.lyrics.lines {
        if line.end_time as f64 >= song_pos {
            return state.song_info.lyrics.lines.iter().skip_while(|x| x.start_time != line.start_time).take(2).cloned().collect();
        }
    }
    Vec::new()
}

#[derive(GodotClass)]
#[class(base=Node3D)]
pub struct GuitarChart {
    #[base]
    base: Base<Node3D>,
    pub state: Option<SongState>,
    pub audio_player: Option<Gd<AudioStreamPlayer>>,
    last_note_block_time: Option<f32>,
    last_note_block_node: Option<Gd<Node3D>>,
}

#[godot_api]
impl INode3D for GuitarChart {
    fn init(base: Base<Node3D>) -> Self {
        Self {
            base,
            state: None,
            audio_player: None,
            last_note_block_time: None,
            last_note_block_node: None,
        }
    }

    fn ready(&mut self) {
        let mut material = StandardMaterial3D::new();
        material.set_albedo_color(Color::from_rgb(0.82, 0.71, 0.55));
        let mut plane_mesh = PlaneMesh::new();
        plane_mesh.set_size(Vector2::new(6.0, 6.0));
        plane_mesh.set_center_offset(Vector3::new(2.5, 0.0, -3.0));
        plane_mesh.set_material(material.upcast());
        let mut plane = MeshInstance3D::new();
        plane.set_transform(Transform3D::new(
            Basis::from_rows(Vector3::new(0.0, -1.0, 0.0), Vector3::new(1.0, 0.0, 0.0), Vector3::new(0.0, 0.0, 1.0)),
            Vector3::ZERO,
        ));
        plane.set_mesh(plane_mesh.upcast());
        self.base.add_child(plane.upcast());

        let mut camera = Camera3D::new();
        camera.set_fov(60.0);
        camera.set_transform(Transform3D::new(
            Basis::from_rows(
                Vector3::new(0.0, 0.310809, -0.950472),
                Vector3::new(0.0, 0.950472, 0.310809),
                Vector3::new(1.0, 0.0, 0.0),
            ),
            Vector3::new(-12.0, 10.0, 8.0),
        ));
        self.base.add_child(camera.upcast());

        let mut light = DirectionalLight3D::new();
        light.set_transform(Transform3D::new(
            Basis::from_rows(
                Vector3::new(-0.177838, 0.752991, -0.633544),
                Vector3::new(-0.317607, 0.565433, 0.761191),
                Vector3::new(0.931397, 0.336587, 0.1386),
            ),
            Vector3::ZERO,
        ));
        self.base.add_child(light.upcast());

        if let Some(state) = &self.state {
            for i in 0..6 {
                let colour = SettingsService::get_color_from_string_num(i);
                let mut string_material = StandardMaterial3D::new();
                string_material.set_albedo_color(colour);
                let mut string_mesh = BoxMesh::new();
                string_mesh.set_size(Vector3::new(0.08, 0.08, 50.0));
                string_mesh.set_material(string_material.upcast());
                let mut string_obj = MeshInstance3D::new();
                string_obj.set_transform(Transform3D::new(
                    Basis::IDENTITY,
                    Vector3::new(0.0, DisplayConst::calc_note_height_y(i as i32), 25.0),
                ));
                string_obj.set_mesh(string_mesh.upcast());
                self.base.add_child(string_obj.upcast());
            }

            let mut fret_material = StandardMaterial3D::new();
            fret_material.set_albedo_color(Color::from_rgb(0.82, 0.71, 0.55));
            let mut fret_mesh = BoxMesh::new();
            fret_mesh.set_size(Vector3::new(0.03, 5.0 + DisplayConst::TRACK_BOTTOM_WORLD.abs() * 2.0, 0.03));
            fret_mesh.set_material(fret_material.upcast());
            for i in 0..25 {
                let mut fret_obj = MeshInstance3D::new();
                fret_obj.set_transform(Transform3D::new(
                    Basis::IDENTITY,
                    Vector3::new(0.0, 2.5, DisplayConst::calc_fret_pos_z(i)),
                ));
                fret_obj.set_mesh(fret_mesh.clone().upcast());
                self.base.add_child(fret_obj.upcast());
            }
        }
    }

    fn process(&mut self, delta: f64) {
        let song_pos = self.audio_player.as_ref().map(|player| calculate_song_position(player)).unwrap_or(0.0);
        if let Some(state) = &self.state {
            if let Some(note_block) = state.instrument().notes.iter().find(|b| b.time as f64 > song_pos) {
                if let Some(mut cam) = self.base.get_tree().and_then(|tree| tree.get_root().and_then(|root| root.get_camera_3d())) {
                    let cam_move_speed = SettingsService::settings().camera_aim_speed as f64 / 50.0;
                    let want_pos = DisplayConst::calc_middle_window_z(note_block.fret_window_start, note_block.fret_window_length);
                    let new_z = cam.get_position().z * (1.0 - delta * cam_move_speed as f64) + want_pos as f64 * delta * cam_move_speed as f64;
                    cam.set_position(Vector3::new(cam.get_position().x, cam.get_position().y, new_z as f32));
                }
                if self.last_note_block_time != Some(note_block.time) {
                    self.last_note_block_time = Some(note_block.time);
                    if let Some(node) = &self.last_note_block_node {
                        node.queue_free();
                    }
                    let mut block_node = Node3D::new();
                    self.base.add_child(block_node.clone().upcast());
                    for note in &note_block.notes {
                        let note_node = NoteGenerator::get_basic_note(
                            note,
                            state.instrument().config,
                            0.2 / state.instrument().config.note_speed,
                            note_block.fret_window_start,
                            note_block.fret_window_length,
                        );
                        block_node.add_child(note_node);
                    }
                    self.last_note_block_node = Some(block_node);
                }
            }
        }
    }
}

#[derive(GodotClass)]
#[class(base=Node2D)]
pub struct NoteMiniGraph {
    #[base]
    base: Base<Node2D>,
    pub song_state: Option<SongState>,
    pub audio_player: Option<Gd<AudioStreamPlayer>>,
    note_plot_image: Option<Gd<Texture2D>>,
}

#[derive(GodotClass)]
#[class(base=Node)]
pub struct NoteBucketGraph {
    #[base]
    base: Base<Node>,
}

#[godot_api]
impl INode for NoteBucketGraph {
    fn init(base: Base<Node>) -> Self {
        Self { base }
    }
}

#[godot_api]
impl INode2D for NoteMiniGraph {
    fn init(base: Base<Node2D>) -> Self {
        Self {
            base,
            song_state: None,
            audio_player: None,
            note_plot_image: None,
        }
    }

    fn ready(&mut self) {
        if let Some(state) = &self.song_state {
            let string_colours = SettingsService::settings().string_colours;
            let window = self.base.get_viewport().unwrap().get_visible_rect();
            let left_offset = (0.07 * window.size.x) as i32;
            let note_offset = ((1.0 - 0.07 * 2.0) * window.size.x) as i32;
            let mut image = Image::create_empty(window.size.x as i32, window.size.y as i32, false, Image::FORMAT_RGBA8);
            image.fill_rect(
                Rect2i::new(Vector2i::new(left_offset, window.size.y as i32 - 30 - 25 * 3), Vector2i::new(note_offset, window.size.y as i32 - 30)),
                Color::from_rgba(1.0, 1.0, 1.0, 0.05),
            );
            for note_block in &state.instrument().notes {
                let pos_x = left_offset as f32 + note_offset as f32 * note_block.time / state.song_info.metadata.song_length;
                for note in &note_block.notes {
                    let pos_y = window.size.y as i32 - note.fret_num * 3 - 30;
                    draw_note_pixel(&mut image, string_colours[note.string_num as usize], Vector2i::new(pos_x as i32, pos_y));
                }
            }
            let texture = ImageTexture::create_from_image(image);
            self.note_plot_image = Some(texture.upcast());
        }
    }

    fn draw(&mut self) {
        if let Some(texture) = &self.note_plot_image {
            self.base.draw_texture(texture.clone(), Vector2::ZERO, Color::from_rgb(1.0, 1.0, 1.0));
        }
        if let (Some(state), Some(player)) = (&self.song_state, &self.audio_player) {
            let window = self.base.get_viewport().unwrap().get_visible_rect();
            let left_offset = (0.07 * window.size.x) as f32;
            let note_offset = (1.0 - 0.07 * 2.0) * window.size.x;
            let pos_x = left_offset + note_offset * calculate_song_position(player) as f32 / state.song_info.metadata.song_length;
            self.base.draw_line(
                Vector2::new(pos_x, window.size.y - 30.0),
                Vector2::new(pos_x, window.size.y - 30.0 - 24.0 * 3.0),
                Color::from_rgb(1.0, 1.0, 1.0),
                1.0,
                false,
            );
        }
    }

    fn process(&mut self, _delta: f64) {
        if let Some(player) = &self.audio_player {
            if calculate_song_position(player) > 0.0 {
                self.base.queue_redraw();
            }
        }
    }
}

fn draw_note_pixel(image: &mut Gd<Image>, colour: Color, pos: Vector2i) {
    for i in -1..1 {
        for j in -1..1 {
            image.set_pixel(pos.x + i, pos.y + j, colour);
        }
    }
}

#[derive(GodotClass)]
#[class(base=Node3D)]
pub struct SongChart {
    #[base]
    base: Base<Node3D>,
    pub instrument: Option<Instrument>,
    last_chord: Option<NoteBlock>,
}

#[godot_api]
impl INode3D for SongChart {
    fn init(base: Base<Node3D>) -> Self {
        Self {
            base,
            instrument: None,
            last_chord: None,
        }
    }

    fn ready(&mut self) {
        if let Some(instrument) = &self.instrument {
            let items = self.load_notes(instrument);
            for item in items {
                self.base.add_child(item);
            }
        }
    }
}

#[godot_api]
impl SongChart {
    fn load_notes(&mut self, instrument: &Instrument) -> Vec<Gd<Node3D>> {
        let mut result = Vec::new();
        let mut fret_last: HashMap<i32, f32> = HashMap::new();
        for note_block in &instrument.notes {
            if note_block.is_chord() {
                result.extend(self.chord(note_block, instrument.config));
                self.last_chord = Some(note_block.clone());
            } else {
                result.extend(self.single_note(note_block, instrument.config));
                self.last_chord = None;
            }
            for note in note_block.notes.iter().rev() {
                if note.fret_num == 0 || note.fret_num > 30 {
                    continue;
                }
                let entry = fret_last.entry(note.fret_num).or_insert(-10.0);
                if (note_block.time - *entry).abs() > 0.6 {
                    *entry = note_block.time;
                    let z_pos = DisplayConst::calc_in_fret_pos_z(note.fret_num);
                    result.push(MeshGenerator::text_vertical(
                        &note.fret_num.to_string(),
                        Vector3::new(
                            note_block.time * instrument.config.note_speed,
                            DisplayConst::calc_note_height_y(note.string_num) + 1.0,
                            z_pos,
                        ),
                    ).upcast());
                }
            }
        }
        result.extend(generate_note_block_frets(instrument));
        result
    }

    fn chord(&self, note_block: &NoteBlock, config: InstrumentConfig) -> Vec<Gd<Node3D>> {
        let mut list = Vec::new();
        let line_start_z = DisplayConst::calc_fret_pos_z(note_block.fret_window_start - 1);
        let across = Vector3::new(0.0, 0.0, DisplayConst::calc_fret_width_z(note_block.fret_window_start, note_block.fret_window_length));
        let bottom_left = Vector3::new(note_block.time * config.note_speed, DisplayConst::TRACK_BOTTOM_WORLD + 0.01, line_start_z);
        let chord_dir_up = Vector3::new(0.0, 1.0, 0.0) * 6.0 * DisplayConst::STRING_DISTANCE_APART;
        if note_block.chord_flags.contains(&NoteBlockFlags::Mute) {
            list.push(MeshGenerator::box_line(Color::from_rgb(0.83, 0.83, 0.83), bottom_left, bottom_left + across + chord_dir_up * 0.5).upcast());
            list.push(MeshGenerator::box_line(Color::from_rgb(0.83, 0.83, 0.83), bottom_left + across, bottom_left + chord_dir_up * 0.5).upcast());
        } else if !note_block.is_same_chord_as(self.last_chord.as_ref()) {
            for note in &note_block.notes {
                list.extend(NoteGenerator::get_note(note, config, note_block));
                list.extend(NoteGenerator::create_note_line(note_block, note, config));
            }
            if let Some(label) = &note_block.label {
                list.push(MeshGenerator::text_vertical(label, Vector3::new(
                    note_block.time * config.note_speed,
                    7.0 * DisplayConst::STRING_DISTANCE_APART,
                    DisplayConst::calc_in_fret_pos_z(note_block.fret_window_start),
                )).upcast());
            }
        }
        if !note_block.notes.iter().all(|n| n.types.contains(&NoteType::Child)) {
            list.push(MeshGenerator::box_line(Color::from_rgb(0.83, 0.83, 0.83), bottom_left + chord_dir_up, bottom_left).upcast());
            list.push(MeshGenerator::box_line(Color::from_rgb(0.83, 0.83, 0.83), bottom_left, bottom_left + across).upcast());
            list.push(MeshGenerator::box_line(Color::from_rgb(0.83, 0.83, 0.83), bottom_left + across, bottom_left + across + chord_dir_up).upcast());
        }
        list
    }

    fn single_note(&self, note_block: &NoteBlock, config: InstrumentConfig) -> Vec<Gd<Node3D>> {
        let mut list = Vec::new();
        if note_block.notes.len() != 1 {
            return list;
        }
        let note = &note_block.notes[0];
        list.extend(NoteGenerator::get_note(note, config, note_block));
        list.extend(NoteGenerator::create_note_line(note_block, note, config));
        if note.fret_num != 0 {
            let note_pos = Vector3::new(
                note_block.time * config.note_speed,
                DisplayConst::calc_note_height_y(note.string_num),
                DisplayConst::calc_in_fret_pos_z(note.fret_num),
            );
            let dir = Vector3::new(0.0, note_pos.y - DisplayConst::TRACK_BOTTOM_WORLD, 0.0);
            let start = Vector3::new(note_pos.x, DisplayConst::TRACK_BOTTOM_WORLD, note_pos.z);
            list.push(MeshGenerator::box_line(SettingsService::get_color_from_string_num(note.string_num as usize), start, start + dir).upcast());
            let pos = Vector3::new(
                note_pos.x,
                DisplayConst::TRACK_BOTTOM_WORLD + 0.01,
                DisplayConst::calc_fret_pos_z(note.fret_num - 1),
            );
            let across = Vector3::new(0.0, 0.0, DisplayConst::calc_fret_width_z(note.fret_num, 1));
            list.push(MeshGenerator::box_line(SettingsService::get_color_from_string_num(note.string_num as usize), pos, pos + across).upcast());
        }
        list
    }
}

fn generate_note_block_frets(instrument: &Instrument) -> Vec<Gd<Node3D>> {
    let mut output = Vec::new();
    let mut start_section = -10.0;
    let mut cur_start = -1;
    let mut cur_length = -1;
    for note in &instrument.notes {
        if cur_start != note.fret_window_start || cur_length != note.fret_window_length {
            if cur_start > 0 {
                while start_section < note.time {
                    let length = (note.time - start_section).min(10.0);
                    output.push(create_window_piece(cur_start, cur_length, start_section + length, start_section, instrument.config));
                    start_section += length;
                }
            }
            cur_start = note.fret_window_start;
            cur_length = note.fret_window_length;
            start_section = note.time;
        }
    }
    if cur_start > 0 {
        if let Some(last_note) = instrument.notes.last() {
            while start_section < last_note.time + 1.0 {
                let length = (last_note.time + 1.0 - start_section).min(10.0);
                output.push(create_window_piece(cur_start, cur_length, start_section + length, start_section, instrument.config));
                start_section += length;
            }
        }
    }
    output
}

fn create_window_piece(fret: i32, length: i32, end_time: f32, start_time: f32, config: InstrumentConfig) -> Gd<Node3D> {
    let across = DisplayConst::calc_fret_width_z(fret, length);
    let pos = Vector3::new(
        (end_time + start_time) / 2.0 * config.note_speed - 0.5,
        DisplayConst::TRACK_BOTTOM_WORLD - 0.01,
        DisplayConst::calc_fret_pos_z(fret - 1) + across / 2.0,
    );
    MeshGenerator::plane(Color::from_rgb(0.28, 0.24, 0.55), pos, Vector2::new(config.note_speed * (end_time - start_time), across))
}
