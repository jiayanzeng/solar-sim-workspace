//! WP13 NASA HEASARC BSC5P parser and deterministic offline baker.
//!
//! The approved input is a four-column, HR-sorted VOTable BINARY response
//! from HEASARC's TAP service. The baker retains the visually brightest rows,
//! rotates J2000 equatorial coordinates into ecliptic-J2000, and writes a
//! compact point record stream for the renderer. It never fetches data.

use std::fmt;
use std::path::Path;

pub const DEFAULT_STAR_LIMIT: usize = 5_000;
pub const EXPECTED_BSC5P_ROWS: usize = 9_110;
pub const EXPECTED_BSC5P_STARS: usize = 9_096;
pub const ECLIPTIC_OBLIQUITY_DEG: f64 = 23.439_291_1;
const HEASARC_RECORD_BYTES: usize = 22;
// HEASARC explicitly identifies these retained HR rows as non-stellar
// historical objects. They have added coordinates but null V magnitudes.
const NON_STELLAR_HR: [u16; 14] = [
    92, 95, 182, 1057, 1841, 2472, 2496, 3515, 3671, 6309, 6515, 7189, 7539, 8296,
];
const BAKED_MAGIC: &[u8; 8] = b"SSBSC1\0\0";
const BAKED_HEADER_BYTES: usize = 12;
const BAKED_RECORD_BYTES: usize = 22;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CatalogStar {
    pub hr: u16,
    pub right_ascension_rad: f64,
    pub declination_rad: f64,
    pub magnitude: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BakedStar {
    pub hr: u16,
    pub position_ecliptic: [f32; 3],
    pub magnitude: f32,
    pub point_size: f32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceRecordError {
    pub record: usize,
    pub field: &'static str,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HeasarcSourceError {
    QueryStatusMissing,
    QueryFailed(String),
    Schema(String),
    StreamMissing,
    InvalidBase64 { offset: usize },
    BinaryLength { bytes: usize },
    UnexpectedCounts { rows: usize, stars: usize },
    InvalidRecords(Vec<SourceRecordError>),
}

impl fmt::Display for HeasarcSourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::QueryStatusMissing => write!(f, "HEASARC query status is missing"),
            Self::QueryFailed(status) => write!(f, "HEASARC query failed: {status}"),
            Self::Schema(message) => write!(f, "unexpected HEASARC schema: {message}"),
            Self::StreamMissing => write!(f, "HEASARC base64 BINARY stream is missing"),
            Self::InvalidBase64 { offset } => {
                write!(f, "invalid HEASARC base64 stream at byte {offset}")
            }
            Self::BinaryLength { bytes } => write!(
                f,
                "HEASARC binary stream is {bytes} bytes, not a multiple of {HEASARC_RECORD_BYTES}"
            ),
            Self::UnexpectedCounts { rows, stars } => write!(
                f,
                "HEASARC BSC5P has {rows} rows/{stars} stars; expected {EXPECTED_BSC5P_ROWS}/{EXPECTED_BSC5P_STARS}"
            ),
            Self::InvalidRecords(errors) => {
                write!(f, "HEASARC BSC5P contains invalid records")?;
                for error in errors {
                    write!(
                        f,
                        "\n- record {}, {}: {}",
                        error.record, error.field, error.message
                    )?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for HeasarcSourceError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BakedStarError {
    BadMagic,
    Truncated,
    TooManyRecords { count: usize },
    LengthMismatch { expected: usize, actual: usize },
    InvalidRecord { index: usize },
}

impl fmt::Display for BakedStarError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadMagic => write!(f, "not a solar-sim BSC starfield"),
            Self::Truncated => write!(f, "starfield header is truncated"),
            Self::TooManyRecords { count } => {
                write!(
                    f,
                    "starfield record count {count} exceeds its binary format"
                )
            }
            Self::LengthMismatch { expected, actual } => {
                write!(f, "starfield is {actual} bytes; expected {expected}")
            }
            Self::InvalidRecord { index } => write!(
                f,
                "starfield record {index} is not finite or on the unit sphere"
            ),
        }
    }
}

impl std::error::Error for BakedStarError {}

/// Parse the approved HEASARC TAP response for:
/// `SELECT hr,ra,dec,vmag FROM bsc5p ORDER BY hr`.
pub fn parse_heasarc_votable(source: &str) -> Result<Vec<CatalogStar>, HeasarcSourceError> {
    parse_heasarc_votable_with_count(source).map(|(stars, _)| stars)
}

fn parse_heasarc_votable_with_count(
    source: &str,
) -> Result<(Vec<CatalogStar>, usize), HeasarcSourceError> {
    validate_query_status(source)?;
    validate_schema(source)?;
    let stream = base64_stream(source)?;
    let binary = decode_base64(stream)?;
    let row_count = binary.len() / HEASARC_RECORD_BYTES;
    decode_heasarc_records(&binary).map(|stars| (stars, row_count))
}

fn validate_query_status(source: &str) -> Result<(), HeasarcSourceError> {
    let mut remaining = source;
    while let Some(start) = remaining.find("<INFO") {
        remaining = &remaining[start..];
        let Some(end) = remaining.find('>') else {
            return Err(HeasarcSourceError::QueryStatusMissing);
        };
        let tag = &remaining[..=end];
        if xml_attribute(tag, "name") == Some("QUERY_STATUS") {
            return match xml_attribute(tag, "value") {
                Some("OK") => Ok(()),
                Some(value) => Err(HeasarcSourceError::QueryFailed(value.to_string())),
                None => Err(HeasarcSourceError::QueryStatusMissing),
            };
        }
        remaining = &remaining[end + 1..];
    }
    Err(HeasarcSourceError::QueryStatusMissing)
}

fn validate_schema(source: &str) -> Result<(), HeasarcSourceError> {
    let header = source
        .split_once("<DATA>")
        .map(|(header, _)| header)
        .ok_or_else(|| HeasarcSourceError::Schema("DATA element is missing".into()))?;
    let mut fields = Vec::new();
    let mut remaining = header;
    while let Some(start) = remaining.find("<FIELD") {
        remaining = &remaining[start..];
        let Some(end) = remaining.find('>') else {
            return Err(HeasarcSourceError::Schema(
                "FIELD opening tag is incomplete".into(),
            ));
        };
        let tag = &remaining[..=end];
        fields.push((
            xml_attribute(tag, "name"),
            xml_attribute(tag, "datatype"),
            xml_attribute(tag, "unit"),
        ));
        remaining = &remaining[end + 1..];
    }
    let expected = [
        ("hr", "short", None),
        ("ra", "double", Some("deg")),
        ("dec", "double", Some("deg")),
        ("vmag", "float", None),
    ];
    if fields.len() != expected.len() {
        return Err(HeasarcSourceError::Schema(format!(
            "expected four fields, found {}",
            fields.len()
        )));
    }
    for (index, ((name, datatype, unit), expected)) in fields.into_iter().zip(expected).enumerate()
    {
        if name != Some(expected.0) || datatype != Some(expected.1) || unit != expected.2 {
            return Err(HeasarcSourceError::Schema(format!(
                "field {} must be {} {} {:?}, found {:?} {:?} {:?}",
                index + 1,
                expected.0,
                expected.1,
                expected.2,
                name,
                datatype,
                unit
            )));
        }
    }
    Ok(())
}

fn xml_attribute<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let needle = format!("{name}=\"");
    let (_, value) = tag.split_once(&needle)?;
    value.split_once('"').map(|(value, _)| value)
}

