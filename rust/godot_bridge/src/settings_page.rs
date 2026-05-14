use crate::common::*;

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
