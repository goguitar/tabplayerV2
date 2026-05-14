use crate::common::*;

#[derive(GodotClass)]
#[class(base=Node3D)]
struct GuitarChart {
    #[base]
    _base: Base<Node3D>,
}

#[godot_api]
impl INode3D for GuitarChart {
    fn init(base: Base<Node3D>) -> Self {
        Self { _base: base }
    }
}
