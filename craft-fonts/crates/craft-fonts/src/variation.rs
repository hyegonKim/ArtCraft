//! Pure-Rust OpenType SFNT directory, `fvar`, `STAT`, and `name` variable-font axis inspector.
//!
//! Clean-room implementation supporting TrueType (`0x00010000` / `true`), CFF OpenType (`OTTO`),
//! and TrueType Collections (`ttcf`, face 0). All bounds and arithmetic are checked so hostile or
//! truncated font binaries never panic.

const SFNT_TRUETYPE: u32 = 0x0001_0000;
const SFNT_OTTO: u32 = 0x4F54_544F;
const SFNT_TRUE_APPLE: u32 = 0x7472_7565;
const SFNT_TYP1_APPLE: u32 = 0x7479_7031;
const SFNT_TTCF: u32 = 0x7474_6366;

/// Flag bit in OpenType `fvar` `VariationAxisRecord.flags` indicating the axis should be hidden in UI.
pub const FVAR_AXIS_HIDDEN: u16 = 0x0001;

/// Registered OpenType variation axis tags commonly exposed in typography panels.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum KnownAxisTag {
    /// `wght` — Weight (1..=1000, typically 100..=900, Regular = 400).
    Weight,
    /// `wdth` — Width (percentage of normal, Normal = 100).
    Width,
    /// `opsz` — Optical size (typographic points).
    OpticalSize,
    /// `slnt` — Slant (degrees, counter-clockwise positive / clockwise negative).
    Slant,
    /// `ital` — Italic (`0.0` upright to `1.0` italic).
    Italic,
    /// Any custom or unregistered 4-byte OpenType axis tag (e.g. `GRAD`, `YTLC`).
    Custom([u8; 4]),
}

impl KnownAxisTag {
    /// Classify a 4-byte OpenType axis tag.
    pub fn from_tag(tag: [u8; 4]) -> Self {
        match &tag {
            b"wght" => Self::Weight,
            b"wdth" => Self::Width,
            b"opsz" => Self::OpticalSize,
            b"slnt" => Self::Slant,
            b"ital" => Self::Italic,
            _ => Self::Custom(tag),
        }
    }

    /// Return the raw 4-byte OpenType tag.
    pub fn tag_bytes(self) -> [u8; 4] {
        match self {
            Self::Weight => *b"wght",
            Self::Width => *b"wdth",
            Self::OpticalSize => *b"opsz",
            Self::Slant => *b"slnt",
            Self::Italic => *b"ital",
            Self::Custom(tag) => tag,
        }
    }

    /// Default human-readable English label when a font's `name` table omits an axis label.
    pub fn fallback_label(self) -> &'static str {
        match self {
            Self::Weight => "Weight",
            Self::Width => "Width",
            Self::OpticalSize => "Optical Size",
            Self::Slant => "Slant",
            Self::Italic => "Italic",
            Self::Custom(_) => "Custom Axis",
        }
    }
}

/// An axis value label parsed from an OpenType `STAT` (Style Attributes) table.
#[derive(Clone, Debug, PartialEq)]
pub struct StatAxisStop {
    /// Localized label resolved from the `name` table (e.g. `"Regular"`, `"SemiBold"`, `"Condensed"`).
    pub label: String,
    /// Nominal coordinate value on this axis.
    pub value: f32,
    /// Optional range bounds `(min, max)` when encoded via `STAT` AxisValue Format 2.
    pub range: Option<(f32, f32)>,
    /// Optional linked style value (e.g. `400.0` -> `700.0` bold link) from `STAT` Format 3.
    pub linked_value: Option<f32>,
    /// Raw `STAT` AxisValue flags (e.g. `0x0002` `ELIDABLE_AXIS_VALUE_NAME`).
    pub flags: u16,
}

/// One named instance's coordinate along a specific [`VariationAxis`].
#[derive(Clone, Debug, PartialEq)]
pub struct NamedInstanceStop {
    /// Subfamily display name resolved from the `name` table (e.g. `"SemiBold"`).
    pub name: String,
    /// `name` table ID for the subfamily name.
    pub subfamily_name_id: u16,
    /// Optional PostScript name resolved from the `name` table (e.g. `"Montserrat-SemiBold"`).
    pub postscript_name: Option<String>,
    /// Coordinate value for this axis in user design units.
    pub value: f32,
    /// Full coordinate vector across all axes in `fvar` order.
    pub all_coordinates: Vec<f32>,
    /// Raw `InstanceRecord.flags`.
    pub flags: u16,
}

