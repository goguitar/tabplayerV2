use crate::common::*;

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
        let _ = ensure_song_catalog_loaded();
        let song_count = catalog_list_song_files().len();
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
        let _ = catalog_rescan_default_dlc();
        let song_count = catalog_list_song_files().len();
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
