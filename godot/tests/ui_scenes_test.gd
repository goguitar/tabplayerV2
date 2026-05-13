extends SceneTree

func _initialize() -> void:
	await _assert_scene("res://scenes/StartMenu.tscn", {
		"VBoxContainer/PlayButton": Button,
		"VBoxContainer/ConvertButton": Button,
		"VBoxContainer/HBoxContainer/ReloadButton": Button,
		"VBoxContainer/HBoxContainer/SongCountLabel": Label,
		"VBoxContainer/InfoButton": Button,
		"VBoxContainer/SettingsButton": Button,
		"VBoxContainer/QuitButton": Button,
	})
	await _assert_scene("res://scenes/ConvertMenu.tscn", {
		"FileDialog": FileDialog,
		"ChoseButton": Button,
		"FromDownloadsButton": Button,
		"RecreateRadio": CheckButton,
		"CopySourceRadio": CheckButton,
	})
	await _assert_scene("res://scenes/SongList.tscn", {
		"HBoxContainer/RandomButton": Button,
		"HBoxContainer/FilterLineEdit": LineEdit,
		"HBoxContainer/TuningOptionButton": OptionButton,
		"HBoxContainer/CapoCheckBox": CheckBox,
		"HBoxContainer/SongsLoadedLabel": Label,
		"HSplitContainer/ScrollContainer/GridContainer": GridContainer,
	})
	await _assert_scene("res://scenes/SongPick.tscn", {
		"MarginContainer/VBoxContainer/HBoxContainer/BackButton": Button,
		"MarginContainer/VBoxContainer/HBoxContainer/Title": Label,
		"TuningConfirmationDialog": ConfirmationDialog,
	})
	await _assert_scene("res://scenes/InfoPage.tscn", {
		"ProjectSourceButton": Button,
		"OpenConfigFolderButton": Button,
		"BackButton": Button,
	})
	await _assert_scene("res://scenes/SongDisplay.tscn", {
		"AlbumArtTextureRect": TextureRect,
		"ArtistLabel": Label,
		"SongNameLabel": Label,
		"InstrumentGridContainer": GridContainer,
	})
	await _assert_scene("res://scenes/SongScene.tscn", {
		"AudioStreamPlayer": AudioStreamPlayer,
		"DetailsVBoxContainer/SongInfoLabel": Label,
		"GridContainer/PauseButton": Button,
	})
	await _assert_scene("res://scenes/SettingsPage.tscn", {})
	quit()

func _assert_scene(path: String, nodes: Dictionary) -> void:
	var instance := await _instantiate_scene(path)
	for node_path in nodes:
		var node := instance.get_node_or_null(node_path)
		assert(node != null)
		var expected_type = nodes[node_path]
		if expected_type != null:
			assert(node is expected_type)
	instance.queue_free()
	await process_frame

func _instantiate_scene(path: String) -> Node:
	var scene := load(path)
	assert(scene != null)
	var instance := scene.instantiate()
	assert(instance != null)
	get_root().add_child(instance)
	await process_frame
	return instance
