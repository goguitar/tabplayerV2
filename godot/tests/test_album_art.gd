extends RefCounted

func run(backend: Object) -> Dictionary:
	var result: Dictionary = {
		"ok": false,
		"message": "",
		"checked": 0,
		"exists": 0,
		"compressed_loadable": 0,
		"loadable": 0,
	}

	var folders: PackedStringArray = backend.list_song_folders()
	if folders.is_empty():
		result["message"] = "no songs available to validate album art"
		return result

	for folder in folders:
		var status: Dictionary = backend.song_art_status(str(folder))
		result["checked"] = int(result["checked"]) + 1
		if bool(status.get("exists", false)):
			result["exists"] = int(result["exists"]) + 1
		if bool(status.get("compressed_loadable", false)):
			result["compressed_loadable"] = int(result["compressed_loadable"]) + 1
		if bool(status.get("loadable", false)):
			result["loadable"] = int(result["loadable"]) + 1

	if int(result["exists"]) <= 0:
		result["message"] = "no album.dds files found"
		return result

	if int(result["loadable"]) <= 0:
		result["message"] = "album.dds exists but none loadable by Godot"
		return result

	result["ok"] = true
	result["message"] = "album art validated for %d/%d songs (compressed ok: %d)" % [int(result["loadable"]), int(result["checked"]), int(result["compressed_loadable"])]
	return result
