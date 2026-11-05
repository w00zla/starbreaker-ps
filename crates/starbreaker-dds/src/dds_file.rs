use std::path::Path;

use starbreaker_common::SpanReader;

pub use crate::alpha::{AlphaMipFormat, AlphaMipLayout};

use crate::alpha::{
    decode_r8_alpha_mip, infer_alpha_mip_format_from_payload, parse_alpha_tail_mips,
    parse_raw_alpha_tail_mips,
};
use crate::decode::{BlockFormat, decode_block_compressed};
use crate::error::DdsError;
use crate::sibling::ReadSibling;
use crate::types::{DDS_MAGIC, DdsHeader, DdsHeaderDxt10, DdsPixelFormat, DxgiFormat};

/// A parsed DDS texture with separated mip levels.
pub struct DdsFile {
    pub header: DdsHeader,
    pub dxt10_header: Option<DdsHeaderDxt10>,
    /// Mip levels, index 0 = largest (full resolution).
    pub mip_data: Vec<Vec<u8>>,
    /// Alpha channel mip levels (empty if none). Same ordering as mip_data.
    pub alpha_mip_data: Vec<Vec<u8>>,
    /// Legacy summary format declared by an alpha tail DDS header, if present.
    pub alpha_mip_format: Option<AlphaMipFormat>,
    /// Encoding for each alpha/smoothness mip when known from headers or raw-tail splitting.
    /// `None` means the payload has no header and must be inferred from mip dimensions.
    pub alpha_mip_formats: Vec<Option<AlphaMipFormat>>,
    /// Source layout for each alpha/smoothness mip, parallel to alpha_mip_data.
    pub alpha_mip_layouts: Vec<AlphaMipLayout>,
}

impl DdsFile {
    /// Parse only the DDS headers, ignoring pixel data.
    /// Works for any format, even unsupported ones.
    pub fn headers_only(data: &[u8]) -> Result<Self, DdsError> {
        let mut reader = SpanReader::new(data);
        let magic = reader.read_bytes(4)?;
        if magic != DDS_MAGIC {
            return Err(DdsError::InvalidMagic);
        }
        let header = *reader.read_type::<DdsHeader>()?;
        let dxt10_header = if header.pixel_format.four_cc == *b"DX10" {
            Some(*reader.read_type::<DdsHeaderDxt10>()?)
        } else {
            None
        };
        Ok(Self {
            header,
            dxt10_header,
            mip_data: Vec::new(),
            alpha_mip_data: Vec::new(),
            alpha_mip_format: None,
            alpha_mip_formats: Vec::new(),
            alpha_mip_layouts: Vec::new(),
        })
    }

    /// Parse a complete (already merged) DDS file from bytes.
    pub fn from_bytes(data: &[u8]) -> Result<Self, DdsError> {
        let mut reader = SpanReader::new(data);

        // Read and validate magic
        let magic = reader.read_bytes(4)?;
        if magic != DDS_MAGIC {
            return Err(DdsError::InvalidMagic);
        }

        // Read main header
        let header = *reader.read_type::<DdsHeader>()?;

        // Read optional DX10 header
        let dxt10_header = if header.pixel_format.four_cc == *b"DX10" {
            Some(*reader.read_type::<DdsHeaderDxt10>()?)
        } else {
            None
        };

        let mip_count = std::cmp::max(1, header.mipmap_count) as usize;

        // Determine number of faces (cubemaps have 6 faces)
        let faces = face_count(&header, dxt10_header.as_ref());

        // Calculate sizes for each mip level (per face)
        let mip_sizes: Vec<usize> = (0..mip_count)
            .map(|level| {
                compute_mip_size(
                    &header.pixel_format,
                    dxt10_header.as_ref(),
                    header.width,
                    header.height,
                    level,
                )
            })
            .collect();

        let remaining = reader.read_bytes(reader.remaining())?;

        let mut mip_data = Vec::with_capacity(mip_count);
        let mut offset = 0;

        if faces == 1 {
            for &size in &mip_sizes {
                if offset + size > remaining.len() {
                    break;
                }
                mip_data.push(remaining[offset..offset + size].to_vec());
                offset += size;
            }
        } else {
            // Cubemap: store per-mip with all faces concatenated.
            for &size in &mip_sizes {
                let total = size * faces;
                if offset + total > remaining.len() {
                    break;
                }
                mip_data.push(remaining[offset..offset + total].to_vec());
                offset += total;
            }
        }

        Ok(Self {
            header,
            dxt10_header,
            mip_data,
            alpha_mip_data: Vec::new(),
            alpha_mip_format: None,
            alpha_mip_formats: Vec::new(),
            alpha_mip_layouts: Vec::new(),
        })
    }

    /// Parse a split CryEngine DDS file and merge sibling chunks.
    ///
    /// `base_data` is the content of the `.dds` file (header + smallest mip data).
    /// `reader` provides access to sibling files (`.1`, `.2`, ..., `.7`, `.a`, `.1a`, etc.).
    pub fn from_split(base_data: &[u8], reader: &dyn ReadSibling) -> Result<Self, DdsError> {
        Self::from_split_min_mip(base_data, reader, 0)
    }

