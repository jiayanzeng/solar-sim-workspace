//! WP15 offline texture conversion and license audit.
//!
//! Source imagery is deliberately acquired outside the application. This
//! module accepts the smallest portable interchange format we can validate
//! ourselves (binary PPM), writes a standards-compliant single-level KTX2,
//! and audits the provenance sidecar beside every shipped texture. No image
//! codec or network dependency enters the workspace.

use serde::Deserialize;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

const KTX2_MAGIC: [u8; 12] = [
    0xab, b'K', b'T', b'X', b' ', b'2', b'0', 0xbb, b'\r', b'\n', 0x1a, b'\n',
];
const KTX2_HEADER_BYTES: usize = 80;
const KTX2_LEVEL_INDEX_BYTES: usize = 24;
const VK_FORMAT_R8G8B8_SRGB: u32 = 29;
const VK_FORMAT_R8G8B8A8_SRGB: u32 = 43;
const TEXTURE_METADATA_SCHEMA: u32 = 1;
const PUBLIC_DOMAIN_LICENSE: &str = "LicenseRef-Public-Domain-USGov";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RasterImage {
    pub width: u32,
    pub height: u32,
    pub channels: u8,
    pub pixels: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlphaMode {
    Opaque,
    FromLuminance,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TexturePipelineError {
    Read { path: PathBuf, message: String },
    Write { path: PathBuf, message: String },
    InvalidPpm(String),
    InvalidKtx2(String),
}

impl fmt::Display for TexturePipelineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read { path, message } => {
                write!(f, "could not read '{}': {message}", path.display())
            }
            Self::Write { path, message } => {
                write!(f, "could not write '{}': {message}", path.display())
            }
            Self::InvalidPpm(message) => write!(f, "invalid binary PPM: {message}"),
            Self::InvalidKtx2(message) => write!(f, "invalid KTX2: {message}"),
        }
    }
}

impl std::error::Error for TexturePipelineError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextureAuditError {
    pub issues: Vec<String>,
}

impl fmt::Display for TextureAuditError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "texture metadata audit failed")?;
        for issue in &self.issues {
            write!(f, "\n- {issue}")?;
        }
        Ok(())
    }
}

impl std::error::Error for TextureAuditError {}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum TextureUsage {
    Sphere,
    SaturnRing,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TextureMetadata {
    schema_version: u32,
    asset: String,
    title: String,
    body_ids: Vec<String>,
    usage: TextureUsage,
    creator: String,
    source_page: String,
    source_file: String,
    source_sha256: String,
    source_dimensions: [u32; 2],
    retrieved_utc: String,
    license_spdx: String,
    license_name: String,
    license_url: String,
    transform: String,
    output_dimensions: [u32; 2],
    asset_sha256: String,
}

/// Convert a binary PPM file to an uncompressed sRGB KTX2 texture.
///
/// KTX2 is the runtime container; the source PPM is only a reproducible,
/// codec-neutral staging format. Ring strips derive straight alpha from the
/// source luminance so empty space stays translucent.
pub fn convert_texture_file(
    source: &Path,
    output: &Path,
    alpha_mode: AlphaMode,
) -> Result<RasterImage, TexturePipelineError> {
    let source_bytes = fs::read(source).map_err(|error| TexturePipelineError::Read {
        path: source.to_path_buf(),
        message: error.to_string(),
    })?;
    let source_image = parse_binary_ppm(&source_bytes)?;
    let encoded = encode_ktx2(&source_image, alpha_mode)?;
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).map_err(|error| TexturePipelineError::Write {
            path: parent.to_path_buf(),
            message: error.to_string(),
        })?;
    }
    fs::write(output, &encoded).map_err(|error| TexturePipelineError::Write {
        path: output.to_path_buf(),
        message: error.to_string(),
    })?;
    decode_ktx2(&encoded)
}

