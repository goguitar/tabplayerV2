use crate::common::*;

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
