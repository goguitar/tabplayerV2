extends SceneTree

const DLC_DIR := "/home/csantz/Music/DLC"

func _init() -> void:
	var failures: Array[String] = []
	var ext = load("res://tabplayer_rust.gdextension")
	if ext == null:
		push_error("Failed to load tabplayer_rust.gdextension")
		quit(1)
		return

	if not ClassDB.class_exists("TabPlayerBackend"):
		push_error("TabPlayerBackend class not registered")
		quit(1)
		return

	var backend = ClassDB.instantiate("TabPlayerBackend")
	if backend == null:
		push_error("Failed to instantiate TabPlayerBackend")
		quit(1)
		return

	print("[test] Using DLC path: %s" % DLC_DIR)

	var conversion_test = load("res://tests/test_conversion.gd").new()
	var conversion_result: Dictionary = conversion_test.run(backend, DLC_DIR)
	print("[test] Conversion: %s" % str(conversion_result))
	if not bool(conversion_result.get("ok", false)):
		failures.append("conversion failed: %s" % str(conversion_result.get("message", "unknown")))

	var load_test = load("res://tests/test_music_load.gd").new()
	var load_result: Dictionary = load_test.run(backend)
	print("[test] Music load: %s" % str(load_result))
	if not bool(load_result.get("ok", false)):
		failures.append("music load failed: %s" % str(load_result.get("message", "unknown")))

	var art_test = load("res://tests/test_album_art.gd").new()
	var art_result: Dictionary = art_test.run(backend)
	print("[test] Album art: %s" % str(art_result))
	if not bool(art_result.get("ok", false)):
		failures.append("album art failed: %s" % str(art_result.get("message", "unknown")))

	if failures.is_empty():
		print("[test] All checks passed")
		quit(0)
		return

	for failure in failures:
		push_error("[test] %s" % failure)
	quit(1)