pub fn parse_binary_ppm(bytes: &[u8]) -> Result<RasterImage, TexturePipelineError> {
    let mut cursor = PpmCursor::new(bytes);
    if cursor.token()? != b"P6" {
        return Err(TexturePipelineError::InvalidPpm("magic must be P6".into()));
    }
    let width = parse_ppm_u32(cursor.token()?, "width")?;
    let height = parse_ppm_u32(cursor.token()?, "height")?;
    let max_value = parse_ppm_u32(cursor.token()?, "maximum channel value")?;
    if width == 0 || height == 0 {
        return Err(TexturePipelineError::InvalidPpm(
            "dimensions must be non-zero".into(),
        ));
    }
    if max_value != 255 {
        return Err(TexturePipelineError::InvalidPpm(format!(
            "maximum channel value is {max_value}, expected 255"
        )));
    }
    cursor.consume_pixel_separator()?;
    let pixel_bytes = (width as usize)
        .checked_mul(height as usize)
        .and_then(|pixels| pixels.checked_mul(3))
        .ok_or_else(|| TexturePipelineError::InvalidPpm("dimensions overflow".into()))?;
    let pixels = cursor.remaining();
    if pixels.len() != pixel_bytes {
        return Err(TexturePipelineError::InvalidPpm(format!(
            "pixel payload is {} bytes, expected {pixel_bytes}",
            pixels.len()
        )));
    }
    Ok(RasterImage {
        width,
        height,
        channels: 3,
        pixels: pixels.to_vec(),
    })
}

fn parse_ppm_u32(token: &[u8], field: &str) -> Result<u32, TexturePipelineError> {
    let text = std::str::from_utf8(token)
        .map_err(|_| TexturePipelineError::InvalidPpm(format!("{field} is not ASCII decimal")))?;
    text.parse::<u32>().map_err(|_| {
        TexturePipelineError::InvalidPpm(format!("{field} '{text}' is not an unsigned integer"))
    })
}

struct PpmCursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> PpmCursor<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn token(&mut self) -> Result<&'a [u8], TexturePipelineError> {
        self.skip_whitespace_and_comments();
        let start = self.offset;
        while self
            .bytes
            .get(self.offset)
            .is_some_and(|byte| !byte.is_ascii_whitespace() && *byte != b'#')
        {
            self.offset += 1;
        }
        if self.offset == start {
            return Err(TexturePipelineError::InvalidPpm(
                "header ended before all fields were present".into(),
            ));
        }
        Ok(&self.bytes[start..self.offset])
    }

    fn skip_whitespace_and_comments(&mut self) {
        loop {
            while self
                .bytes
                .get(self.offset)
                .is_some_and(u8::is_ascii_whitespace)
            {
                self.offset += 1;
            }
            if self.bytes.get(self.offset) != Some(&b'#') {
                break;
            }
            while self
                .bytes
                .get(self.offset)
                .is_some_and(|byte| *byte != b'\n')
            {
                self.offset += 1;
            }
        }
    }

    fn consume_pixel_separator(&mut self) -> Result<(), TexturePipelineError> {
        let Some(separator) = self.bytes.get(self.offset) else {
            return Err(TexturePipelineError::InvalidPpm(
                "missing whitespace before pixel payload".into(),
            ));
        };
        if !separator.is_ascii_whitespace() {
            return Err(TexturePipelineError::InvalidPpm(
                "missing whitespace before pixel payload".into(),
            ));
        }
        // PPM defines one whitespace separator after maxval. Consuming all
        // whitespace would incorrectly eat a valid first pixel byte.
        if *separator == b'\r' && self.bytes.get(self.offset + 1) == Some(&b'\n') {
            self.offset += 2;
        } else {
            self.offset += 1;
        }
        Ok(())
    }

    fn remaining(&self) -> &'a [u8] {
        self.bytes.get(self.offset..).unwrap_or_default()
    }
}

