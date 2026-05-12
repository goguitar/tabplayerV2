extends RefCounted

func run(backend: Object) -> Dictionary:
	var result: Dictionary = {
		"ok": false,
		"message": "",
	}

	var folders: PackedStringArray = backend.list_song_folders()
	if folders.is_empty():
		result["message"] = "no songs available to load"
		return result

	var folder: String = str(folders[0])
	var summary: Dictionary = backend.load_song_summary(folder)
	if not bool(summary.get("ok", false)):
		result["message"] = "load_song_summary failed for folder '%s'" % folder
		return result

	if int(summary.get("instrument_count", 0)) <= 0:
		result["message"] = "song has no instruments"
		return result

	if int(summary.get("note_count", 0)) <= 0:
		result["message"] = "song has no notes"
		return result

	result["ok"] = true
	result["message"] = "loaded '%s' by '%s'" % [str(summary.get("name", "")), str(summary.get("artist", ""))]
	return result
