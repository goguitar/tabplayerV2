use std::ffi::CString;
use std::path::Path;

use anyhow::{anyhow, Context, Result};

use crate::vgm_stream_bindings::{
    libstreamfile_close, libstreamfile_open_from_stdio, libvgmstream_create, libvgmstream_fill,
    libvgmstream_free, LibStreamFile, LibVgmstream,
};

const LIBVGMSTREAM_SFMT_PCM16: i32 = 1;
const LIBVGMSTREAM_SFMT_FLOAT: i32 = 4;

pub struct DecodedAudio {
    pub sample_rate: u32,
    pub channels: u16,
    pub interleaved_i16: Vec<i16>,
}

pub fn decode_wem_to_pcm(path: &Path) -> Result<DecodedAudio> {
    let c_path = CString::new(path.to_string_lossy().as_bytes())
        .with_context(|| format!("invalid wem path '{}'", path.display()))?;

    let sf = unsafe { libstreamfile_open_from_stdio(c_path.as_ptr()) };
    if sf.is_null() {
        return Err(anyhow!("libvgmstream could not open streamfile '{}'", path.display()));
    }

    let mut sf_guard = StreamFileGuard { sf: Some(sf) };

    let lib = unsafe { libvgmstream_create(sf, 0, std::ptr::null_mut()) };
    if lib.is_null() {
        return Err(anyhow!("libvgmstream could not decode '{}'", path.display()));
    }

    sf_guard.close_now();

    let mut lib_guard = VgmstreamGuard { lib: Some(lib) };

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

        let status = unsafe { libvgmstream_fill(lib, buf_ptr, 4096) };
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

pub fn pcm_to_wav_bytes(audio: &DecodedAudio) -> Result<Vec<u8>> {
    if audio.channels == 0 || audio.sample_rate == 0 {
        return Err(anyhow!("invalid pcm metadata"));
    }

    let channels = audio.channels as u32;
    let sample_rate = audio.sample_rate;
    let bytes_per_sample = 2_u32;
    let block_align = (channels * bytes_per_sample) as u16;
    let byte_rate = sample_rate
        .checked_mul(channels)
        .and_then(|v| v.checked_mul(bytes_per_sample))
        .ok_or_else(|| anyhow!("wav byte rate overflow"))?;

    let mut data_bytes = Vec::with_capacity(audio.interleaved_i16.len() * 2);
    for sample in &audio.interleaved_i16 {
        data_bytes.extend_from_slice(&sample.to_le_bytes());
    }

    let data_len = u32::try_from(data_bytes.len()).map_err(|_| anyhow!("wav payload too large"))?;
    let riff_len = 36_u32
        .checked_add(data_len)
        .ok_or_else(|| anyhow!("wav riff size overflow"))?;

    let mut out = Vec::with_capacity((44_u32 + data_len) as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&riff_len.to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16_u32.to_le_bytes());
    out.extend_from_slice(&1_u16.to_le_bytes());
    out.extend_from_slice(&(audio.channels).to_le_bytes());
    out.extend_from_slice(&sample_rate.to_le_bytes());
    out.extend_from_slice(&byte_rate.to_le_bytes());
    out.extend_from_slice(&block_align.to_le_bytes());
    out.extend_from_slice(&16_u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    out.extend_from_slice(&data_bytes);
    Ok(out)
}

pub fn decode_wem_to_wav(path: &Path) -> Result<Vec<u8>> {
    let decoded = decode_wem_to_pcm(path)
        .with_context(|| format!("failed decoding wem '{}'", path.display()))?;
    pcm_to_wav_bytes(&decoded)
}

struct StreamFileGuard {
    sf: Option<*mut LibStreamFile>,
}

impl StreamFileGuard {
    fn close_now(&mut self) {
        if let Some(sf) = self.sf.take() {
            unsafe { libstreamfile_close(sf) };
        }
    }
}

impl Drop for StreamFileGuard {
    fn drop(&mut self) {
        self.close_now();
    }
}

struct VgmstreamGuard {
    lib: Option<*mut LibVgmstream>,
}

impl VgmstreamGuard {
    fn close_now(&mut self) {
        if let Some(lib) = self.lib.take() {
            unsafe { libvgmstream_free(lib) };
        }
    }
}

impl Drop for VgmstreamGuard {
    fn drop(&mut self) {
        self.close_now();
    }
}