pub fn encode_ktx2(
    source: &RasterImage,
    alpha_mode: AlphaMode,
) -> Result<Vec<u8>, TexturePipelineError> {
    if source.width == 0 || source.height == 0 || source.channels != 3 {
        return Err(TexturePipelineError::InvalidPpm(
            "encoder requires a non-empty RGB image".into(),
        ));
    }
    let source_len = (source.width as usize)
        .checked_mul(source.height as usize)
        .and_then(|pixels| pixels.checked_mul(3))
        .ok_or_else(|| TexturePipelineError::InvalidPpm("dimensions overflow".into()))?;
    if source.pixels.len() != source_len {
        return Err(TexturePipelineError::InvalidPpm(format!(
            "RGB payload is {} bytes, expected {source_len}",
            source.pixels.len()
        )));
    }

    let (format, channels, pixel_data) = match alpha_mode {
        AlphaMode::Opaque => (VK_FORMAT_R8G8B8_SRGB, 3_u8, source.pixels.clone()),
        AlphaMode::FromLuminance => {
            let mut rgba = Vec::with_capacity(source.width as usize * source.height as usize * 4);
            for rgb in source.pixels.chunks_exact(3) {
                rgba.extend_from_slice(rgb);
                let alpha =
                    ((u16::from(rgb[0]) * 54 + u16::from(rgb[1]) * 183 + u16::from(rgb[2]) * 19)
                        / 256) as u8;
                rgba.push(alpha);
            }
            (VK_FORMAT_R8G8B8A8_SRGB, 4_u8, rgba)
        }
    };

    let dfd = encode_basic_dfd(channels);
    let dfd_offset = KTX2_HEADER_BYTES + KTX2_LEVEL_INDEX_BYTES;
    let level_offset = align_to(dfd_offset + dfd.len(), 8);
    let total_len = level_offset
        .checked_add(pixel_data.len())
        .ok_or_else(|| TexturePipelineError::InvalidKtx2("file size overflow".into()))?;
    let mut output = vec![0_u8; total_len];
    output[..12].copy_from_slice(&KTX2_MAGIC);
    write_u32(&mut output, 12, format);
    write_u32(&mut output, 16, 1);
    write_u32(&mut output, 20, source.width);
    write_u32(&mut output, 24, source.height);
    write_u32(&mut output, 28, 0);
    write_u32(&mut output, 32, 0);
    write_u32(&mut output, 36, 1);
    write_u32(&mut output, 40, 1);
    write_u32(&mut output, 44, 0);
    write_u32(&mut output, 48, dfd_offset as u32);
    write_u32(&mut output, 52, dfd.len() as u32);
    write_u32(&mut output, 56, 0);
    write_u32(&mut output, 60, 0);
    write_u64(&mut output, 64, 0);
    write_u64(&mut output, 72, 0);
    write_u64(&mut output, 80, level_offset as u64);
    write_u64(&mut output, 88, pixel_data.len() as u64);
    write_u64(&mut output, 96, pixel_data.len() as u64);
    output[dfd_offset..dfd_offset + dfd.len()].copy_from_slice(&dfd);
    output[level_offset..].copy_from_slice(&pixel_data);
    Ok(output)
}

fn encode_basic_dfd(channels: u8) -> Vec<u8> {
    let sample_count = usize::from(channels);
    let descriptor_bytes = 8 + 16 + sample_count * 16;
    let total_bytes = 4 + descriptor_bytes;
    let mut dfd = vec![0_u8; total_bytes];
    write_u32(&mut dfd, 0, total_bytes as u32);
    write_u32(&mut dfd, 4, 0); // Khronos vendor, basic descriptor type.
    write_u16(&mut dfd, 8, 2);
    write_u16(&mut dfd, 10, descriptor_bytes as u16);
    dfd[12] = 1; // KHR_DF_MODEL_RGBSDA
    dfd[13] = 1; // KHR_DF_PRIMARIES_BT709
    dfd[14] = 2; // KHR_DF_TRANSFER_SRGB
    dfd[15] = 0; // straight alpha
    dfd[16..20].fill(0); // 1x1x1x1 texel block, stored as dimension - 1.
    dfd[20] = channels;
    dfd[21..28].fill(0);
    let channel_ids = if channels == 4 {
        [0_u8, 1, 2, 15]
    } else {
        [0_u8, 1, 2, 0]
    };
    for (sample, channel_id) in channel_ids.iter().take(sample_count).enumerate() {
        let offset = 28 + sample * 16;
        write_u16(&mut dfd, offset, (sample * 8) as u16);
        dfd[offset + 2] = 7; // Eight bits are stored as bitLength - 1.
        dfd[offset + 3] = *channel_id;
        write_u32(&mut dfd, offset + 8, 0);
        write_u32(&mut dfd, offset + 12, 255);
    }
    dfd
}