    /// Like [`Self::from_split`], but only reads the split files that hold
    /// `min_mip` and smaller levels.
    ///
    /// CryEngine split layout: the numbered siblings `.8` ... `.1` that exist,
    /// taken from the highest number down, hold one level each (all faces of a
    /// cubemap) from mip 0 on; the base file holds the remaining smaller levels.
    /// So with `k` numbered siblings, `.k` is mip 0 and `.1` is mip `k - 1`.
    /// Alpha siblings `.8a` ... `.1a` follow the same rule, `.a` holds the
    /// remaining alpha levels. Which siblings exist (and the size of the largest,
    /// which gives the face count) comes from [`ReadSibling::sibling_size`], so
    /// skipped files are never read.
    ///
    /// Levels below `min(min_mip, last color level, last alpha level)` are left
    /// as empty buffers: decoding `min_mip` clamped to the color and/or alpha
    /// levels present works as with [`Self::from_split`], smaller indices do not,
    /// nor does [`Self::to_dds`]. `mip_count()` and the alpha mip count are the
    /// same as with [`Self::from_split`].
    pub fn from_split_min_mip(
        base_data: &[u8],
        reader: &dyn ReadSibling,
        min_mip: usize,
    ) -> Result<Self, DdsError> {
        let mut r = SpanReader::new(base_data);

        // Read and validate magic
        let magic = r.read_bytes(4)?;
        if magic != DDS_MAGIC {
            return Err(DdsError::InvalidMagic);
        }

        // Read main header
        let header = *r.read_type::<DdsHeader>()?;

        // Read optional DX10 header
        let dxt10_header = if header.pixel_format.four_cc == *b"DX10" {
            Some(*r.read_type::<DdsHeaderDxt10>()?)
        } else {
            None
        };

        let mip_count = std::cmp::max(1, header.mipmap_count) as usize;

        // Remaining bytes in the base file are the smallest mip level(s)
        let small_mip_bytes = r.read_bytes(r.remaining())?;

        // Calculate sizes for each mip level
        let mip_sizes: Vec<usize> = (0..mip_count)
            .map(|level| {
                compute_mip_size(
                    &header.pixel_format,
                    dxt10_header.as_ref(),
                    header.width,
                    header.height,
                    level,
                )
            })
            .collect();

        // Probe for sibling split files: .8, .7, .6, ..., .1
        // These are ordered from largest mip to smallest. The highest-numbered
        // existing file corresponds to mip 0 (full resolution).
        let read_all = min_mip == 0;
        let siblings = probe_numbered_siblings(reader, "", read_all);

        // Determine face count from the largest split file
        let faces = if !siblings.is_empty() && !mip_sizes.is_empty() {
            let largest_mip_size = mip_sizes[0];
            if largest_mip_size > 0 && siblings[0].len.is_multiple_of(largest_mip_size) {
                siblings[0].len / largest_mip_size
            } else {
                1
            }
        } else {
            face_count(&header, dxt10_header.as_ref())
        };

        // Mips from the base file, after those in the sibling files
        let num_sibling_mips = siblings.len();
        let mut base_mips = Vec::with_capacity(mip_count);

        if faces <= 1 {
            // Simple case: no cubemap
            let mut offset = 0;
            for &size in mip_sizes.iter().skip(num_sibling_mips) {
                if offset + size > small_mip_bytes.len() {
                    break;
                }
                base_mips.push(small_mip_bytes[offset..offset + size].to_vec());
                offset += size;
            }
        } else {
            // Cubemap case: each sibling file contains data for ALL faces at that mip level.
            let mut offset = 0;
            for &per_face_size in mip_sizes.iter().skip(num_sibling_mips) {
                let total_size = per_face_size * faces;
                if offset + total_size > small_mip_bytes.len() {
                    // Try without face multiplier as fallback
                    if offset + per_face_size <= small_mip_bytes.len() {
                        base_mips.push(small_mip_bytes[offset..offset + per_face_size].to_vec());
                        offset += per_face_size;
                    } else {
                        break;
                    }
                } else {
                    base_mips.push(small_mip_bytes[offset..offset + total_size].to_vec());
                    offset += total_size;
                }
            }
        }

        // Probe for alpha sibling files: .8a, .7a, ..., .1a, then .a
        let alpha_siblings = probe_numbered_siblings(reader, "a", read_all);
        let mut alpha_tail: Vec<Vec<u8>> = Vec::new();
        let mut alpha_tail_formats: Vec<Option<AlphaMipFormat>> = Vec::new();
        let mut alpha_tail_layouts: Vec<AlphaMipLayout> = Vec::new();
        let mut alpha_mip_format = None;
        if let Some(data) = reader.read_sibling(".a") {
            let numbered_alpha_mips = alpha_siblings.len();
            let tail_mips = parse_alpha_tail_mips(
                &data,
                numbered_alpha_mips,
                mip_count,
                header.width,
                header.height,
            );
            if tail_mips.mips.is_empty() {
                let raw_tail_mips = parse_raw_alpha_tail_mips(
                    &data,
                    numbered_alpha_mips,
                    mip_count,
                    header.width,
                    header.height,
                );
                if raw_tail_mips.mips.is_empty() {
                    alpha_tail.push(data);
                    alpha_tail_formats.push(None);
                    alpha_tail_layouts.push(AlphaMipLayout::RawSinglePayload);
                } else {
                    alpha_mip_format = raw_tail_mips.format;
                    alpha_tail_formats.extend(std::iter::repeat_n(
                        raw_tail_mips.format,
                        raw_tail_mips.mips.len(),
                    ));
                    alpha_tail_layouts.extend(std::iter::repeat_n(
                        AlphaMipLayout::RawTailSplit,
                        raw_tail_mips.mips.len(),
                    ));
                    alpha_tail.extend(raw_tail_mips.mips);
                }
            } else {
                alpha_mip_format = tail_mips.format;
                alpha_tail_formats
                    .extend(std::iter::repeat_n(tail_mips.format, tail_mips.mips.len()));
                alpha_tail_layouts.extend(std::iter::repeat_n(
                    AlphaMipLayout::HeaderedTail,
                    tail_mips.mips.len(),
                ));
                alpha_tail.extend(tail_mips.mips);
            }
        }

        // The smallest level a caller can end up decoding: min_mip, clamped to
        // the last color level and, when there are alpha levels, the last alpha one.
        let color_levels = siblings.len() + base_mips.len();
        let alpha_levels = alpha_siblings.len() + alpha_tail.len();
        let mut first_needed = min_mip.min(color_levels.saturating_sub(1));
        if alpha_levels > 0 {
            first_needed = first_needed.min(alpha_levels - 1);
        }

        let num_alpha_siblings = alpha_siblings.len();
        let (Some(sibling_mips), Some(alpha_sibling_mips)) = (
            load_numbered_siblings(reader, siblings, first_needed),
            load_numbered_siblings(reader, alpha_siblings, first_needed),
        ) else {
            // A sibling that exists could not be read: the full read treats it as
            // missing, which shifts the levels, so take that path.
            return Self::from_split_min_mip(base_data, reader, 0);
        };

        let mut mip_data = sibling_mips;
        mip_data.extend(base_mips);

        let mut alpha_mip_data = alpha_sibling_mips;
        alpha_mip_data.extend(alpha_tail);
        let mut alpha_mip_formats = vec![None; num_alpha_siblings];
        alpha_mip_formats.extend(alpha_tail_formats);
        let mut alpha_mip_layouts = vec![AlphaMipLayout::NumberedSibling; num_alpha_siblings];
        alpha_mip_layouts.extend(alpha_tail_layouts);

        Ok(Self {
            header,
            dxt10_header,
            mip_data,
            alpha_mip_data,
            alpha_mip_format,
            alpha_mip_formats,
            alpha_mip_layouts,
        })
    }