/// A font-wide named variation instance from the OpenType `fvar` table.
#[derive(Clone, Debug, PartialEq)]
pub struct NamedVariationInstance {
    /// Subfamily display name resolved from `name` (or `"Instance #N"` fallback).
    pub name: String,
    /// `name` table ID for the subfamily name.
    pub subfamily_name_id: u16,
    /// Optional `name` table ID for the PostScript name.
    pub postscript_name_id: Option<u16>,
    /// Optional PostScript name resolved from `name`.
    pub postscript_name: Option<String>,
    /// Raw `InstanceRecord.flags`.
    pub flags: u16,
    /// `(axis_tag, value)` pairs in `fvar` axis declaration order.
    pub coordinates: Vec<([u8; 4], f32)>,
}

/// One design-variation axis extracted from an OpenType `fvar` (and optional `STAT` + `name`) table.
#[derive(Clone, Debug, PartialEq)]
pub struct VariationAxis {
    /// Raw 4-byte OpenType axis tag (e.g. `*b"wght"`).
    pub tag: [u8; 4],
    /// Strongly-typed known or custom tag classification.
    pub known_tag: KnownAxisTag,
    /// Human-readable axis name from `name` / `STAT` (e.g. `"Weight"`).
    pub name: String,
    /// `name` table ID referenced by `fvar` `axisNameID`.
    pub axis_name_id: u16,
    /// Minimum design-space coordinate (converted from Fixed 16.16).
    pub min_value: f32,
    /// Default design-space coordinate (converted from Fixed 16.16).
    pub default_value: f32,
    /// Maximum design-space coordinate (converted from Fixed 16.16).
    pub max_value: f32,
    /// Raw OpenType `fvar` axis flags (`0x0001` = hidden).
    pub flags: u16,
    /// Optional `STAT` `axisOrdering` index when a `STAT` table is present.
    pub stat_ordering: Option<u16>,
    /// Optional `STAT` discrete/range stops associated with this axis.
    pub stat_stops: Vec<StatAxisStop>,
    /// Named instances declared in `fvar`, projected onto this axis alongside full coordinates.
    pub named_instances: Vec<NamedInstanceStop>,
}

impl VariationAxis {
    /// Four-character ASCII string representation of `self.tag` (non-ASCII bytes replaced with `?`).
    pub fn tag_str(&self) -> String {
        self.tag
            .iter()
            .map(|&b| {
                if (0x20..=0x7E).contains(&b) {
                    char::from(b)
                } else {
                    '?'
                }
            })
            .collect()
    }

    /// Whether the `HIDDEN_AXIS` (`0x0001`) flag bit is set.
    pub fn is_hidden(&self) -> bool {
        (self.flags & FVAR_AXIS_HIDDEN) != 0
    }

    /// Clamp an arbitrary user coordinate into `[min_value, max_value]`.
    pub fn clamp(&self, value: f32) -> f32 {
        if value.is_nan() {
            return self.default_value;
        }
        value.clamp(self.min_value, self.max_value)
    }

    /// Normalize a user coordinate to OpenType normalized `[-1.0, 1.0]` space.
    pub fn normalize(&self, value: f32) -> f32 {
        let v = self.clamp(value);
        if (v - self.default_value).abs() <= f32::EPSILON {
            0.0
        } else if v < self.default_value {
            let span = self.default_value - self.min_value;
            if span <= f32::EPSILON {
                0.0
            } else {
                ((v - self.default_value) / span).clamp(-1.0, 0.0)
            }
        } else {
            let span = self.max_value - self.default_value;
            if span <= f32::EPSILON {
                0.0
            } else {
                ((v - self.default_value) / span).clamp(0.0, 1.0)
            }
        }
    }
}

/// Complete variable-font inspection summary combining `fvar`, `STAT`, and `name`.
#[derive(Clone, Debug, PartialEq)]
pub struct VariableFontInfo {
    /// Family name from the `name` table, when present.
    pub family_name: Option<String>,
    /// Variation axes in `fvar` declaration order.
    pub axes: Vec<VariationAxis>,
    /// Named instances in `fvar` declaration order.
    pub instances: Vec<NamedVariationInstance>,
    /// Whether an OpenType `STAT` table was present and parsed.
    pub has_stat_table: bool,
}

/// Parse OpenType variation axes (`fvar` + `STAT` + `name`) from raw `.ttf` / `.otf` / `.ttc` bytes.
///
/// Returns an empty `Vec` for static fonts (which have no `fvar` table) or malformed/truncated inputs.
pub fn parse_variation_axes(ttf_or_otf_bytes: &[u8]) -> Vec<VariationAxis> {
    inspect_variable_font(ttf_or_otf_bytes)
        .map(|info| info.axes)
        .unwrap_or_default()
}