pub fn decode_ktx2(bytes: &[u8]) -> Result<RasterImage, TexturePipelineError> {
    if bytes.len() < KTX2_HEADER_BYTES + KTX2_LEVEL_INDEX_BYTES {
        return Err(TexturePipelineError::InvalidKtx2(
            "header or level index is truncated".into(),
        ));
    }
    if bytes.get(..12) != Some(KTX2_MAGIC.as_slice()) {
        return Err(TexturePipelineError::InvalidKtx2("bad magic".into()));
    }
    let format = read_u32(bytes, 12)?;
    let channels = match format {
        VK_FORMAT_R8G8B8_SRGB => 3_u8,
        VK_FORMAT_R8G8B8A8_SRGB => 4_u8,
        other => {
            return Err(TexturePipelineError::InvalidKtx2(format!(
                "unsupported Vulkan format {other}"
            )))
        }
    };
    if read_u32(bytes, 16)? != 1
        || read_u32(bytes, 28)? != 0
        || read_u32(bytes, 32)? != 0
        || read_u32(bytes, 36)? != 1
        || read_u32(bytes, 40)? != 1
        || read_u32(bytes, 44)? != 0
    {
        return Err(TexturePipelineError::InvalidKtx2(
            "only one-level, two-dimensional, uncompressed textures are supported".into(),
        ));
    }
    let width = read_u32(bytes, 20)?;
    let height = read_u32(bytes, 24)?;
    if width == 0 || height == 0 {
        return Err(TexturePipelineError::InvalidKtx2(
            "dimensions must be non-zero".into(),
        ));
    }
    let dfd_offset = read_u32(bytes, 48)? as usize;
    let dfd_length = read_u32(bytes, 52)? as usize;
    validate_dfd(bytes, dfd_offset, dfd_length, channels)?;
    if read_u32(bytes, 60)? != 0 || read_u64(bytes, 72)? != 0 {
        return Err(TexturePipelineError::InvalidKtx2(
            "unexpected KVD or SGD payload".into(),
        ));
    }
    let level_offset = usize::try_from(read_u64(bytes, 80)?).map_err(|_| {
        TexturePipelineError::InvalidKtx2("level offset does not fit this platform".into())
    })?;
    let level_length = usize::try_from(read_u64(bytes, 88)?).map_err(|_| {
        TexturePipelineError::InvalidKtx2("level length does not fit this platform".into())
    })?;
    let uncompressed_length = usize::try_from(read_u64(bytes, 96)?).map_err(|_| {
        TexturePipelineError::InvalidKtx2("uncompressed length does not fit this platform".into())
    })?;
    let expected = (width as usize)
        .checked_mul(height as usize)
        .and_then(|pixels| pixels.checked_mul(usize::from(channels)))
        .ok_or_else(|| TexturePipelineError::InvalidKtx2("dimensions overflow".into()))?;
    if level_length != expected || uncompressed_length != expected {
        return Err(TexturePipelineError::InvalidKtx2(format!(
            "level is {level_length}/{uncompressed_length} bytes, expected {expected}"
        )));
    }
    let level_end = level_offset
        .checked_add(level_length)
        .ok_or_else(|| TexturePipelineError::InvalidKtx2("level bounds overflow".into()))?;
    let pixels = bytes.get(level_offset..level_end).ok_or_else(|| {
        TexturePipelineError::InvalidKtx2("level data is outside the file".into())
    })?;
    if level_end != bytes.len() {
        return Err(TexturePipelineError::InvalidKtx2(
            "trailing bytes after the only mip level".into(),
        ));
    }
    Ok(RasterImage {
        width,
        height,
        channels,
        pixels: pixels.to_vec(),
    })
}