    /// Serialize back to a complete DDS file (magic + header + optional DX10 + mip data).
    pub fn to_dds(&self) -> Vec<u8> {
        let total_mip_bytes: usize = self.mip_data.iter().map(|m| m.len()).sum();
        let header_bytes = 4 + 124 + if self.dxt10_header.is_some() { 20 } else { 0 };
        let mut out = Vec::with_capacity(header_bytes + total_mip_bytes);

        out.extend_from_slice(&DDS_MAGIC);
        out.extend_from_slice(zerocopy::IntoBytes::as_bytes(&self.header));
        if let Some(ref dx10) = self.dxt10_header {
            out.extend_from_slice(zerocopy::IntoBytes::as_bytes(dx10));
        }

        for mip in &self.mip_data {
            out.extend_from_slice(mip);
        }

        out
    }

    /// Decode a specific mip level to RGBA8 pixels.
    ///
    /// Returns a `Vec<u8>` with 4 bytes per pixel (R, G, B, A).
    pub fn decode_rgba(&self, mip_level: usize) -> Result<Vec<u8>, DdsError> {
        if mip_level >= self.mip_data.len() {
            return Err(DdsError::MipOutOfRange {
                level: mip_level,
                max: self.mip_data.len().saturating_sub(1),
            });
        }

        let pf = &self.header.pixel_format;
        let (w, h) = self.dimensions(mip_level);
        let data = &self.mip_data[mip_level];

        // Uncompressed: FourCC is zero and rgb_bit_count is set
        if pf.four_cc == [0; 4] && self.dxt10_header.is_none() && pf.rgb_bit_count > 0 {
            return decode_uncompressed(data, w, h, pf);
        }

        // DX10 header with an uncompressed DXGI format (e.g. R8_UNORM)
        if let Some(ref dx10) = self.dxt10_header {
            if let Some(rgba) = decode_dxgi_uncompressed(data, w, h, dx10.dxgi_format)? {
                return Ok(rgba);
            }
        }

        let format = resolve_format(pf, self.dxt10_header.as_ref())?;
        let is_snorm = matches!(format, DxgiFormat::BC4Snorm | DxgiFormat::BC5Snorm);
        let block_format = dxgi_to_block_format(format)?;

        decode_block_compressed(data, w, h, block_format, is_snorm)
    }

    /// Resolve the encoding used by an alpha/smoothness mip.
    ///
    /// Headered alpha tails declare this explicitly. Raw sibling payloads do not,
    /// so use the payload size when it unambiguously identifies an R8 mip and
    /// otherwise keep CryEngine's BC4 default.
    pub fn alpha_mip_format_for_mip(&self, mip_level: usize) -> Option<AlphaMipFormat> {
        if mip_level >= self.alpha_mip_data.len() {
            return None;
        }
        if mip_level < self.alpha_mip_formats.len() {
            if let Some(format) = self.alpha_mip_formats[mip_level] {
                return Some(format);
            }
        } else if let Some(format) = self.alpha_mip_format {
            return Some(format);
        }
        let (w, h) = self.dimensions(mip_level);
        infer_alpha_mip_format_from_payload(&self.alpha_mip_data[mip_level], w, h)
            .or(Some(AlphaMipFormat::Bc4Unorm))
    }