/// Inspect a variable font's `fvar`, `STAT`, and `name` tables.
///
/// Returns `None` if `ttf_or_otf_bytes` is not a valid SFNT container, lacks an `fvar` table,
/// or declares zero variation axes.
pub fn inspect_variable_font(ttf_or_otf_bytes: &[u8]) -> Option<VariableFontInfo> {
    let dir_offset = sfnt_directory_offset(ttf_or_otf_bytes)?;
    let fvar = find_sfnt_table(ttf_or_otf_bytes, dir_offset, b"fvar")?;
    let name_table = find_sfnt_table(ttf_or_otf_bytes, dir_offset, b"name");
    let stat_table = find_sfnt_table(ttf_or_otf_bytes, dir_offset, b"STAT");

    let major = read_u16_be(fvar, 0)?;
    let minor = read_u16_be(fvar, 2)?;
    if major != 1 || minor != 0 {
        return None;
    }

    let axes_offset = usize::from(read_u16_be(fvar, 4)?);
    let axis_count = usize::from(read_u16_be(fvar, 8)?);
    let axis_size = usize::from(read_u16_be(fvar, 10)?);
    let instance_count = usize::from(read_u16_be(fvar, 12)?);
    let instance_size = usize::from(read_u16_be(fvar, 14)?);

    if axis_count == 0 || axis_size < 20 {
        return None;
    }

    let axes_byte_len = axis_count.checked_mul(axis_size)?;
    let instances_offset = axes_offset.checked_add(axes_byte_len)?;
    if instances_offset > fvar.len() {
        return None;
    }

    let parsed_stat = stat_table.and_then(|stat| parse_stat_table(stat, name_table));

    let mut raw_axes = Vec::with_capacity(axis_count);
    for idx in 0..axis_count {
        let record_offset = axes_offset.checked_add(idx.checked_mul(axis_size)?)?;
        let record = fvar.get(record_offset..record_offset.checked_add(axis_size)?)?;
        let tag = read_tag(record, 0)?;
        let min_fixed = read_i32_be(record, 4)?;
        let default_fixed = read_i32_be(record, 8)?;
        let max_fixed = read_i32_be(record, 12)?;
        let flags = read_u16_be(record, 16)?;
        let axis_name_id = read_u16_be(record, 18)?;

        let min_value = fixed_16_16_to_f32(min_fixed);
        let max_value = fixed_16_16_to_f32(max_fixed);
        if min_value > max_value {
            return None;
        }
        let default_value = fixed_16_16_to_f32(default_fixed).clamp(min_value, max_value);

        let known_tag = KnownAxisTag::from_tag(tag);
        let stat_entry = parsed_stat
            .as_ref()
            .and_then(|s| s.axes.iter().find(|a| a.tag == tag));

        let resolved_name = name_table
            .and_then(|nt| lookup_name_id(nt, axis_name_id))
            .or_else(|| stat_entry.and_then(|se| se.name.clone()))
            .unwrap_or_else(|| known_tag.fallback_label().to_string());

        raw_axes.push(VariationAxis {
            tag,
            known_tag,
            name: resolved_name,
            axis_name_id,
            min_value,
            default_value,
            max_value,
            flags,
            stat_ordering: stat_entry.map(|se| se.ordering),
            stat_stops: stat_entry.map(|se| se.stops.clone()).unwrap_or_default(),
            named_instances: Vec::new(),
        });
    }

    let min_instance_size = axis_count.checked_mul(4)?.checked_add(4)?;
    let mut instances = Vec::new();
    if instance_count > 0
        && instance_size >= min_instance_size
        && let Some(total_inst_bytes) = instance_count.checked_mul(instance_size)
        && let Some(inst_end) = instances_offset.checked_add(total_inst_bytes)
        && inst_end <= fvar.len()
    {
        for inst_idx in 0..instance_count {
            let rec_off = instances_offset + inst_idx * instance_size;
            let Some(rec) = fvar.get(rec_off..rec_off + instance_size) else {
                break;
            };
            let Some(subfamily_name_id) = read_u16_be(rec, 0) else {
                break;
            };
            let Some(inst_flags) = read_u16_be(rec, 2) else {
                break;
            };
            let mut coords = Vec::with_capacity(axis_count);
            let mut tagged_coords = Vec::with_capacity(axis_count);
            let mut valid = true;
            for (axis_i, axis) in raw_axes.iter().enumerate() {
                if let Some(raw_coord) = read_i32_be(rec, 4 + axis_i * 4) {
                    let val = fixed_16_16_to_f32(raw_coord);
                    coords.push(val);
                    tagged_coords.push((axis.tag, val));
                } else {
                    valid = false;
                    break;
                }
            }
            if !valid {
                break;
            }

            let ps_offset = 4 + axis_count * 4;
            let postscript_name_id = if instance_size >= ps_offset + 2 {
                read_u16_be(rec, ps_offset).filter(|&id| id != 0 && id != 0xFFFF)
            } else {
                None
            };

            let instance_name = name_table
                .and_then(|nt| lookup_name_id(nt, subfamily_name_id))
                .unwrap_or_else(|| format!("Instance #{}", inst_idx + 1));
            let postscript_name = postscript_name_id
                .and_then(|ps_id| name_table.and_then(|nt| lookup_name_id(nt, ps_id)));

            for (axis_i, axis) in raw_axes.iter_mut().enumerate() {
                if let Some(&coord_val) = coords.get(axis_i) {
                    axis.named_instances.push(NamedInstanceStop {
                        name: instance_name.clone(),
                        subfamily_name_id,
                        postscript_name: postscript_name.clone(),
                        value: coord_val,
                        all_coordinates: coords.clone(),
                        flags: inst_flags,
                    });
                }
            }

            instances.push(NamedVariationInstance {
                name: instance_name,
                subfamily_name_id,
                postscript_name_id,
                postscript_name,
                flags: inst_flags,
                coordinates: tagged_coords,
            });
        }
    }

    // Typographic family (ID 16) preferred over legacy family (ID 1).
    let family_name =
        name_table.and_then(|nt| lookup_name_id(nt, 16).or_else(|| lookup_name_id(nt, 1)));

    Some(VariableFontInfo {
        family_name,
        axes: raw_axes,
        instances,
        has_stat_table: parsed_stat.is_some(),
    })
}