fn validate_dfd(
    bytes: &[u8],
    offset: usize,
    length: usize,
    channels: u8,
) -> Result<(), TexturePipelineError> {
    let dfd = bytes
        .get(offset..offset.saturating_add(length))
        .ok_or_else(|| TexturePipelineError::InvalidKtx2("DFD is outside the file".into()))?;
    let expected_length = 4 + 8 + 16 + usize::from(channels) * 16;
    if length != expected_length || read_u32(dfd, 0)? as usize != length {
        return Err(TexturePipelineError::InvalidKtx2(
            "DFD length does not match the pixel layout".into(),
        ));
    }
    if read_u32(dfd, 4)? != 0
        || read_u16(dfd, 8)? != 2
        || read_u16(dfd, 10)? as usize != length - 4
        || dfd.get(12..16) != Some([1_u8, 1, 2, 0].as_slice())
        || dfd.get(16..20) != Some([0_u8; 4].as_slice())
        || dfd.get(20) != Some(&channels)
    {
        return Err(TexturePipelineError::InvalidKtx2(
            "DFD is not the expected BT.709 sRGB layout".into(),
        ));
    }
    let channel_ids = if channels == 4 {
        [0_u8, 1, 2, 15]
    } else {
        [0_u8, 1, 2, 0]
    };
    for (sample, channel_id) in channel_ids.iter().take(usize::from(channels)).enumerate() {
        let sample_offset = 28 + sample * 16;
        if read_u16(dfd, sample_offset)? != (sample * 8) as u16
            || dfd.get(sample_offset + 2) != Some(&7)
            || dfd.get(sample_offset + 3) != Some(channel_id)
            || read_u32(dfd, sample_offset + 8)? != 0
            || read_u32(dfd, sample_offset + 12)? != 255
        {
            return Err(TexturePipelineError::InvalidKtx2(format!(
                "DFD sample {sample} does not match the pixel layout"
            )));
        }
    }
    Ok(())
}

fn align_to(value: usize, alignment: usize) -> usize {
    value.div_ceil(alignment) * alignment
}