fn base64_stream(source: &str) -> Result<&str, HeasarcSourceError> {
    let start = source
        .find("<STREAM")
        .ok_or(HeasarcSourceError::StreamMissing)?;
    let stream = &source[start..];
    let opening_end = stream.find('>').ok_or(HeasarcSourceError::StreamMissing)?;
    let opening = &stream[..=opening_end];
    if xml_attribute(opening, "encoding") != Some("base64") {
        return Err(HeasarcSourceError::Schema(
            "STREAM encoding must be base64".into(),
        ));
    }
    let content = &stream[opening_end + 1..];
    let closing = content
        .find("</STREAM>")
        .ok_or(HeasarcSourceError::StreamMissing)?;
    Ok(&content[..closing])
}

fn decode_base64(source: &str) -> Result<Vec<u8>, HeasarcSourceError> {
    let compact: Vec<_> = source
        .bytes()
        .filter(|byte| !byte.is_ascii_whitespace())
        .collect();
    if compact.len() % 4 != 0 {
        return Err(HeasarcSourceError::InvalidBase64 {
            offset: compact.len(),
        });
    }
    let mut output = Vec::with_capacity(compact.len() / 4 * 3);
    for (chunk_index, chunk) in compact.chunks_exact(4).enumerate() {
        let offset = chunk_index * 4;
        let a = base64_value(chunk[0]).ok_or(HeasarcSourceError::InvalidBase64 { offset })?;
        let b = base64_value(chunk[1])
            .ok_or(HeasarcSourceError::InvalidBase64 { offset: offset + 1 })?;
        let c = if chunk[2] == b'=' {
            64
        } else {
            base64_value(chunk[2])
                .ok_or(HeasarcSourceError::InvalidBase64 { offset: offset + 2 })?
        };
        let d = if chunk[3] == b'=' {
            64
        } else {
            base64_value(chunk[3])
                .ok_or(HeasarcSourceError::InvalidBase64 { offset: offset + 3 })?
        };
        let last = chunk_index + 1 == compact.len() / 4;
        if a == 64 || b == 64 || c == 64 && d != 64 || d == 64 && !last {
            return Err(HeasarcSourceError::InvalidBase64 { offset });
        }
        output.push((a << 2) | (b >> 4));
        if c != 64 {
            output.push((b << 4) | (c >> 2));
        }
        if d != 64 {
            output.push((c << 6) | d);
        }
    }
    Ok(output)
}