#[derive(Clone, Debug)]
struct ParsedStatAxis {
    tag: [u8; 4],
    name: Option<String>,
    ordering: u16,
    stops: Vec<StatAxisStop>,
}

#[derive(Clone, Debug)]
struct ParsedStat {
    axes: Vec<ParsedStatAxis>,
}

fn parse_stat_table(stat: &[u8], name_table: Option<&[u8]>) -> Option<ParsedStat> {
    let major = read_u16_be(stat, 0)?;
    if major != 1 {
        return None;
    }
    let design_axis_size = usize::from(read_u16_be(stat, 4)?);
    let design_axis_count = usize::from(read_u16_be(stat, 6)?);
    let design_axes_offset = usize::try_from(read_u32_be(stat, 8)?).ok()?;
    let axis_value_count = usize::from(read_u16_be(stat, 12)?);
    let axis_values_base = usize::try_from(read_u32_be(stat, 14)?).ok()?;

    if design_axis_count == 0 || design_axis_size < 8 {
        return None;
    }

    let mut axes = Vec::with_capacity(design_axis_count);
    for idx in 0..design_axis_count {
        let off = design_axes_offset.checked_add(idx.checked_mul(design_axis_size)?)?;
        let rec = stat.get(off..off.checked_add(design_axis_size)?)?;
        let tag = read_tag(rec, 0)?;
        let name_id = read_u16_be(rec, 4)?;
        let ordering = read_u16_be(rec, 6)?;
        let name = name_table.and_then(|nt| lookup_name_id(nt, name_id));
        axes.push(ParsedStatAxis {
            tag,
            name,
            ordering,
            stops: Vec::new(),
        });
    }

    if axis_value_count > 0 && axis_values_base < stat.len() {
        for idx in 0..axis_value_count {
            let offset_slot = axis_values_base.checked_add(idx.checked_mul(2)?)?;
            let Some(rel_off) = read_u16_be(stat, offset_slot) else {
                break;
            };
            let Some(table_off) = axis_values_base.checked_add(usize::from(rel_off)) else {
                continue;
            };
            let Some(sub) = stat.get(table_off..) else {
                continue;
            };
            let Some(format) = read_u16_be(sub, 0) else {
                continue;
            };
            match format {
                1 => {
                    let Some(axis_index) = read_u16_be(sub, 2).map(usize::from) else {
                        continue;
                    };
                    let Some(flags) = read_u16_be(sub, 4) else {
                        continue;
                    };
                    let Some(val_name_id) = read_u16_be(sub, 6) else {
                        continue;
                    };
                    let Some(raw_val) = read_i32_be(sub, 8) else {
                        continue;
                    };
                    if let Some(target_axis) = axes.get_mut(axis_index) {
                        let label = name_table
                            .and_then(|nt| lookup_name_id(nt, val_name_id))
                            .unwrap_or_else(|| format!("Stop #{val_name_id}"));
                        target_axis.stops.push(StatAxisStop {
                            label,
                            value: fixed_16_16_to_f32(raw_val),
                            range: None,
                            linked_value: None,
                            flags,
                        });
                    }
                }
                2 => {
                    let Some(axis_index) = read_u16_be(sub, 2).map(usize::from) else {
                        continue;
                    };
                    let Some(flags) = read_u16_be(sub, 4) else {
                        continue;
                    };
                    let Some(val_name_id) = read_u16_be(sub, 6) else {
                        continue;
                    };
                    let Some(nominal) = read_i32_be(sub, 8) else {
                        continue;
                    };
                    let Some(r_min) = read_i32_be(sub, 12) else {
                        continue;
                    };
                    let Some(r_max) = read_i32_be(sub, 16) else {
                        continue;
                    };
                    if let Some(target_axis) = axes.get_mut(axis_index) {
                        let label = name_table
                            .and_then(|nt| lookup_name_id(nt, val_name_id))
                            .unwrap_or_else(|| format!("Stop #{val_name_id}"));
                        target_axis.stops.push(StatAxisStop {
                            label,
                            value: fixed_16_16_to_f32(nominal),
                            range: Some((fixed_16_16_to_f32(r_min), fixed_16_16_to_f32(r_max))),
                            linked_value: None,
                            flags,
                        });
                    }
                }
                3 => {
                    let Some(axis_index) = read_u16_be(sub, 2).map(usize::from) else {
                        continue;
                    };
                    let Some(flags) = read_u16_be(sub, 4) else {
                        continue;
                    };
                    let Some(val_name_id) = read_u16_be(sub, 6) else {
                        continue;
                    };
                    let Some(raw_val) = read_i32_be(sub, 8) else {
                        continue;
                    };
                    let Some(linked) = read_i32_be(sub, 12) else {
                        continue;
                    };
                    if let Some(target_axis) = axes.get_mut(axis_index) {
                        let label = name_table
                            .and_then(|nt| lookup_name_id(nt, val_name_id))
                            .unwrap_or_else(|| format!("Stop #{val_name_id}"));
                        target_axis.stops.push(StatAxisStop {
                            label,
                            value: fixed_16_16_to_f32(raw_val),
                            range: None,
                            linked_value: Some(fixed_16_16_to_f32(linked)),
                            flags,
                        });
                    }
                }
                _ => {}
            }
        }
    }

    Some(ParsedStat { axes })
}