fn write_u16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn write_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, TexturePipelineError> {
    let raw = bytes.get(offset..offset + 2).ok_or_else(|| {
        TexturePipelineError::InvalidKtx2(format!("u16 at byte {offset} is truncated"))
    })?;
    let mut value = [0_u8; 2];
    value.copy_from_slice(raw);
    Ok(u16::from_le_bytes(value))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, TexturePipelineError> {
    let raw = bytes.get(offset..offset + 4).ok_or_else(|| {
        TexturePipelineError::InvalidKtx2(format!("u32 at byte {offset} is truncated"))
    })?;
    let mut value = [0_u8; 4];
    value.copy_from_slice(raw);
    Ok(u32::from_le_bytes(value))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, TexturePipelineError> {
    let raw = bytes.get(offset..offset + 8).ok_or_else(|| {
        TexturePipelineError::InvalidKtx2(format!("u64 at byte {offset} is truncated"))
    })?;
    let mut value = [0_u8; 8];
    value.copy_from_slice(raw);
    Ok(u64::from_le_bytes(value))
}

/// Validate every `.ktx2`/`.license.json` pair and verify the shipped bytes.
pub fn audit_texture_directory(root: &Path) -> Result<usize, TextureAuditError> {
    let mut files = Vec::new();
    if let Err(error) = collect_files(root, &mut files) {
        return Err(TextureAuditError {
            issues: vec![format!("could not enumerate '{}': {error}", root.display())],
        });
    }
    files.sort();
    let texture_files: Vec<_> = files
        .iter()
        .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("ktx2"))
        .cloned()
        .collect();
    let metadata_files: Vec<_> = files
        .iter()
        .filter(|path| {
            path.file_name()
                .and_then(|value| value.to_str())
                .is_some_and(|name| name.ends_with(".license.json"))
        })
        .cloned()
        .collect();
    let mut issues = Vec::new();
    if texture_files.is_empty() {
        issues.push("no KTX2 textures found".into());
    }

    for texture_path in &texture_files {
        let metadata_path = texture_path.with_extension("license.json");
        if !metadata_path.is_file() {
            issues.push(format!(
                "{} has no sibling {}",
                relative_display(root, texture_path),
                metadata_path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("<invalid metadata filename>")
            ));
            continue;
        }
        audit_texture_pair(root, texture_path, &metadata_path, &mut issues);
    }
    for metadata_path in &metadata_files {
        let Some(name) = metadata_path.file_name().and_then(|value| value.to_str()) else {
            issues.push(format!(
                "{} has an invalid filename",
                metadata_path.display()
            ));
            continue;
        };
        let Some(stem) = name.strip_suffix(".license.json") else {
            continue;
        };
        let texture_path = metadata_path.with_file_name(format!("{stem}.ktx2"));
        if !texture_path.is_file() {
            issues.push(format!(
                "{} has no sibling {stem}.ktx2",
                relative_display(root, metadata_path)
            ));
        }
    }

    if issues.is_empty() {
        Ok(texture_files.len())
    } else {
        Err(TextureAuditError { issues })
    }
}

fn audit_texture_pair(
    root: &Path,
    texture_path: &Path,
    metadata_path: &Path,
    issues: &mut Vec<String>,
) {
    let relative = relative_display(root, texture_path);
    let metadata_bytes = match fs::read(metadata_path) {
        Ok(bytes) => bytes,
        Err(error) => {
            issues.push(format!(
                "could not read {}: {error}",
                relative_display(root, metadata_path)
            ));
            return;
        }
    };
    let metadata: TextureMetadata = match serde_json::from_slice(&metadata_bytes) {
        Ok(metadata) => metadata,
        Err(error) => {
            issues.push(format!(
                "{} is invalid JSON metadata: {error}",
                relative_display(root, metadata_path)
            ));
            return;
        }
    };
    if metadata.schema_version != TEXTURE_METADATA_SCHEMA {
        issues.push(format!(
            "{relative}: metadata schema {} is not {TEXTURE_METADATA_SCHEMA}",
            metadata.schema_version
        ));
    }
    if metadata.asset != relative {
        issues.push(format!(
            "{relative}: metadata asset '{}' does not match its path",
            metadata.asset
        ));
    }
    for (field, value) in [
        ("title", metadata.title.as_str()),
        ("creator", metadata.creator.as_str()),
        ("license_name", metadata.license_name.as_str()),
        ("transform", metadata.transform.as_str()),
    ] {
        if value.trim().is_empty() {
            issues.push(format!("{relative}: {field} must not be empty"));
        }
    }
    if metadata.body_ids.is_empty() || metadata.body_ids.iter().any(|id| id.trim().is_empty()) {
        issues.push(format!(
            "{relative}: body_ids must contain at least one non-empty catalog id"
        ));
    }
    if metadata.license_spdx != PUBLIC_DOMAIN_LICENSE {
        issues.push(format!(
            "{relative}: license_spdx '{}' is not the approved public-domain identifier",
            metadata.license_spdx
        ));
    }
    for (field, url) in [
        ("source_page", metadata.source_page.as_str()),
        ("source_file", metadata.source_file.as_str()),
        ("license_url", metadata.license_url.as_str()),
    ] {
        if !url.starts_with("https://") {
            issues.push(format!("{relative}: {field} must be an HTTPS URL"));
        }
    }
    if !is_approved_source_url(&metadata.source_page)
        || !is_approved_source_url(&metadata.source_file)
    {
        issues.push(format!(
            "{relative}: source URLs must be official NASA, USGS, or NASA GitHub resources"
        ));
    }
    if metadata.license_url != "https://www.nasa.gov/nasa-brand-center/images-and-media/"
        && metadata.license_url
            != "https://www.usgs.gov/information-policies-and-instructions/copyrights-and-credits"
        && metadata.license_url != "https://github.com/nasa/NASA-3D-Resources"
    {
        issues.push(format!(
            "{relative}: license_url is not an approved public-domain policy route"
        ));
    }
    if !is_sha256(&metadata.source_sha256) || !is_sha256(&metadata.asset_sha256) {
        issues.push(format!(
            "{relative}: source_sha256 and asset_sha256 must be lowercase SHA-256 hex"
        ));
    }
    if metadata.source_dimensions.contains(&0) || metadata.output_dimensions.contains(&0) {
        issues.push(format!(
            "{relative}: source/output dimensions must be non-zero"
        ));
    }
    if metadata.retrieved_utc.len() != 10
        || metadata.retrieved_utc.as_bytes().get(4) != Some(&b'-')
        || metadata.retrieved_utc.as_bytes().get(7) != Some(&b'-')
    {
        issues.push(format!(
            "{relative}: retrieved_utc must use the YYYY-MM-DD form"
        ));
    }
    if !metadata.transform.contains("xtask") || !metadata.transform.contains("convert-texture") {
        issues.push(format!(
            "{relative}: transform must record the xtask convert-texture command"
        ));
    }
    let texture_bytes = match fs::read(texture_path) {
        Ok(bytes) => bytes,
        Err(error) => {
            issues.push(format!("could not read {relative}: {error}"));
            return;
        }
    };
    if sha256_hex(&texture_bytes) != metadata.asset_sha256 {
        issues.push(format!(
            "{relative}: asset_sha256 does not match shipped bytes"
        ));
    }
    match decode_ktx2(&texture_bytes) {
        Ok(image) => {
            if [image.width, image.height] != metadata.output_dimensions {
                issues.push(format!(
                    "{relative}: KTX2 dimensions {}x{} do not match metadata {}x{}",
                    image.width,
                    image.height,
                    metadata.output_dimensions[0],
                    metadata.output_dimensions[1]
                ));
            }
            match metadata.usage {
                TextureUsage::Sphere if [image.width, image.height] != [2048, 1024] => {
                    issues.push(format!(
                        "{relative}: sphere texture must be the 2K 2048x1024 equirectangular size"
                    ));
                }
                TextureUsage::SaturnRing
                    if image.width != 2048 || image.height == 0 || image.channels != 4 =>
                {
                    issues.push(format!(
                        "{relative}: Saturn ring must be a 2K-wide RGBA strip"
                    ));
                }
                _ => {}
            }
        }
        Err(error) => issues.push(format!("{relative}: {error}")),
    }
    let lower_name = relative.to_ascii_lowercase();
    if ["logo", "insignia", "worm", "meatball"]
        .iter()
        .any(|needle| lower_name.contains(needle))
    {
        issues.push(format!("{relative}: NASA branding assets are prohibited"));
    }
}

fn collect_files(root: &Path, output: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_files(&path, output)?;
        } else if path.is_file() {
            output.push(path);
        }
    }
    Ok(())
}

