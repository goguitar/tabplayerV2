use std::ffi::{c_char, c_int, c_void};

#[repr(C)]
pub struct LibStreamFile {
    _private: [u8; 0],
}

#[repr(C)]
pub struct LibVgmstreamFormat {
    pub channels: c_int,
    pub sample_rate: c_int,
    pub sample_format: c_int,
    pub sample_size: c_int,
}

#[repr(C)]
pub struct LibVgmstreamDecoder {
    pub buf: *mut c_void,
    pub buf_samples: c_int,
    pub buf_bytes: c_int,
    pub done: bool,
}

#[repr(C)]
pub struct LibVgmstream {
    pub priv_data: *mut c_void,
    pub format: *const LibVgmstreamFormat,
    pub decoder: *mut LibVgmstreamDecoder,
}

pub type LibstreamfileOpenFromStdio = unsafe extern "C" fn(filename: *const c_char) -> *mut LibStreamFile;
pub type LibstreamfileClose = unsafe extern "C" fn(libsf: *mut LibStreamFile);
pub type LibvgmstreamCreate = unsafe extern "C" fn(
    libsf: *mut LibStreamFile,
    subsong: c_int,
    cfg: *mut c_void,
) -> *mut LibVgmstream;
pub type LibvgmstreamFree = unsafe extern "C" fn(lib: *mut LibVgmstream);
pub type LibvgmstreamFill = unsafe extern "C" fn(
    lib: *mut LibVgmstream,
    buf: *mut c_void,
    buf_samples: c_int,
) -> c_int;

unsafe extern "C" {
    pub fn libstreamfile_open_from_stdio(filename: *const c_char) -> *mut LibStreamFile;
    pub fn libstreamfile_close(libsf: *mut LibStreamFile);
    pub fn libvgmstream_create(
        libsf: *mut LibStreamFile,
        subsong: c_int,
        cfg: *mut c_void,
    ) -> *mut LibVgmstream;
    pub fn libvgmstream_free(lib: *mut LibVgmstream);
    pub fn libvgmstream_fill(lib: *mut LibVgmstream, buf: *mut c_void, buf_samples: c_int) -> c_int;
}
