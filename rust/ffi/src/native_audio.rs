use std::ffi::CString;
use std::fs;
use std::io::Write;
use std::num::{NonZeroU32, NonZeroU8};
use std::path::Path;
use std::sync::OnceLock;

use anyhow::{anyhow, Context, Result};
use libloading::Library;
use vorbis_rs::VorbisEncoderBuilder;

use crate::vgm_stream_bindings::{
    LibStreamFile, LibVgmstream, LibstreamfileClose, LibstreamfileOpenFromStdio, LibvgmstreamCreate,
    LibvgmstreamFill, LibvgmstreamFree,
};

const LIBVGMSTREAM_SFMT_PCM16: i32 = 1;
const LIBVGMSTREAM_SFMT_FLOAT: i32 = 4;

pub struct DecodedAudio {
    pub sample_rate: u32,
    pub channels: u16,
    pub interleaved_i16: Vec<i16>,
}

struct VgmstreamApi {
    _library: Library,
    open_stdio: LibstreamfileOpenFromStdio,
    close_sf: LibstreamfileClose,
    create: LibvgmstreamCreate,
    free: LibvgmstreamFree,
    fill: LibvgmstreamFill,
}

static API: OnceLock<Result<VgmstreamApi, String>> = OnceLock::new();

fn candidate_library_names() -> Vec<String> {
    let mut out = Vec::new();
    if let Ok(path) = std::env::var("VGMSTREAM_LIB_PATH") {
        if !path.trim().is_empty() {
            out.push(path);
        }
    }

    out.push("libvgmstream.so".to_string());
    out.push("vgmstream.dll".to_string());
    out.push("libvgmstream.dll".to_string());
    out.push("libvgmstream.dylib".to_string());
    out
}

fn api() -> Result<&'static VgmstreamApi> {
    let result = API.get_or_init(|| {
        for candidate in candidate_library_names() {
            let lib = unsafe { Library::new(&candidate) };
            let Ok(lib) = lib else {
                continue;
            };

            let open_stdio = unsafe { lib.get::<LibstreamfileOpenFromStdio>(b"libstreamfile_open_from_stdio\0") };
            let close_sf = unsafe { lib.get::<LibstreamfileClose>(b"libstreamfile_close\0") };
            let create = unsafe { lib.get::<LibvgmstreamCreate>(b"libvgmstream_create\0") };
            let free = unsafe { lib.get::<LibvgmstreamFree>(b"libvgmstream_free\0") };
            let fill = unsafe { lib.get::<LibvgmstreamFill>(b"libvgmstream_fill\0") };

            if let (Ok(open_stdio), Ok(close_sf), Ok(create), Ok(free), Ok(fill)) =
                (open_stdio, close_sf, create, free, fill)
            {
                let api = VgmstreamApi {
                    open_stdio: *open_stdio,
                    close_sf: *close_sf,
                    create: *create,
                    free: *free,
                    fill: *fill,
                    _library: lib,
                };
                return Ok(api);
            }
        }

        Err(
            "unable to load libvgmstream shared library (set VGMSTREAM_LIB_PATH or install libvgmstream.so/.dll/.dylib)"
                .to_string(),
        )
    });

    match result {
        Ok(api) => Ok(api),
        Err(err) => Err(anyhow!(err.clone())),
    }
}

pub fn decode_wem_to_pcm(path: &Path) -> Result<DecodedAudio> {
    let api = api()?;
    let c_path = CString::new(path.to_string_lossy().as_bytes())
        .with_context(|| format!("invalid wem path '{}'", path.display()))?;

    let sf = unsafe { (api.open_stdio)(c_path.as_ptr()) };
    if sf.is_null() {
        return Err(anyhow!("libvgmstream could not open streamfile '{}'", path.display()));
    }

    let mut sf_guard = StreamFileGuard {
        api,
        sf: Some(sf),
    };

    let lib = unsafe { (api.create)(sf, 0, std::ptr::null_mut()) };
    if lib.is_null() {
        return Err(anyhow!("libvgmstream could not decode '{}'", path.display()));
    }

    sf_guard.close_now();

    let mut lib_guard = VgmstreamGuard {
        api,
        lib: Some(lib),
    };

    let format = unsafe { (*lib).format };
    if format.is_null() {
        return Err(anyhow!("libvgmstream returned null format for '{}'", path.display()));
    }

    let channels = unsafe { (*format).channels };
    let sample_rate = unsafe { (*format).sample_rate };
    let sample_format = unsafe { (*format).sample_format };
    let sample_size = unsafe { (*format).sample_size };

    if channels <= 0 || sample_rate <= 0 {
        return Err(anyhow!("invalid decoded metadata for '{}'", path.display()));
    }
    if sample_format != LIBVGMSTREAM_SFMT_PCM16 && sample_format != LIBVGMSTREAM_SFMT_FLOAT {
        return Err(anyhow!(
            "unsupported decoded sample format for '{}': format={} size={}",
            path.display(),
            sample_format,
            sample_size,
        ));
    }

    let channels_usize = channels as usize;
    let mut out = Vec::<i16>::new();
    let mut temp_i16 = vec![0_i16; channels_usize * 4096];
    let mut temp_f32 = vec![0_f32; channels_usize * 4096];

    loop {
        let buf_ptr = if sample_format == LIBVGMSTREAM_SFMT_PCM16 {
            temp_i16.as_mut_ptr().cast()
        } else {
            temp_f32.as_mut_ptr().cast()
        };

        let status = unsafe { (api.fill)(lib, buf_ptr, 4096) };
        if status < 0 {
            return Err(anyhow!("libvgmstream decode error for '{}'", path.display()));
        }

        let decoder = unsafe { (*lib).decoder };
        if decoder.is_null() {
            return Err(anyhow!("libvgmstream returned null decoder for '{}'", path.display()));
        }

        let got_samples_per_channel = unsafe { (*decoder).buf_samples }.max(0) as usize;
        let got_interleaved = got_samples_per_channel * channels_usize;
        if got_interleaved > 0 {
            if sample_format == LIBVGMSTREAM_SFMT_PCM16 {
                out.extend_from_slice(&temp_i16[..got_interleaved]);
            } else {
                for s in &temp_f32[..got_interleaved] {
                    let v = (*s * 32767.0).round().clamp(i16::MIN as f32, i16::MAX as f32) as i16;
                    out.push(v);
                }
            }
        }

        let done = unsafe { (*decoder).done };
        if done {
            break;
        }
        if got_interleaved == 0 {
            break;
        }
    }

    lib_guard.close_now();

    Ok(DecodedAudio {
        sample_rate: sample_rate as u32,
        channels: channels as u16,
        interleaved_i16: out,
    })
}