    /// Return how a specific alpha/smoothness mip was sourced from split DDS siblings.
    pub fn alpha_mip_layout_for_mip(&self, mip_level: usize) -> Option<AlphaMipLayout> {
        self.alpha_mip_layouts.get(mip_level).copied()
    }

    /// Decode an alpha sibling mip to a single-channel grayscale buffer.
    ///
    /// Alpha mips are stored in separate `.7a`, `.6a`, ..., `.a` sibling files
    /// and contain per-pixel smoothness data (EFTT_SMOOTHNESS) as BC4 or raw R8.
    /// Returns a `Vec<u8>` with 1 byte per pixel (smoothness value).
    pub fn decode_alpha_mip(&self, mip_level: usize) -> Result<Vec<u8>, DdsError> {
        if mip_level >= self.alpha_mip_data.len() {
            return Err(DdsError::MipOutOfRange {
                level: mip_level,
                max: self.alpha_mip_data.len().saturating_sub(1),
            });
        }
        let (w, h) = self.dimensions(mip_level);
        let data = &self.alpha_mip_data[mip_level];
        match self
            .alpha_mip_format_for_mip(mip_level)
            .unwrap_or(AlphaMipFormat::Bc4Unorm)
        {
            AlphaMipFormat::Bc4Unorm | AlphaMipFormat::Bc4Snorm => {
                let is_snorm =
                    self.alpha_mip_format_for_mip(mip_level) == Some(AlphaMipFormat::Bc4Snorm);
                let rgba = crate::decode::decode_block_compressed(
                    data,
                    w,
                    h,
                    crate::decode::BlockFormat::BC4,
                    is_snorm,
                )?;
                Ok(rgba.iter().step_by(4).copied().collect())
            }
            AlphaMipFormat::R8Unorm => decode_r8_alpha_mip(data, w, h),
        }
    }

    /// Decode a BC6H gobo (projector texture) to float RGB preserving HDR values.
    ///
    /// This is specifically for exporting gobos to EXR format for Blender.
    /// Unlike `decode_rgba`, this does NOT tone-map or clamp values,
    /// so floating-point values >1.0 (common in HDR gobos) are preserved.
    ///
    /// Returns `None` if the DDS is not a BC6H format.
    /// Returns `Some((width, height, rgb_floats))` where rgb_floats is in row-major order
    /// with 3 floats per pixel (R, G, B).
    pub fn decode_bc6h_to_float_rgb(
        &self,
        mip_level: usize,
    ) -> Result<Option<(u32, u32, Vec<f32>)>, DdsError> {
        if mip_level >= self.mip_data.len() {
            return Err(DdsError::MipOutOfRange {
                level: mip_level,
                max: self.mip_data.len().saturating_sub(1),
            });
        }

        let pf = &self.header.pixel_format;
        let (w, h) = self.dimensions(mip_level);
        let data = &self.mip_data[mip_level];

        // Check if this is a BC6H format (requires DX10 header)
        let format = match resolve_format(pf, self.dxt10_header.as_ref()) {
            Ok(fmt) => fmt,
            Err(_) => return Ok(None), // Not a recognized format, return None
        };

        // Only handle BC6H; all other formats return None
        if !matches!(format, DxgiFormat::BC6hUf16) {
            return Ok(None);
        }

        // Decode BC6H to float RGB
        let float_rgb = crate::decode::decode_bc6h_to_float_rgb(data, w, h)?;
        Ok(Some((w, h, float_rgb)))
    }

    /// Check if this DDS has alpha/smoothness mip data.
    pub fn has_alpha_mips(&self) -> bool {
        !self.alpha_mip_data.is_empty()
    }

    /// Save a specific mip level as a PNG file.
    pub fn save_png(&self, path: &Path, mip_level: usize) -> Result<(), DdsError> {
        let rgba = self.decode_rgba(mip_level)?;
        let (w, h) = self.dimensions(mip_level);

        let img = image::RgbaImage::from_raw(w, h, rgba)
            .ok_or_else(|| DdsError::Image("failed to create image from raw pixels".to_string()))?;

        img.save(path).map_err(|e| DdsError::Image(e.to_string()))?;

        Ok(())
    }

    /// Get the dimensions (width, height) of a specific mip level.
    pub fn dimensions(&self, mip_level: usize) -> (u32, u32) {
        let w = std::cmp::max(1, self.header.width >> mip_level);
        let h = std::cmp::max(1, self.header.height >> mip_level);
        (w, h)
    }

    /// Number of mip levels present.
    pub fn mip_count(&self) -> usize {
        self.mip_data.len()
    }

    /// Returns `true` if this is a cubemap texture.
    pub fn is_cubemap(&self) -> bool {
        // Check DX10 misc_flag bit 2 (D3D11_RESOURCE_MISC_TEXTURECUBE = 0x4)
        if let Some(ref dx10) = self.dxt10_header
            && dx10.misc_flag & 0x4 != 0
        {
            return true;
        }
        // Check cubemap_flags in base header (DDSCAPS2_CUBEMAP = 0x200)
        self.header.cubemap_flags & 0x200 != 0
    }
}

