extends SceneTree

func _initialize() -> void:
	var scene := load("res://MainScene.tscn")
	assert(scene != null)
	var main := scene.instantiate()
	get_root().add_child(main)
	await process_frame
	assert(main.is_inside_tree())
	quit()