fn relative_display(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn is_approved_source_url(url: &str) -> bool {
    [
        "https://science.nasa.gov/",
        "https://assets.science.nasa.gov/",
        "https://svs.gsfc.nasa.gov/",
        "https://astrogeology.usgs.gov/",
        "https://planetarymaps.usgs.gov/",
        "https://github.com/nasa/NASA-3D-Resources",
        "https://raw.githubusercontent.com/nasa/NASA-3D-Resources/",
    ]
    .iter()
    .any(|prefix| url.starts_with(prefix))
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// Dependency-free SHA-256 used to bind metadata to exact source/output bytes.
pub fn sha256_hex(bytes: &[u8]) -> String {
    const INITIAL: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    const ROUND: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let bit_length = (bytes.len() as u64).wrapping_mul(8);
    let mut padded = bytes.to_vec();
    padded.push(0x80);
    while padded.len() % 64 != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_length.to_be_bytes());
    let mut hash = INITIAL;
    for block in padded.chunks_exact(64) {
        let mut words = [0_u32; 64];
        for (index, chunk) in block.chunks_exact(4).enumerate() {
            let mut word = [0_u8; 4];
            word.copy_from_slice(chunk);
            words[index] = u32::from_be_bytes(word);
        }
        for index in 16..64 {
            let s0 = words[index - 15].rotate_right(7)
                ^ words[index - 15].rotate_right(18)
                ^ (words[index - 15] >> 3);
            let s1 = words[index - 2].rotate_right(17)
                ^ words[index - 2].rotate_right(19)
                ^ (words[index - 2] >> 10);
            words[index] = words[index - 16]
                .wrapping_add(s0)
                .wrapping_add(words[index - 7])
                .wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = hash;
        for index in 0..64 {
            let sum1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let choice = (e & f) ^ ((!e) & g);
            let temp1 = h
                .wrapping_add(sum1)
                .wrapping_add(choice)
                .wrapping_add(ROUND[index])
                .wrapping_add(words[index]);
            let sum0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let majority = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = sum0.wrapping_add(majority);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }
        hash[0] = hash[0].wrapping_add(a);
        hash[1] = hash[1].wrapping_add(b);
        hash[2] = hash[2].wrapping_add(c);
        hash[3] = hash[3].wrapping_add(d);
        hash[4] = hash[4].wrapping_add(e);
        hash[5] = hash[5].wrapping_add(f);
        hash[6] = hash[6].wrapping_add(g);
        hash[7] = hash[7].wrapping_add(h);
    }
    hash.iter().map(|word| format!("{word:08x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

    struct TestDir(PathBuf);

    impl TestDir {
        fn new(label: &str) -> Self {
            let index = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "solar-sim-wp15-{label}-{}-{index}",
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn ppm(width: u32, height: u32, pixels: &[u8]) -> Vec<u8> {
        let mut output =
            format!("P6\n# deterministic fixture\n{width} {height}\n255\n").into_bytes();
        output.extend_from_slice(pixels);
        output
    }

    #[test]
    fn ktx2_pipeline_round_trips_one_texture_without_a_codec_dependency() {
        let pixels = [255, 0, 0, 0, 255, 0, 0, 0, 255, 250, 128, 4];
        let source = parse_binary_ppm(&ppm(2, 2, &pixels)).unwrap();
        let encoded = encode_ktx2(&source, AlphaMode::Opaque).unwrap();
        let decoded = decode_ktx2(&encoded).unwrap();
        assert_eq!(decoded, source);
        assert_eq!(encoded.len() % 3, 1, "container must not be raw PPM bytes");

        let rgba = decode_ktx2(&encode_ktx2(&source, AlphaMode::FromLuminance).unwrap()).unwrap();
        assert_eq!(rgba.channels, 4);
        assert_eq!(rgba.pixels[3], 53);
        assert_eq!(rgba.pixels[7], 182);
    }

    #[test]
    fn corrupt_ppm_and_ktx2_inputs_are_rejected_without_panicking() {
        for bytes in [
            b"P3\n1 1\n255\n\0\0\0".as_slice(),
            b"P6\n0 1\n255\n".as_slice(),
            b"P6\n1 1\n65535\n\0\0\0".as_slice(),
            b"P6\n1 1\n255\n\0\0".as_slice(),
        ] {
            assert!(parse_binary_ppm(bytes).is_err(), "accepted {bytes:?}");
        }
        assert!(decode_ktx2(b"not a texture").is_err());
        let source = parse_binary_ppm(&ppm(1, 1, &[1, 2, 3])).unwrap();
        let mut encoded = encode_ktx2(&source, AlphaMode::Opaque).unwrap();
        encoded[88] = 2;
        assert!(decode_ktx2(&encoded).is_err());
    }

    #[test]
    fn metadata_audit_rejects_an_orphan_texture_and_then_verifies_exact_bytes() {
        let directory = TestDir::new("metadata");
        let texture = directory.0.join("earth.ktx2");
        let source = RasterImage {
            width: 2048,
            height: 1024,
            channels: 3,
            pixels: vec![7; 2048 * 1024 * 3],
        };
        let bytes = encode_ktx2(&source, AlphaMode::Opaque).unwrap();
        fs::write(&texture, &bytes).unwrap();
        let error = audit_texture_directory(&directory.0).unwrap_err();
        assert!(error
            .issues
            .iter()
            .any(|issue| issue.contains("no sibling")));

        let digest = sha256_hex(&bytes);
        let metadata = format!(
            r#"{{
  "schema_version": 1,
  "asset": "earth.ktx2",
  "title": "Earth test texture",
  "body_ids": ["earth"],
  "usage": "sphere",
  "creator": "NASA",
  "source_page": "https://science.nasa.gov/3d-resources/earth-a/",
  "source_file": "https://assets.science.nasa.gov/earth.jpg",
  "source_sha256": "{}",
  "source_dimensions": [2048, 1024],
  "retrieved_utc": "2026-07-14",
  "license_spdx": "LicenseRef-Public-Domain-USGov",
  "license_name": "Public Domain - U.S. Government Work",
  "license_url": "https://www.nasa.gov/nasa-brand-center/images-and-media/",
  "transform": "cargo run -p xtask -- convert-texture --source earth.ppm --out earth.ktx2",
  "output_dimensions": [2048, 1024],
  "asset_sha256": "{digest}"
}}"#,
            "a".repeat(64)
        );
        fs::write(directory.0.join("earth.license.json"), metadata).unwrap();
        assert_eq!(audit_texture_directory(&directory.0).unwrap(), 1);

        let mut changed = bytes;
        changed.push(0);
        fs::write(&texture, changed).unwrap();
        let error = audit_texture_directory(&directory.0).unwrap_err();
        assert!(error
            .issues
            .iter()
            .any(|issue| issue.contains("asset_sha256")));
    }

    #[test]
    fn sha256_matches_published_empty_and_abc_vectors() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