/// Calculate the byte size for a single mip level.
/// Works for both block-compressed and uncompressed formats.
fn compute_mip_size(
    pf: &DdsPixelFormat,
    dxt10: Option<&DdsHeaderDxt10>,
    width: u32,
    height: u32,
    mip_level: usize,
) -> usize {
    let w = std::cmp::max(1, width >> mip_level);
    let h = std::cmp::max(1, height >> mip_level);

    // Try block-compressed first
    if let Ok(format) = resolve_format(pf, dxt10) {
        let block_size = format.block_size();
        let blocks_w = w.div_ceil(4) as usize;
        let blocks_h = h.div_ceil(4) as usize;
        return blocks_w * blocks_h * block_size;
    }

    // DX10 uncompressed formats — bytes per pixel from DXGI format
    if let Some(dx10) = dxt10 {
        if let Some(bpp) = dxgi_uncompressed_bpp(dx10.dxgi_format) {
            return (w as usize) * (h as usize) * bpp;
        }
    }

    // Legacy uncompressed: use rgb_bit_count
    let bpp = { pf.rgb_bit_count } as usize;
    let byte_pp = if bpp > 0 { bpp / 8 } else { 4 }; // default to 32-bit
    (w as usize) * (h as usize) * byte_pp
}

/// Determine number of faces (1 for regular textures, 6 for cubemaps).
fn face_count(header: &DdsHeader, dxt10: Option<&DdsHeaderDxt10>) -> usize {
    if let Some(dx10) = dxt10
        && dx10.misc_flag & 0x4 != 0
    {
        return 6;
    }
    // DDSCAPS2_CUBEMAP = 0x200
    if header.cubemap_flags & 0x200 != 0 {
        // Count set face bits
        let mut faces = 0;
        for bit in [0x400, 0x800, 0x1000, 0x2000, 0x4000, 0x8000] {
            if header.cubemap_flags & bit != 0 {
                faces += 1;
            }
        }
        if faces == 0 { 6 } else { faces }
    } else {
        1
    }
}

/// A numbered split file (`.N` or `.Na`) that exists next to the base file.
struct SplitSibling {
    suffix: String,
    len: usize,
    /// Its bytes when already read.
    data: Option<Vec<u8>>,
}

/// The numbered siblings `.8{tag}` ... `.1{tag}` that exist, largest mip first:
/// read now when `read_now`, else only sized (read later if needed).
fn probe_numbered_siblings(reader: &dyn ReadSibling, tag: &str, read_now: bool) -> Vec<SplitSibling> {
    (1..=8)
        .rev()
        .filter_map(|n| {
            let suffix = format!(".{n}{tag}");
            if read_now {
                let data = reader.read_sibling(&suffix)?;
                Some(SplitSibling {
                    suffix,
                    len: data.len(),
                    data: Some(data),
                })
            } else {
                let len = reader.sibling_size(&suffix)?;
                Some(SplitSibling {
                    suffix,
                    len,
                    data: None,
                })
            }
        })
        .collect()
}

/// The siblings' mip data, levels below `first_needed` left empty (unread).
/// `None` when a needed sibling cannot be read or its size changed.
fn load_numbered_siblings(
    reader: &dyn ReadSibling,
    siblings: Vec<SplitSibling>,
    first_needed: usize,
) -> Option<Vec<Vec<u8>>> {
    siblings
        .into_iter()
        .enumerate()
        .map(|(level, sibling)| match sibling.data {
            Some(data) => Some(data),
            None if level < first_needed => Some(Vec::new()),
            None => reader
                .read_sibling(&sibling.suffix)
                .filter(|data| data.len() == sibling.len),
        })
        .collect()
}

/// Resolve the DXGI format from the DDS headers.
pub fn resolve_format(
    pf: &DdsPixelFormat,
    dxt10: Option<&DdsHeaderDxt10>,
) -> Result<DxgiFormat, DdsError> {
    if let Some(dx10) = dxt10 {
        let dxgi_fmt = { dx10.dxgi_format };
        DxgiFormat::from_dxgi(dxgi_fmt)
            .ok_or_else(|| DdsError::UnsupportedFormat(format!("DXGI format {dxgi_fmt}")))
    } else {
        let four_cc = { pf.four_cc };
        DxgiFormat::from_four_cc(&four_cc).ok_or_else(|| {
            let cc = String::from_utf8_lossy(&four_cc);
            let flags = { pf.flags };
            let bpp = { pf.rgb_bit_count };
            let rm = { pf.r_bit_mask };
            let gm = { pf.g_bit_mask };
            let bm = { pf.b_bit_mask };
            let am = { pf.a_bit_mask };
            DdsError::UnsupportedFormat(format!(
                "FourCC '{cc}' (flags=0x{flags:08X}, bpp={bpp}, rmask=0x{rm:08X}, gmask=0x{gm:08X}, bmask=0x{bm:08X}, amask=0x{am:08X})"
            ))
        })
    }
}

/// Map a DxgiFormat to the decode module's BlockFormat.
fn dxgi_to_block_format(format: DxgiFormat) -> Result<BlockFormat, DdsError> {
    match format {
        DxgiFormat::BC1Unorm | DxgiFormat::BC1UnormSrgb => Ok(BlockFormat::BC1),
        DxgiFormat::BC3Unorm | DxgiFormat::BC3UnormSrgb => Ok(BlockFormat::BC3),
        DxgiFormat::BC4Unorm | DxgiFormat::BC4Snorm => Ok(BlockFormat::BC4),
        DxgiFormat::BC5Unorm | DxgiFormat::BC5Snorm => Ok(BlockFormat::BC5),
        DxgiFormat::BC6hUf16 => Ok(BlockFormat::BC6H),
        DxgiFormat::BC7Unorm | DxgiFormat::BC7UnormSrgb => Ok(BlockFormat::BC7),
    }
}