pub fn encode_ogg_48k(audio: &DecodedAudio, out_path: &Path) -> Result<()> {
    let target_rate = 48_000_u32;
    let resampled = if audio.sample_rate == target_rate {
        audio.interleaved_i16.clone()
    } else {
        linear_resample_interleaved_i16(&audio.interleaved_i16, audio.channels, audio.sample_rate, target_rate)
    };

    let channels = NonZeroU8::new(audio.channels.max(1) as u8)
        .ok_or_else(|| anyhow!("invalid channel count"))?;
    let mut encoder = VorbisEncoderBuilder::new(
        NonZeroU32::new(target_rate).unwrap(),
        channels,
        Vec::<u8>::new(),
    )?
    .build()?;

    let channels_usize = audio.channels.max(1) as usize;
    let frames = resampled.len() / channels_usize;
    let mut frame = 0_usize;
    const CHUNK: usize = 2048;

    while frame < frames {
        let take = (frames - frame).min(CHUNK);
        let mut planar = vec![vec![0.0_f32; take]; channels_usize];

        for i in 0..take {
            for ch in 0..channels_usize {
                let s = resampled[(frame + i) * channels_usize + ch];
                planar[ch][i] = (s as f32) / 32768.0;
            }
        }

        encoder.encode_audio_block(&planar)?;
        frame += take;
    }

    let encoded = encoder.finish()?;
    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = fs::File::create(out_path)
        .with_context(|| format!("failed to create '{}'", out_path.display()))?;
    file.write_all(&encoded)
        .with_context(|| format!("failed writing '{}'", out_path.display()))?;
    Ok(())
}

fn linear_resample_interleaved_i16(input: &[i16], channels: u16, src_rate: u32, dst_rate: u32) -> Vec<i16> {
    if src_rate == 0 || dst_rate == 0 || input.is_empty() {
        return input.to_vec();
    }

    let ch = channels.max(1) as usize;
    let in_frames = input.len() / ch;
    if in_frames < 2 {
        return input.to_vec();
    }

    let out_frames = ((in_frames as f64) * (dst_rate as f64) / (src_rate as f64)).round() as usize;
    let mut out = vec![0_i16; out_frames * ch];

    for i in 0..out_frames {
        let src_pos = (i as f64) * (src_rate as f64) / (dst_rate as f64);
        let idx0 = src_pos.floor() as usize;
        let idx1 = (idx0 + 1).min(in_frames - 1);
        let frac = (src_pos - (idx0 as f64)) as f32;

        for c in 0..ch {
            let s0 = input[idx0 * ch + c] as f32;
            let s1 = input[idx1 * ch + c] as f32;
            let v = s0 + (s1 - s0) * frac;
            out[i * ch + c] = v.round().clamp(i16::MIN as f32, i16::MAX as f32) as i16;
        }
    }

    out
}

struct StreamFileGuard<'a> {
    api: &'a VgmstreamApi,
    sf: Option<*mut LibStreamFile>,
}

impl StreamFileGuard<'_> {
    fn close_now(&mut self) {
        if let Some(sf) = self.sf.take() {
            unsafe { (self.api.close_sf)(sf) };
        }
    }
}

impl Drop for StreamFileGuard<'_> {
    fn drop(&mut self) {
        self.close_now();
    }
}

struct VgmstreamGuard<'a> {
    api: &'a VgmstreamApi,
    lib: Option<*mut LibVgmstream>,
}

impl VgmstreamGuard<'_> {
    fn close_now(&mut self) {
        if let Some(lib) = self.lib.take() {
            unsafe { (self.api.free)(lib) };
        }
    }
}

impl Drop for VgmstreamGuard<'_> {
    fn drop(&mut self) {
        self.close_now();
    }
}
