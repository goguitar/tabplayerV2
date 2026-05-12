use super::shared::*;

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