/// Decode uncompressed pixel data using the DDS pixel format bit masks.
fn decode_uncompressed(
    data: &[u8],
    width: u32,
    height: u32,
    pf: &DdsPixelFormat,
) -> Result<Vec<u8>, DdsError> {
    let bpp = pf.rgb_bit_count as usize;
    let byte_pp = bpp / 8;
    if byte_pp == 0 || bpp % 8 != 0 {
        return Err(DdsError::UnsupportedFormat(format!(
            "uncompressed {bpp}-bit (non-byte-aligned)"
        )));
    }

    let pixel_count = (width as usize) * (height as usize);
    let expected = pixel_count * byte_pp;
    if data.len() < expected {
        return Err(DdsError::Decode(format!(
            "uncompressed data too short: need {expected}, have {}",
            data.len()
        )));
    }

    let r_shift = pf.r_bit_mask.trailing_zeros();
    let g_shift = pf.g_bit_mask.trailing_zeros();
    let b_shift = pf.b_bit_mask.trailing_zeros();
    let has_alpha = pf.a_bit_mask != 0;
    let a_shift = pf.a_bit_mask.trailing_zeros();

    let mut out = vec![255u8; pixel_count * 4];

    for i in 0..pixel_count {
        let off = i * byte_pp;
        let mut raw = 0u32;
        for b in 0..byte_pp {
            raw |= (data[off + b] as u32) << (b * 8);
        }

        out[i * 4] = ((raw >> r_shift) & 0xFF) as u8;
        out[i * 4 + 1] = ((raw >> g_shift) & 0xFF) as u8;
        out[i * 4 + 2] = ((raw >> b_shift) & 0xFF) as u8;
        if has_alpha {
            out[i * 4 + 3] = ((raw >> a_shift) & 0xFF) as u8;
        }
    }

    Ok(out)
}

/// Decode uncompressed pixel data identified by a DXGI format value in a DX10 header.
///
/// Returns `Ok(Some(rgba))` if the format is a known uncompressed type,
/// `Ok(None)` if the format is not uncompressed (caller should try block-compressed),
/// or `Err` if the data is invalid for the format.
fn decode_dxgi_uncompressed(
    data: &[u8],
    width: u32,
    height: u32,
    dxgi_format: u32,
) -> Result<Option<Vec<u8>>, DdsError> {
    let pixel_count = (width as usize) * (height as usize);
    match dxgi_format {
        // DXGI_FORMAT_R8_UNORM (61) — single-channel 8-bit grayscale
        61 => {
            if data.len() < pixel_count {
                return Err(DdsError::Decode(format!(
                    "R8_UNORM data too short: need {pixel_count}, have {}",
                    data.len()
                )));
            }
            let mut out = vec![255u8; pixel_count * 4];
            for i in 0..pixel_count {
                let v = data[i];
                out[i * 4] = v;
                out[i * 4 + 1] = v;
                out[i * 4 + 2] = v;
                // alpha stays 255
            }
            Ok(Some(out))
        }
        // DXGI_FORMAT_R8G8_UNORM (49) — two-channel 8-bit
        49 => {
            let expected = pixel_count * 2;
            if data.len() < expected {
                return Err(DdsError::Decode(format!(
                    "R8G8_UNORM data too short: need {expected}, have {}",
                    data.len()
                )));
            }
            let mut out = vec![255u8; pixel_count * 4];
            for i in 0..pixel_count {
                out[i * 4] = data[i * 2];
                out[i * 4 + 1] = data[i * 2 + 1];
                out[i * 4 + 2] = 0;
                // alpha stays 255
            }
            Ok(Some(out))
        }
        // DXGI_FORMAT_R8G8B8A8_UNORM (28) / R8G8B8A8_UNORM_SRGB (29)
        28 | 29 => {
            let expected = pixel_count * 4;
            if data.len() < expected {
                return Err(DdsError::Decode(format!(
                    "R8G8B8A8_UNORM data too short: need {expected}, have {}",
                    data.len()
                )));
            }
            Ok(Some(data[..expected].to_vec()))
        }
        // DXGI_FORMAT_B8G8R8A8_UNORM (87) / B8G8R8A8_UNORM_SRGB (91)
        87 | 91 => {
            let expected = pixel_count * 4;
            if data.len() < expected {
                return Err(DdsError::Decode(format!(
                    "B8G8R8A8_UNORM data too short: need {expected}, have {}",
                    data.len()
                )));
            }
            let mut out = Vec::with_capacity(expected);
            for i in 0..pixel_count {
                let off = i * 4;
                out.push(data[off + 2]); // R
                out.push(data[off + 1]); // G
                out.push(data[off]); // B
                out.push(data[off + 3]); // A
            }
            Ok(Some(out))
        }
        _ => Ok(None),
    }
}

