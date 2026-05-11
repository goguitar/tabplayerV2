use godot::classes::ProjectSettings;
use godot::prelude::*;
use libloading::{Library, Symbol};
use std::ffi::{c_char, c_int, c_longlong, c_void, CString};
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug, thiserror::Error)]
pub enum VgmstreamError {
    #[error("vgmstream library not found at {0}")]
    LibraryNotFound(String),
    #[error("vgmstream init failed")]
    InitFailed,
    #[error("vgmstream open failed")]
    OpenFailed,
    #[error("vgmstream render failed")]
    RenderFailed,
}

pub struct DecodedAudio {
    pub data: Vec<u8>,
    pub sample_rate: i32,
    pub channels: i32,
}

pub struct Vgmstream {
    _lib: Library,
    init: Symbol<'static, unsafe extern "C" fn() -> *mut LibVgmstream>,
    free: Symbol<'static, unsafe extern "C" fn(*mut LibVgmstream)>,
    setup: Symbol<'static, unsafe extern "C" fn(*mut LibVgmstream, *mut LibVgmstreamConfig)>,
    open_stream:
        Symbol<'static, unsafe extern "C" fn(*mut LibVgmstream, *mut LibStreamFile, c_int) -> c_int>,
    close_stream: Symbol<'static, unsafe extern "C" fn(*mut LibVgmstream)>,
    render: Symbol<'static, unsafe extern "C" fn(*mut LibVgmstream) -> c_int>,
    streamfile_close: Symbol<'static, unsafe extern "C" fn(*mut LibStreamFile)>,
}

impl Vgmstream {
    pub fn load_default() -> Result<Self, VgmstreamError> {
        let path = resolve_library_path().ok_or_else(|| {
            VgmstreamError::LibraryNotFound("third_party/vgmstream".to_string())
        })?;
        Self::load_from_path(&path)
    }

    pub fn load_from_path(path: &Path) -> Result<Self, VgmstreamError> {
        let lib = unsafe { Library::new(path) }
            .map_err(|_| VgmstreamError::LibraryNotFound(path.display().to_string()))?;

        unsafe {
            let init: Symbol<unsafe extern "C" fn() -> *mut LibVgmstream> = lib
                .get(b"libvgmstream_init\0")
                .map_err(|_| VgmstreamError::InitFailed)?;
            let free: Symbol<unsafe extern "C" fn(*mut LibVgmstream)> =
                lib.get(b"libvgmstream_free\0")
                    .map_err(|_| VgmstreamError::InitFailed)?;
            let setup: Symbol<
                unsafe extern "C" fn(*mut LibVgmstream, *mut LibVgmstreamConfig),
            > = lib
                .get(b"libvgmstream_setup\0")
                .map_err(|_| VgmstreamError::InitFailed)?;
            let open_stream: Symbol<
                unsafe extern "C" fn(*mut LibVgmstream, *mut LibStreamFile, c_int) -> c_int,
            > = lib
                .get(b"libvgmstream_open_stream\0")
                .map_err(|_| VgmstreamError::InitFailed)?;
            let close_stream: Symbol<unsafe extern "C" fn(*mut LibVgmstream)> = lib
                .get(b"libvgmstream_close_stream\0")
                .map_err(|_| VgmstreamError::InitFailed)?;
            let render: Symbol<unsafe extern "C" fn(*mut LibVgmstream) -> c_int> = lib
                .get(b"libvgmstream_render\0")
                .map_err(|_| VgmstreamError::InitFailed)?;
            let streamfile_close: Symbol<unsafe extern "C" fn(*mut LibStreamFile)> = lib
                .get(b"libstreamfile_close\0")
                .map_err(|_| VgmstreamError::InitFailed)?;

            Ok(Self {
                _lib: lib,
                init: std::mem::transmute(init),
                free: std::mem::transmute(free),
                setup: std::mem::transmute(setup),
                open_stream: std::mem::transmute(open_stream),
                close_stream: std::mem::transmute(close_stream),
                render: std::mem::transmute(render),
                streamfile_close: std::mem::transmute(streamfile_close),
            })
        }
    }