fn sfnt_directory_offset(bytes: &[u8]) -> Option<usize> {
    let magic = read_u32_be(bytes, 0)?;
    match magic {
        SFNT_TRUETYPE | SFNT_OTTO | SFNT_TRUE_APPLE | SFNT_TYP1_APPLE => Some(0),
        SFNT_TTCF => {
            let num_fonts = read_u32_be(bytes, 8)?;
            if num_fonts == 0 {
                return None;
            }
            let first_offset = usize::try_from(read_u32_be(bytes, 12)?).ok()?;
            let inner_magic = read_u32_be(bytes, first_offset)?;
            match inner_magic {
                SFNT_TRUETYPE | SFNT_OTTO | SFNT_TRUE_APPLE | SFNT_TYP1_APPLE => Some(first_offset),
                _ => None,
            }
        }
        _ => None,
    }
}

fn find_sfnt_table<'a>(
    bytes: &'a [u8],
    dir_offset: usize,
    wanted_tag: &[u8; 4],
) -> Option<&'a [u8]> {
    let num_tables = usize::from(read_u16_be(bytes, dir_offset.checked_add(4)?)?);
    let records_start = dir_offset.checked_add(12)?;
    for idx in 0..num_tables {
        let rec_off = records_start.checked_add(idx.checked_mul(16)?)?;
        let tag = read_tag(bytes, rec_off)?;
        if &tag == wanted_tag {
            let table_offset = usize::try_from(read_u32_be(bytes, rec_off + 8)?).ok()?;
            let table_len = usize::try_from(read_u32_be(bytes, rec_off + 12)?).ok()?;
            let table_end = table_offset.checked_add(table_len)?;
            return bytes.get(table_offset..table_end);
        }
    }
    None
}

