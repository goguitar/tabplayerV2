use crate::common::*;

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
