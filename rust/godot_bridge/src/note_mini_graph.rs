use crate::common::*;

#[derive(GodotClass)]
#[class(base=Node2D)]
struct NoteMiniGraph {
    #[base]
    _base: Base<Node2D>,
}

#[godot_api]
impl INode2D for NoteMiniGraph {
    fn init(base: Base<Node2D>) -> Self {
        Self { _base: base }
    }
}