/// Resolve a `nameID` from an OpenType `name` table, preferring English UTF-16BE records.
pub(crate) fn lookup_name_id(name_table: &[u8], target_id: u16) -> Option<String> {
    let count = usize::from(read_u16_be(name_table, 2)?);
    let storage_offset = usize::from(read_u16_be(name_table, 4)?);
    let mut best_rank: u8 = 0;
    let mut best_value: Option<String> = None;

    for idx in 0..count {
        let rec_off = 6usize.checked_add(idx.checked_mul(12)?)?;
        let Some(platform_id) = read_u16_be(name_table, rec_off) else {
            break;
        };
        let Some(encoding_id) = read_u16_be(name_table, rec_off + 2) else {
            break;
        };
        let Some(language_id) = read_u16_be(name_table, rec_off + 4) else {
            break;
        };
        let Some(name_id) = read_u16_be(name_table, rec_off + 6) else {
            break;
        };
        if name_id != target_id {
            continue;
        }
        let Some(str_len) = read_u16_be(name_table, rec_off + 8).map(usize::from) else {
            continue;
        };
        let Some(str_rel) = read_u16_be(name_table, rec_off + 10).map(usize::from) else {
            continue;
        };
        let Some(abs_start) = storage_offset.checked_add(str_rel) else {
            continue;
        };
        let Some(abs_end) = abs_start.checked_add(str_len) else {
            continue;
        };
        let Some(raw_bytes) = name_table.get(abs_start..abs_end) else {
            continue;
        };

        let (rank, decoded) = match (platform_id, encoding_id, language_id) {
            // Windows Unicode BMP / UCS-4, US English (0x0409)
            (3, 1 | 10, 0x0409) => (4u8, decode_utf16be(raw_bytes)),
            // Unicode platform
            (0, _, _) => (3u8, decode_utf16be(raw_bytes)),
            // Windows Unicode, other language
            (3, 1 | 10, _) => (2u8, decode_utf16be(raw_bytes)),
            // Macintosh Roman English
            (1, 0, _) => (1u8, decode_mac_roman_ascii(raw_bytes)),
            _ => (0u8, None),
        };

        if rank > best_rank
            && let Some(s) = decoded.filter(|s| !s.trim().is_empty())
        {
            best_rank = rank;
            best_value = Some(s);
            if best_rank == 4 {
                break;
            }
        }
    }

    best_value
}

fn decode_utf16be(bytes: &[u8]) -> Option<String> {
    if !bytes.len().is_multiple_of(2) {
        return None;
    }
    let units = (0..bytes.len() / 2).filter_map(|i| read_u16_be(bytes, i * 2));
    let mut out = String::new();
    for ch in char::decode_utf16(units) {
        out.push(ch.ok()?);
    }
    Some(out)
}

fn decode_mac_roman_ascii(bytes: &[u8]) -> Option<String> {
    let mut out = String::with_capacity(bytes.len());
    for &b in bytes {
        if (0x20..=0x7E).contains(&b) {
            out.push(char::from(b));
        } else {
            return None;
        }
    }
    Some(out)
}

fn fixed_16_16_to_f32(raw: i32) -> f32 {
    (f64::from(raw) / 65536.0) as f32
}

fn read_u16_be(data: &[u8], offset: usize) -> Option<u16> {
    let slice = data.get(offset..offset.checked_add(2)?)?;
    let arr: [u8; 2] = slice.try_into().ok()?;
    Some(u16::from_be_bytes(arr))
}

fn read_u32_be(data: &[u8], offset: usize) -> Option<u32> {
    let slice = data.get(offset..offset.checked_add(4)?)?;
    let arr: [u8; 4] = slice.try_into().ok()?;
    Some(u32::from_be_bytes(arr))
}

fn read_i32_be(data: &[u8], offset: usize) -> Option<i32> {
    let slice = data.get(offset..offset.checked_add(4)?)?;
    let arr: [u8; 4] = slice.try_into().ok()?;
    Some(i32::from_be_bytes(arr))
}