/// Bytes per pixel for known uncompressed DXGI formats.
/// Returns `None` for block-compressed or unknown formats.
fn dxgi_uncompressed_bpp(dxgi_format: u32) -> Option<usize> {
    match dxgi_format {
        61 => Some(1),      // R8_UNORM
        49 => Some(2),      // R8G8_UNORM
        28 | 29 => Some(4), // R8G8B8A8_UNORM / SRGB
        87 | 91 => Some(4), // B8G8R8A8_UNORM / SRGB
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::HashMap;

    use super::*;

    /// Siblings in memory that record which ones were read; `broken` exists
    /// (has a size) but cannot be read.
    struct RecordingSiblings {
        files: HashMap<&'static str, Vec<u8>>,
        broken: Option<&'static str>,
        reads: RefCell<Vec<String>>,
    }

    impl RecordingSiblings {
        fn new(files: Vec<(&'static str, Vec<u8>)>) -> Self {
            Self {
                files: files.into_iter().collect(),
                broken: None,
                reads: RefCell::new(Vec::new()),
            }
        }

        fn reads(&self) -> Vec<String> {
            let mut reads = self.reads.borrow().clone();
            reads.sort();
            reads
        }
    }

    impl ReadSibling for RecordingSiblings {
        fn read_sibling(&self, suffix: &str) -> Option<Vec<u8>> {
            self.reads.borrow_mut().push(suffix.to_string());
            if self.broken == Some(suffix) {
                return None;
            }
            self.files.get(suffix).cloned()
        }

        fn sibling_size(&self, suffix: &str) -> Option<usize> {
            self.files.get(suffix).map(Vec::len)
        }
    }

    fn header(width: u32, height: u32, mipmap_count: u32, cubemap_flags: u32) -> DdsHeader {
        DdsHeader {
            size: 124,
            flags: 0x0002_1007,
            height,
            width,
            pitch_or_linear_size: 0,
            depth: 0,
            mipmap_count,
            reserved1: [0; 11],
            pixel_format: DdsPixelFormat {
                size: 32,
                flags: 0x4,
                four_cc: *b"DX10",
                rgb_bit_count: 0,
                r_bit_mask: 0,
                g_bit_mask: 0,
                b_bit_mask: 0,
                a_bit_mask: 0,
            },
            surface_flags: 0x0040_1008,
            cubemap_flags,
            reserved2: [0; 3],
        }
    }

    fn dx10(dxgi_format: u32, misc_flag: u32) -> DdsHeaderDxt10 {
        DdsHeaderDxt10 {
            dxgi_format,
            resource_dimension: 3,
            misc_flag,
            array_size: 1,
            misc_flags2: 0,
        }
    }

    fn dds(magic: bool, header: DdsHeader, dxt10: DdsHeaderDxt10, payload: &[u8]) -> Vec<u8> {
        let mut bytes = Vec::new();
        if magic {
            bytes.extend_from_slice(&DDS_MAGIC);
        }
        bytes.extend_from_slice(zerocopy::IntoBytes::as_bytes(&header));
        bytes.extend_from_slice(zerocopy::IntoBytes::as_bytes(&dxt10));
        bytes.extend_from_slice(payload);
        bytes
    }

    /// A distinct fill per file so a level read from the wrong file shows.
    fn fill(byte: u8, len: usize) -> Vec<u8> {
        vec![byte; len]
    }

    /// The levels a partial parse must hold exactly as the full parse does.
    fn assert_same_from(partial: &DdsFile, full: &DdsFile, first: usize) {
        assert_eq!(partial.mip_count(), full.mip_count());
        assert_eq!(partial.alpha_mip_data.len(), full.alpha_mip_data.len());
        assert_eq!(partial.mip_data[first..], full.mip_data[first..]);
        // Below it: unread siblings are empty, levels from the base file kept.
        assert!(unread_or_same(&partial.mip_data[..first], &full.mip_data));
        let alpha_first = first.min(full.alpha_mip_data.len());
        assert_eq!(
            partial.alpha_mip_data[alpha_first..],
            full.alpha_mip_data[alpha_first..]
        );
        assert!(unread_or_same(
            &partial.alpha_mip_data[..alpha_first],
            &full.alpha_mip_data
        ));
        assert_eq!(partial.alpha_mip_formats, full.alpha_mip_formats);
        assert_eq!(partial.alpha_mip_layouts, full.alpha_mip_layouts);
        assert_eq!(partial.alpha_mip_format, full.alpha_mip_format);
        for level in first..full.alpha_mip_data.len() {
            assert_eq!(
                partial.alpha_mip_format_for_mip(level),
                full.alpha_mip_format_for_mip(level)
            );
        }
    }

    fn unread_or_same(partial: &[Vec<u8>], full: &[Vec<u8>]) -> bool {
        partial
            .iter()
            .zip(full)
            .all(|(partial, full)| partial.is_empty() || partial == full)
    }

    /// 16x16 BC5 with 5 levels (256, 64, 16, 16, 16 bytes): `.3` `.2` `.1`
    /// hold mips 0-2, the base file mips 3-4.
    fn color_only() -> (Vec<u8>, RecordingSiblings) {
        let base = dds(true, header(16, 16, 5, 0), dx10(83, 0), &fill(0xB0, 16 + 16));
        let siblings = RecordingSiblings::new(vec![
            (".3", fill(0x30, 256)),
            (".2", fill(0x20, 64)),
            (".1", fill(0x10, 16)),
        ]);
        (base, siblings)
    }

    #[test]
    fn min_mip_reads_only_the_siblings_from_that_level_on() {
        let (base, siblings) = color_only();
        let full = DdsFile::from_split(&base, &siblings).unwrap();
        assert_eq!(
            full.mip_data.iter().map(Vec::len).collect::<Vec<_>>(),
            vec![256, 64, 16, 16, 16]
        );

        let siblings = color_only().1;
        let partial = DdsFile::from_split_min_mip(&base, &siblings, 2).unwrap();
        assert_eq!(siblings.reads(), vec![".1", ".a"]);
        assert_same_from(&partial, &full, 2);
        assert_eq!(partial.decode_rgba(2).unwrap(), full.decode_rgba(2).unwrap());

        // Levels in the base file need no sibling at all.
        let siblings = color_only().1;
        let partial = DdsFile::from_split_min_mip(&base, &siblings, 3).unwrap();
        assert_eq!(siblings.reads(), vec![".a"]);
        assert_same_from(&partial, &full, 3);
    }

    #[test]
    fn min_mip_past_the_last_level_keeps_the_last_level() {
        let (base, siblings) = color_only();
        let full = DdsFile::from_split(&base, &siblings).unwrap();
        let partial = DdsFile::from_split_min_mip(&base, &siblings, 9).unwrap();
        assert_same_from(&partial, &full, 4);
    }

    /// DDNA-like: 16x16 BC5 color as above plus BC4 alpha (128, 32, 8, 8, 8
    /// bytes) in `.2a` `.1a` and a headered `.a` tail for mips 2-4.
    fn with_alpha() -> (Vec<u8>, RecordingSiblings) {
        let base = dds(true, header(16, 16, 5, 0), dx10(83, 0), &fill(0xB0, 16 + 16));
        let alpha_tail = dds(false, header(16, 16, 5, 0), dx10(80, 0), &fill(0xA0, 8 + 8 + 8));
        let siblings = RecordingSiblings::new(vec![
            (".3", fill(0x30, 256)),
            (".2", fill(0x20, 64)),
            (".1", fill(0x10, 16)),
            (".2a", fill(0x2A, 128)),
            (".1a", fill(0x1A, 32)),
            (".a", alpha_tail),
        ]);
        (base, siblings)
    }

    #[test]
    fn min_mip_skips_the_large_alpha_siblings_too() {
        let (base, siblings) = with_alpha();
        let full = DdsFile::from_split(&base, &siblings).unwrap();
        assert_eq!(
            full.alpha_mip_data.iter().map(Vec::len).collect::<Vec<_>>(),
            vec![128, 32, 8, 8, 8]
        );

        let siblings = with_alpha().1;
        let partial = DdsFile::from_split_min_mip(&base, &siblings, 1).unwrap();
        assert_eq!(siblings.reads(), vec![".1", ".1a", ".2", ".a"]);
        assert_same_from(&partial, &full, 1);
        assert_eq!(
            partial.decode_alpha_mip(1).unwrap(),
            full.decode_alpha_mip(1).unwrap()
        );

        let siblings = with_alpha().1;
        let partial = DdsFile::from_split_min_mip(&base, &siblings, 2).unwrap();
        assert_eq!(siblings.reads(), vec![".1", ".a"]);
        assert_same_from(&partial, &full, 2);
    }

    #[test]
    fn fewer_alpha_levels_clamp_the_first_level_read() {
        // Alpha only in `.2a` `.1a` (no tail): a normal map's mip is clamped to
        // alpha level 1, so color level 1 must be read even for min_mip 3.
        let make = || {
            RecordingSiblings::new(vec![
                (".3", fill(0x30, 256)),
                (".2", fill(0x20, 64)),
                (".1", fill(0x10, 16)),
                (".2a", fill(0x2A, 128)),
                (".1a", fill(0x1A, 32)),
            ])
        };
        let base = dds(true, header(16, 16, 5, 0), dx10(83, 0), &fill(0xB0, 16 + 16));
        let full = DdsFile::from_split(&base, &make()).unwrap();
        assert_eq!(full.alpha_mip_data.len(), 2);

        let siblings = make();
        let partial = DdsFile::from_split_min_mip(&base, &siblings, 3).unwrap();
        assert_eq!(siblings.reads(), vec![".1", ".1a", ".2", ".a"]);
        assert_same_from(&partial, &full, 1);
    }

    #[test]
    fn cubemap_faces_come_from_the_largest_sibling_size_without_reading_it() {
        // 8x8 BC1 cubemap, 4 levels (32, 8, 8, 8 bytes per face): `.2` `.1`
        // hold mips 0-1 for all six faces, the base file mips 2-3.
        let make = || {
            RecordingSiblings::new(vec![
                (".2", fill(0x20, 32 * 6)),
                (".1", fill(0x10, 8 * 6)),
            ])
        };
        let base = dds(
            true,
            header(8, 8, 4, 0xFE00),
            dx10(71, 0x4),
            &fill(0xB0, 8 * 6 * 2),
        );
        let full = DdsFile::from_split(&base, &make()).unwrap();
        assert_eq!(
            full.mip_data.iter().map(Vec::len).collect::<Vec<_>>(),
            vec![32 * 6, 8 * 6, 8 * 6, 8 * 6]
        );

        let siblings = make();
        let partial = DdsFile::from_split_min_mip(&base, &siblings, 1).unwrap();
        assert_eq!(siblings.reads(), vec![".1", ".a"]);
        assert_same_from(&partial, &full, 1);
    }

    #[test]
    fn unreadable_sibling_falls_back_to_the_full_read() {
        let (base, mut siblings) = color_only();
        siblings.broken = Some(".1");
        let full = DdsFile::from_split(&base, &siblings).unwrap();
        let partial = DdsFile::from_split_min_mip(&base, &siblings, 2).unwrap();
        assert_eq!(partial.mip_data, full.mip_data);
    }
}
