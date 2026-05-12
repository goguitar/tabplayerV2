extends RefCounted

func run(backend: Object, dlc_dir: String) -> Dictionary:
	var result: Dictionary = {
		"ok": false,
		"message": "",
		"completed": 0,
		"failed": 0,
	}

	var conversion: Dictionary = backend.import_psarc_dir(dlc_dir)
	result["completed"] = int(conversion.get("completed", 0))
	result["failed"] = int(conversion.get("failed", 0))

	if not bool(conversion.get("ok", false)):
		result["message"] = "conversion returned error: %s" % str(conversion)
		return result

	if int(result["completed"]) <= 0:
		result["message"] = "dlc rescan found zero songs"
		return result

	result["ok"] = true
	result["message"] = "rescanned %d songs" % int(result["completed"])
	return result