    pub fn decode_wem(&self, data: &[u8]) -> Result<DecodedAudio, VgmstreamError> {
        let bytes = Arc::new(data.to_vec());
        let libsf = create_streamfile(bytes, "input.wem");

        let lib = unsafe { (self.init)() };
        if lib.is_null() {
            unsafe { (self.streamfile_close)(libsf) };
            return Err(VgmstreamError::InitFailed);
        }

        let mut cfg = LibVgmstreamConfig::default();
        cfg.force_sfmt = LibVgmstreamSampleFormat::Pcm16;
        unsafe {
            (self.setup)(lib, &mut cfg);
        }

        let open_result = unsafe { (self.open_stream)(lib, libsf, 0) };
        unsafe { (self.streamfile_close)(libsf) };
        if open_result < 0 {
            unsafe { (self.free)(lib) };
            return Err(VgmstreamError::OpenFailed);
        }

        let format = unsafe { (*lib).format.as_ref() }.ok_or(VgmstreamError::OpenFailed)?;
        let channels = format.channels;
        let sample_rate = format.sample_rate;

        let mut pcm = Vec::new();
        loop {
            let result = unsafe { (self.render)(lib) };
            if result < 0 {
                unsafe { (self.close_stream)(lib) };
                unsafe { (self.free)(lib) };
                return Err(VgmstreamError::RenderFailed);
            }
            let decoder = unsafe { (*lib).decoder.as_ref() }.ok_or(VgmstreamError::RenderFailed)?;
            if decoder.buf_bytes > 0 && !decoder.buf.is_null() {
                let slice = unsafe {
                    std::slice::from_raw_parts(decoder.buf as *const u8, decoder.buf_bytes as usize)
                };
                pcm.extend_from_slice(slice);
            }
            if decoder.done {
                break;
            }
        }

        unsafe {
            (self.close_stream)(lib);
            (self.free)(lib);
        }

        Ok(DecodedAudio {
            data: pcm,
            sample_rate,
            channels,
        })
    }
}

fn resolve_library_path() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("VGMSTREAM_LIB_PATH") {
        return Some(PathBuf::from(path));
    }

    let (folder, filename) = if cfg!(target_os = "windows") {
        ("windows", "libvgmstream.dll")
    } else if cfg!(target_os = "macos") {
        ("macos", "libvgmstream.dylib")
    } else {
        ("linux", "libvgmstream.so")
    };

    let path = format!("res://third_party/vgmstream/{folder}/{filename}");
    let ps = ProjectSettings::singleton();
    let abs = ps.globalize_path(path.into());
    let abs = abs.to_string();
    let path_buf = PathBuf::from(abs);
    if path_buf.exists() {
        Some(path_buf)
    } else {
        None
    }
}