fn base64_value(byte: u8) -> Option<u8> {
    match byte {
        b'A'..=b'Z' => Some(byte - b'A'),
        b'a'..=b'z' => Some(byte - b'a' + 26),
        b'0'..=b'9' => Some(byte - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

fn decode_heasarc_records(bytes: &[u8]) -> Result<Vec<CatalogStar>, HeasarcSourceError> {
    if !bytes.len().is_multiple_of(HEASARC_RECORD_BYTES) {
        return Err(HeasarcSourceError::BinaryLength { bytes: bytes.len() });
    }
    let mut stars = Vec::with_capacity(bytes.len() / HEASARC_RECORD_BYTES);
    let mut errors = Vec::new();
    let mut previous_hr = 0_u16;
    for (index, record) in bytes.chunks_exact(HEASARC_RECORD_BYTES).enumerate() {
        let record_number = index + 1;
        let hr = i16::from_be_bytes([record[0], record[1]]);
        let right_ascension_deg = f64::from_be_bytes([
            record[2], record[3], record[4], record[5], record[6], record[7], record[8], record[9],
        ]);
        let declination_deg = f64::from_be_bytes([
            record[10], record[11], record[12], record[13], record[14], record[15], record[16],
            record[17],
        ]);
        let magnitude = f32::from_be_bytes([record[18], record[19], record[20], record[21]]);
        if !(1..=9_110).contains(&hr) {
            errors.push(SourceRecordError {
                record: record_number,
                field: "hr",
                message: format!("{hr} is outside 1..=9110"),
            });
        }
        let hr = u16::try_from(hr).unwrap_or(0);
        if hr <= previous_hr {
            errors.push(SourceRecordError {
                record: record_number,
                field: "hr",
                message: format!("{hr} does not follow {previous_hr}"),
            });
        }
        if !right_ascension_deg.is_finite() || !(0.0..360.0).contains(&right_ascension_deg) {
            errors.push(SourceRecordError {
                record: record_number,
                field: "ra",
                message: format!("{right_ascension_deg} degrees is invalid"),
            });
        }
        if !declination_deg.is_finite() || !(-90.0..=90.0).contains(&declination_deg) {
            errors.push(SourceRecordError {
                record: record_number,
                field: "dec",
                message: format!("{declination_deg} degrees is invalid"),
            });
        }
        let recognized_non_star = magnitude.is_nan() && NON_STELLAR_HR.contains(&hr);
        if !magnitude.is_finite() && !recognized_non_star {
            errors.push(SourceRecordError {
                record: record_number,
                field: "vmag",
                message: "magnitude is not finite".into(),
            });
        }
        previous_hr = hr;
        if recognized_non_star {
            continue;
        }
        stars.push(CatalogStar {
            hr,
            right_ascension_rad: right_ascension_deg.to_radians(),
            declination_rad: declination_deg.to_radians(),
            magnitude,
        });
    }
    if errors.is_empty() {
        Ok(stars)
    } else {
        Err(HeasarcSourceError::InvalidRecords(errors))
    }
}

pub fn bake_stars(mut stars: Vec<CatalogStar>, limit: usize) -> Vec<BakedStar> {
    stars.sort_by(|left, right| {
        left.magnitude
            .total_cmp(&right.magnitude)
            .then(left.hr.cmp(&right.hr))
    });
    stars.truncate(limit);
    stars.into_iter().map(bake_star).collect()
}

fn bake_star(star: CatalogStar) -> BakedStar {
    let cos_dec = star.declination_rad.cos();
    let equatorial = [
        cos_dec * star.right_ascension_rad.cos(),
        cos_dec * star.right_ascension_rad.sin(),
        star.declination_rad.sin(),
    ];
    let obliquity = ECLIPTIC_OBLIQUITY_DEG.to_radians();
    let (sin_obliquity, cos_obliquity) = obliquity.sin_cos();
    let ecliptic = [
        equatorial[0],
        cos_obliquity * equatorial[1] + sin_obliquity * equatorial[2],
        -sin_obliquity * equatorial[1] + cos_obliquity * equatorial[2],
    ];
    BakedStar {
        hr: star.hr,
        position_ecliptic: ecliptic.map(|value| value as f32),
        magnitude: star.magnitude,
        point_size: (1.0 + 0.35 * (6.5 - star.magnitude)).clamp(0.5, 4.0),
    }
}

pub fn encode_baked(stars: &[BakedStar]) -> Result<Vec<u8>, BakedStarError> {
    let count = u32::try_from(stars.len())
        .map_err(|_| BakedStarError::TooManyRecords { count: stars.len() })?;
    let capacity = stars
        .len()
        .checked_mul(BAKED_RECORD_BYTES)
        .and_then(|bytes| bytes.checked_add(BAKED_HEADER_BYTES))
        .ok_or(BakedStarError::TooManyRecords { count: stars.len() })?;
    let mut output = Vec::with_capacity(capacity);
    output.extend_from_slice(BAKED_MAGIC);
    output.extend_from_slice(&count.to_le_bytes());
    for star in stars {
        output.extend_from_slice(&star.hr.to_le_bytes());
        for coordinate in star.position_ecliptic {
            output.extend_from_slice(&coordinate.to_le_bytes());
        }
        output.extend_from_slice(&star.magnitude.to_le_bytes());
        output.extend_from_slice(&star.point_size.to_le_bytes());
    }
    Ok(output)
}

pub fn decode_baked(bytes: &[u8]) -> Result<Vec<BakedStar>, BakedStarError> {
    if bytes.len() < BAKED_HEADER_BYTES {
        return Err(BakedStarError::Truncated);
    }
    if &bytes[..8] != BAKED_MAGIC {
        return Err(BakedStarError::BadMagic);
    }
    let count = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]) as usize;
    let expected = count
        .checked_mul(BAKED_RECORD_BYTES)
        .and_then(|record_bytes| record_bytes.checked_add(BAKED_HEADER_BYTES))
        .ok_or(BakedStarError::TooManyRecords { count })?;
    if bytes.len() != expected {
        return Err(BakedStarError::LengthMismatch {
            expected,
            actual: bytes.len(),
        });
    }
    let mut stars = Vec::with_capacity(count);
    for index in 0..count {
        let offset = BAKED_HEADER_BYTES + index * BAKED_RECORD_BYTES;
        let hr = u16::from_le_bytes([bytes[offset], bytes[offset + 1]]);
        let mut cursor = offset + 2;
        let mut read_f32 = || {
            let value = f32::from_le_bytes([
                bytes[cursor],
                bytes[cursor + 1],
                bytes[cursor + 2],
                bytes[cursor + 3],
            ]);
            cursor += 4;
            value
        };
        let position_ecliptic = [read_f32(), read_f32(), read_f32()];
        let magnitude = read_f32();
        let point_size = read_f32();
        let norm_squared: f32 = position_ecliptic.iter().map(|value| value * value).sum();
        if !position_ecliptic.iter().all(|value| value.is_finite())
            || !magnitude.is_finite()
            || !point_size.is_finite()
            || point_size <= 0.0
            || (norm_squared - 1.0).abs() > 2.0e-5
        {
            return Err(BakedStarError::InvalidRecord { index });
        }
        stars.push(BakedStar {
            hr,
            position_ecliptic,
            magnitude,
            point_size,
        });
    }
    Ok(stars)
}

pub fn bake_catalog_file(source: &Path, out: &Path, limit: usize) -> anyhow::Result<usize> {
    let text = std::fs::read_to_string(source)?;
    let (parsed, rows) = parse_heasarc_votable_with_count(&text)?;
    if rows != EXPECTED_BSC5P_ROWS || parsed.len() != EXPECTED_BSC5P_STARS {
        return Err(HeasarcSourceError::UnexpectedCounts {
            rows,
            stars: parsed.len(),
        }
        .into());
    }
    let baked = bake_stars(parsed, limit);
    std::fs::write(out, encode_baked(&baked)?)?;
    Ok(baked.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn binary_record(hr: i16, ra_deg: f64, dec_deg: f64, magnitude: f32) -> Vec<u8> {
        let mut record = Vec::with_capacity(HEASARC_RECORD_BYTES);
        record.extend_from_slice(&hr.to_be_bytes());
        record.extend_from_slice(&ra_deg.to_be_bytes());
        record.extend_from_slice(&dec_deg.to_be_bytes());
        record.extend_from_slice(&magnitude.to_be_bytes());
        record
    }

    fn votable(binary: &[u8]) -> String {
        format!(
            r#"<?xml version="1.0"?>
<VOTABLE><RESOURCE><INFO name="QUERY_STATUS" value="OK"/><TABLE>
<FIELD name="hr" datatype="short"/>
<FIELD name="ra" datatype="double" unit="deg"/>
<FIELD name="dec" datatype="double" unit="deg"/>
<FIELD name="vmag" datatype="float"/>
<DATA><BINARY><STREAM encoding="base64">{}</STREAM></BINARY></DATA>
</TABLE></RESOURCE></VOTABLE>"#,
            encode_base64(binary)
        )
    }

    fn encode_base64(bytes: &[u8]) -> String {
        const ALPHABET: &[u8; 64] =
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut output = String::new();
        for chunk in bytes.chunks(3) {
            let a = chunk[0];
            let b = chunk.get(1).copied().unwrap_or(0);
            let c = chunk.get(2).copied().unwrap_or(0);
            output.push(ALPHABET[(a >> 2) as usize] as char);
            output.push(ALPHABET[(((a & 0x03) << 4) | (b >> 4)) as usize] as char);
            output.push(if chunk.len() > 1 {
                ALPHABET[(((b & 0x0f) << 2) | (c >> 6)) as usize] as char
            } else {
                '='
            });
            output.push(if chunk.len() > 2 {
                ALPHABET[(c & 0x3f) as usize] as char
            } else {
                '='
            });
        }
        output
    }

    #[test]
    fn heasarc_parser_rejects_schema_drift_corruption_and_bad_ranges() {
        let valid = votable(&binary_record(1, 10.0, -20.0, 1.5));
        assert_eq!(parse_heasarc_votable(&valid).unwrap().len(), 1);

        let drifted = valid.replace("name=\"vmag\"", "name=\"visual_mag\"");
        assert!(matches!(
            parse_heasarc_votable(&drifted),
            Err(HeasarcSourceError::Schema(_))
        ));
        let corrupt = valid.replace("</STREAM>", "*</STREAM>");
        assert!(matches!(
            parse_heasarc_votable(&corrupt),
            Err(HeasarcSourceError::InvalidBase64 { .. })
        ));
        let invalid = votable(&binary_record(1, 361.0, -91.0, f32::NAN));
        let Err(HeasarcSourceError::InvalidRecords(errors)) = parse_heasarc_votable(&invalid)
        else {
            panic!("invalid HEASARC values must be rejected collectively");
        };
        assert_eq!(errors.len(), 3);

        let retained_non_star = votable(&binary_record(92, 10.0, -20.0, f32::NAN));
        assert!(parse_heasarc_votable(&retained_non_star)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn bake_keeps_five_thousand_brightest_finite_unit_sphere_stars() {
        let source: Vec<_> = (0..5_100)
            .map(|index| CatalogStar {
                hr: (index + 1) as u16,
                right_ascension_rad: index as f64 * std::f64::consts::TAU / 5_100.0,
                declination_rad: ((index % 180) as f64 - 90.0).to_radians() * 0.99,
                magnitude: index as f32 / 1_000.0,
            })
            .collect();
        let baked = bake_stars(source, DEFAULT_STAR_LIMIT);
        assert_eq!(baked.len(), 5_000);
        assert_eq!(baked.first().unwrap().hr, 1);
        assert_eq!(baked.last().unwrap().hr, 5_000);
        for star in &baked {
            assert!(star.position_ecliptic.iter().all(|value| value.is_finite()));
            let norm: f32 = star
                .position_ecliptic
                .iter()
                .map(|value| value * value)
                .sum();
            assert!((norm - 1.0).abs() < 2.0e-6, "HR {} norm={norm}", star.hr);
        }
        let encoded = encode_baked(&baked).unwrap();
        assert_eq!(decode_baked(&encoded).unwrap(), baked);
    }

    #[test]
    fn polaris_is_about_twenty_three_degrees_from_the_ecliptic_pole() {
        let ra_deg = (2.0 + 31.0 / 60.0 + 49.1 / 3_600.0) * 15.0;
        let dec_deg = 89.0 + 15.0 / 60.0 + 51.0 / 3_600.0;
        let polaris_source = votable(&binary_record(424, ra_deg, dec_deg, 1.98));
        let polaris = bake_stars(parse_heasarc_votable(&polaris_source).unwrap(), 1)[0];
        let separation_deg = polaris.position_ecliptic[2]
            .clamp(-1.0, 1.0)
            .acos()
            .to_degrees();
        assert!((separation_deg - 23.4).abs() < 1.0, "{separation_deg}");
        assert!(
            polaris.position_ecliptic[1].abs() > 0.1,
            "tilt was not applied"
        );
    }
}