fn read_tag(data: &[u8], offset: usize) -> Option<[u8; 4]> {
    let slice = data.get(offset..offset.checked_add(4)?)?;
    slice.try_into().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn push_u16(buf: &mut Vec<u8>, v: u16) {
        buf.extend_from_slice(&v.to_be_bytes());
    }

    fn push_u32(buf: &mut Vec<u8>, v: u32) {
        buf.extend_from_slice(&v.to_be_bytes());
    }

    fn push_fixed(buf: &mut Vec<u8>, v: f64) {
        let raw = (v * 65536.0).round() as i32;
        buf.extend_from_slice(&raw.to_be_bytes());
    }

    fn build_name_table(entries: &[(u16, &str)]) -> Vec<u8> {
        let mut storage = Vec::new();
        let mut records = Vec::new();
        for &(name_id, text) in entries {
            let start = storage.len() as u16;
            for code_unit in text.encode_utf16() {
                push_u16(&mut storage, code_unit);
            }
            let len = (storage.len() as u16) - start;
            records.push((name_id, len, start));
        }

        let mut table = Vec::new();
        push_u16(&mut table, 0); // format 0
        push_u16(&mut table, records.len() as u16);
        let storage_offset = 6 + (records.len() as u16) * 12;
        push_u16(&mut table, storage_offset);
        for (name_id, len, rel_off) in records {
            push_u16(&mut table, 3); // Windows
            push_u16(&mut table, 1); // Unicode BMP
            push_u16(&mut table, 0x0409); // en-US
            push_u16(&mut table, name_id);
            push_u16(&mut table, len);
            push_u16(&mut table, rel_off);
        }
        table.extend_from_slice(&storage);
        table
    }

    fn build_sfnt(sfnt_magic: u32, tables: &[([u8; 4], Vec<u8>)]) -> Vec<u8> {
        let mut out = Vec::new();
        push_u32(&mut out, sfnt_magic);
        push_u16(&mut out, tables.len() as u16);
        push_u16(&mut out, 0);
        push_u16(&mut out, 0);
        push_u16(&mut out, 0);

        let mut data_offset = 12 + tables.len() * 16;
        for (tag, payload) in tables {
            out.extend_from_slice(tag);
            push_u32(&mut out, 0); // checksum
            push_u32(&mut out, data_offset as u32);
            push_u32(&mut out, payload.len() as u32);
            data_offset += payload.len();
        }
        for (_, payload) in tables {
            out.extend_from_slice(payload);
        }
        out
    }

    #[test]
    fn parses_synthetic_five_axis_otf_with_stat_and_named_instances() {
        let name_bytes = build_name_table(&[
            (1, "Synthetic Pro Variable"),
            (256, "Weight"),
            (257, "Width"),
            (258, "Optical Size"),
            (259, "Slant"),
            (260, "Italic"),
            (300, "Regular"),
            (301, "SyntheticPro-Regular"),
            (302, "Bold Condensed"),
            (303, "SyntheticPro-BoldCond"),
            (400, "Book"),
            (401, "Bold"),
        ]);

        // 5 axes: wght, wdth, opsz, slnt, ital; 2 instances with postScriptNameID (instanceSize = 4 + 5*4 + 2 = 26)
        let mut fvar = Vec::new();
        push_u16(&mut fvar, 1); // major
        push_u16(&mut fvar, 0); // minor
        push_u16(&mut fvar, 16); // axesArrayOffset
        push_u16(&mut fvar, 2); // reserved
        push_u16(&mut fvar, 5); // axisCount
        push_u16(&mut fvar, 20); // axisSize
        push_u16(&mut fvar, 2); // instanceCount
        push_u16(&mut fvar, 26); // instanceSize

        let axis_specs = [
            (*b"wght", 100.0, 400.0, 900.0, 0u16, 256u16),
            (*b"wdth", 75.0, 100.0, 125.0, 0u16, 257u16),
            (*b"opsz", 8.0, 14.0, 72.0, 0u16, 258u16),
            (*b"slnt", -12.0, 0.0, 0.0, 0u16, 259u16),
            (*b"ital", 0.0, 0.0, 1.0, FVAR_AXIS_HIDDEN, 260u16),
        ];
        for (tag, min_v, def_v, max_v, flags, name_id) in axis_specs {
            fvar.extend_from_slice(&tag);
            push_fixed(&mut fvar, min_v);
            push_fixed(&mut fvar, def_v);
            push_fixed(&mut fvar, max_v);
            push_u16(&mut fvar, flags);
            push_u16(&mut fvar, name_id);
        }

        // Instance 1: Regular
        push_u16(&mut fvar, 300);
        push_u16(&mut fvar, 0);
        for v in [400.0, 100.0, 14.0, 0.0, 0.0] {
            push_fixed(&mut fvar, v);
        }
        push_u16(&mut fvar, 301);

        // Instance 2: Bold Condensed
        push_u16(&mut fvar, 302);
        push_u16(&mut fvar, 0);
        for v in [700.0, 75.0, 14.0, -6.0, 0.0] {
            push_fixed(&mut fvar, v);
        }
        push_u16(&mut fvar, 303);

        // STAT v1.1 with 1 design axis (wght) and 2 stops (format 3 linked + format 1)
        let mut stat = Vec::new();
        push_u16(&mut stat, 1); // major
        push_u16(&mut stat, 1); // minor
        push_u16(&mut stat, 8); // designAxisSize
        push_u16(&mut stat, 1); // designAxisCount
        push_u32(&mut stat, 20); // designAxesOffset
        push_u16(&mut stat, 2); // axisValueCount
        push_u32(&mut stat, 28); // offsetToAxisValueOffsets
        push_u16(&mut stat, 2); // elidedFallbackNameID
        // DesignAxis 0 @ 20:
        stat.extend_from_slice(b"wght");
        push_u16(&mut stat, 256);
        push_u16(&mut stat, 0);
        // AxisValueOffsets @ 28 (2 * u16 = 4 bytes -> values start at rel 4 and rel 20):
        push_u16(&mut stat, 4);
        push_u16(&mut stat, 20);
        // Stop 0 (Format 3, 16 bytes):
        push_u16(&mut stat, 3);
        push_u16(&mut stat, 0); // axisIndex 0
        push_u16(&mut stat, 0x0002); // flags
        push_u16(&mut stat, 400); // "Book"
        push_fixed(&mut stat, 400.0);
        push_fixed(&mut stat, 700.0);
        // Stop 1 (Format 1, 12 bytes):
        push_u16(&mut stat, 1);
        push_u16(&mut stat, 0);
        push_u16(&mut stat, 0);
        push_u16(&mut stat, 401); // "Bold"
        push_fixed(&mut stat, 700.0);

        let sfnt = build_sfnt(
            SFNT_OTTO,
            &[(*b"name", name_bytes), (*b"fvar", fvar), (*b"STAT", stat)],
        );

        let info = inspect_variable_font(&sfnt).unwrap();
        assert_eq!(info.family_name.as_deref(), Some("Synthetic Pro Variable"));
        assert!(info.has_stat_table);
        assert_eq!(info.axes.len(), 5);
        assert_eq!(info.instances.len(), 2);

        let wght = &info.axes[0];
        assert_eq!(wght.tag_str(), "wght");
        assert_eq!(wght.known_tag, KnownAxisTag::Weight);
        assert_eq!(wght.name, "Weight");
        assert_eq!(
            (wght.min_value, wght.default_value, wght.max_value),
            (100.0, 400.0, 900.0)
        );
        assert!(!wght.is_hidden());
        assert_eq!(wght.stat_stops.len(), 2);
        assert_eq!(wght.stat_stops[0].label, "Book");
        assert_eq!(wght.stat_stops[0].linked_value, Some(700.0));
        assert_eq!(wght.named_instances.len(), 2);
        assert_eq!(wght.named_instances[1].name, "Bold Condensed");
        assert_eq!(
            wght.named_instances[1].postscript_name.as_deref(),
            Some("SyntheticPro-BoldCond")
        );
        assert_eq!(wght.named_instances[1].value, 700.0);
        assert_eq!(wght.normalize(400.0), 0.0);
        assert_eq!(wght.normalize(100.0), -1.0);
        assert_eq!(wght.normalize(900.0), 1.0);

        assert_eq!(info.axes[1].known_tag, KnownAxisTag::Width);
        assert_eq!(info.axes[2].known_tag, KnownAxisTag::OpticalSize);
        assert_eq!(info.axes[3].known_tag, KnownAxisTag::Slant);
        assert_eq!(info.axes[3].min_value, -12.0);
        assert_eq!(info.axes[4].known_tag, KnownAxisTag::Italic);
        assert!(info.axes[4].is_hidden());
    }

    #[test]
    fn rejects_malformed_truncated_and_static_inputs_without_panicking() {
        assert!(parse_variation_axes(b"").is_empty());
        assert!(parse_variation_axes(b"not a font file").is_empty());
        let static_sfnt = build_sfnt(
            SFNT_TRUETYPE,
            &[(*b"name", build_name_table(&[(1, "Static")]))],
        );
        assert!(parse_variation_axes(&static_sfnt).is_empty());

        // Truncated fvar header
        let bad_fvar = build_sfnt(SFNT_TRUETYPE, &[(*b"fvar", vec![0, 1, 0, 0, 0, 16])]);
        assert!(parse_variation_axes(&bad_fvar).is_empty());
    }
}