#[repr(C)]
struct LibVgmstream {
    _priv: *mut c_void,
    format: *const LibVgmstreamFormat,
    decoder: *mut LibVgmstreamDecoder,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct LibVgmstreamFormat {
    channels: c_int,
    sample_rate: c_int,
    sample_format: LibVgmstreamSampleFormat,
    sample_size: c_int,
    channel_layout: u32,
    subsong_index: c_int,
    subsong_count: c_int,
    input_channels: c_int,
    stream_samples: c_longlong,
    loop_start: c_longlong,
    loop_end: c_longlong,
    loop_flag: bool,
    play_forever: bool,
    play_samples: c_longlong,
    stream_bitrate: c_int,
    codec_name: [c_char; 128],
    layout_name: [c_char; 128],
    meta_name: [c_char; 128],
    stream_name: [c_char; 256],
    format_id: c_int,
}

#[repr(C)]
struct LibVgmstreamDecoder {
    buf: *mut c_void,
    buf_samples: c_int,
    buf_bytes: c_int,
    done: bool,
}

#[repr(C)]
#[derive(Clone, Copy)]
enum LibVgmstreamSampleFormat {
    Pcm16 = 1,
    Pcm24 = 2,
    Pcm32 = 3,
    Float = 4,
}

#[repr(C)]
struct LibVgmstreamConfig {
    disable_config_override: bool,
    allow_play_forever: bool,
    play_forever: bool,
    ignore_loop: bool,
    force_loop: bool,
    really_force_loop: bool,
    ignore_fade: bool,
    loop_count: f64,
    fade_time: f64,
    fade_delay: f64,
    stereo_track: c_int,
    auto_downmix_channels: c_int,
    force_sfmt: LibVgmstreamSampleFormat,
}

impl Default for LibVgmstreamConfig {
    fn default() -> Self {
        Self {
            disable_config_override: false,
            allow_play_forever: false,
            play_forever: false,
            ignore_loop: false,
            force_loop: false,
            really_force_loop: false,
            ignore_fade: false,
            loop_count: 1.0,
            fade_time: 0.0,
            fade_delay: 0.0,
            stereo_track: 0,
            auto_downmix_channels: 0,
            force_sfmt: LibVgmstreamSampleFormat::Pcm16,
        }
    }
}

#[repr(C)]
struct LibStreamFile {
    user_data: *mut c_void,
    read: Option<extern "C" fn(*mut c_void, *mut u8, i64, c_int) -> c_int>,
    get_size: Option<extern "C" fn(*mut c_void) -> i64>,
    get_name: Option<extern "C" fn(*mut c_void) -> *const c_char>,
    open: Option<extern "C" fn(*mut c_void, *const c_char) -> *mut LibStreamFile>,
    close: Option<extern "C" fn(*mut LibStreamFile)>,
}

struct StreamFileData {
    bytes: Arc<Vec<u8>>,
    name: CString,
}

fn create_streamfile(bytes: Arc<Vec<u8>>, name: &str) -> *mut LibStreamFile {
    let data = Box::new(StreamFileData {
        bytes,
        name: CString::new(name).unwrap_or_else(|_| CString::new("input.wem").unwrap()),
    });
    let libsf = Box::new(LibStreamFile {
        user_data: Box::into_raw(data) as *mut c_void,
        read: Some(streamfile_read),
        get_size: Some(streamfile_size),
        get_name: Some(streamfile_name),
        open: Some(streamfile_open),
        close: Some(streamfile_close),
    });
    Box::into_raw(libsf)
}

extern "C" fn streamfile_read(user_data: *mut c_void, dst: *mut u8, offset: i64, length: c_int) -> c_int {
    if user_data.is_null() || dst.is_null() || length <= 0 {
        return 0;
    }
    let data = unsafe { &*(user_data as *const StreamFileData) };
    let offset = offset.max(0) as usize;
    if offset >= data.bytes.len() {
        return 0;
    }
    let available = data.bytes.len() - offset;
    let to_copy = available.min(length as usize);
    unsafe {
        std::ptr::copy_nonoverlapping(data.bytes[offset..].as_ptr(), dst, to_copy);
    }
    to_copy as c_int
}

extern "C" fn streamfile_size(user_data: *mut c_void) -> i64 {
    if user_data.is_null() {
        return 0;
    }
    let data = unsafe { &*(user_data as *const StreamFileData) };
    data.bytes.len() as i64
}

extern "C" fn streamfile_name(user_data: *mut c_void) -> *const c_char {
    if user_data.is_null() {
        return std::ptr::null();
    }
    let data = unsafe { &*(user_data as *const StreamFileData) };
    data.name.as_ptr()
}

extern "C" fn streamfile_open(user_data: *mut c_void, filename: *const c_char) -> *mut LibStreamFile {
    if user_data.is_null() {
        return std::ptr::null_mut();
    }
    let data = unsafe { &*(user_data as *const StreamFileData) };
    let name = if filename.is_null() {
        data.name.clone()
    } else {
        let cstr = unsafe { std::ffi::CStr::from_ptr(filename) };
        CString::new(cstr.to_bytes()).unwrap_or_else(|_| CString::new("input.wem").unwrap())
    };
    let libsf = create_streamfile(Arc::clone(&data.bytes), name.to_str().unwrap_or("input.wem"));
    libsf
}

extern "C" fn streamfile_close(libsf: *mut LibStreamFile) {
    if libsf.is_null() {
        return;
    }
    unsafe {
        let libsf_box = Box::from_raw(libsf);
        if !libsf_box.user_data.is_null() {
            let _ = Box::from_raw(libsf_box.user_data as *mut StreamFileData);
        }
    }
}
