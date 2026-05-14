use crate::common::*;

#[derive(GodotClass)]
#[class(base=Node)]
struct NoteBucketGraph {
    #[base]
    _base: Base<Node>,
}

#[godot_api]
impl INode for NoteBucketGraph {
    fn init(base: Base<Node>) -> Self {
        Self { _base: base }
    }
}
