use crate::{Error, Limits, Result};

/// Four-byte magic signature at offset `0..4` of every Adobe Swatch Exchange (`.ase`) file.
pub const ASE_MAGIC: [u8; 4] = *b"ASEF";
/// Canonical `(major, minor)` version `(1, 0)` written into the `ASEF` header.
pub const ASE_VERSION: (u16, u16) = (1, 0);

/// `ASEF` block type for Color Group Start (`0xC001`).
pub const BLOCK_GROUP_START: u16 = 0xC001;
/// `ASEF` block type for Color Group End (`0xC002`).
pub const BLOCK_GROUP_END: u16 = 0xC002;
/// `ASEF` block type for a Color Swatch Entry (`0x0001`).
pub const BLOCK_COLOR_ENTRY: u16 = 0x0001;

/// Quick prefix check for `ASEF` magic header bytes.
pub fn is_ase_bytes(bytes: &[u8]) -> bool {
    bytes.starts_with(&ASE_MAGIC)
}

/// Color model and channel values of an `.ase` swatch entry.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AseColor {
    /// `RGB ` — normalized `[0.0, 1.0]` red, green, blue channels.
    Rgb { r: f32, g: f32, b: f32 },
    /// `CMYK` — normalized `[0.0, 1.0]` cyan, magenta, yellow, black ink channels.
    Cmyk { c: f32, m: f32, y: f32, k: f32 },
    /// `LAB ` — `ASEF` CIE Lab (`l` in `0.0..=1.0` where `1.0` = `L* 100`, `a` and `b` in `-128.0..=127.0`).
    Lab { l: f32, a: f32, b: f32 },
    /// `Gray` — normalized `[0.0, 1.0]` grayscale value.
    Gray { value: f32 },
}

impl AseColor {
    /// Four-byte ASCII color model tag used on the wire in `ASEF` v1.0.
    pub const fn model_tag(self) -> [u8; 4] {
        match self {
            Self::Rgb { .. } => *b"RGB ",
            Self::Cmyk { .. } => *b"CMYK",
            Self::Lab { .. } => *b"LAB ",
            Self::Gray { .. } => *b"Gray",
        }
    }

    /// Whether every channel float in this color is finite (`!NaN && !Infinite`).
    pub fn is_finite(self) -> bool {
        match self {
            Self::Rgb { r, g, b } => r.is_finite() && g.is_finite() && b.is_finite(),
            Self::Cmyk { c, m, y, k } => c.is_finite() && m.is_finite() && y.is_finite() && k.is_finite(),
            Self::Lab { l, a, b } => l.is_finite() && a.is_finite() && b.is_finite(),
            Self::Gray { value } => value.is_finite(),
        }
    }

    /// Approximate 8-bit sRGB preview `[r, g, b]` for UI swatch rendering across all 4 color models.
    pub fn to_srgb8(self) -> [u8; 3] {
        let to_u8 = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
        match self {
            Self::Rgb { r, g, b } => [to_u8(r), to_u8(g), to_u8(b)],
            Self::Gray { value } => {
                let g = to_u8(value);
                [g, g, g]
            }
            Self::Cmyk { c, m, y, k } => {
                let inv_k = 1.0 - k.clamp(0.0, 1.0);
                [to_u8((1.0 - c.clamp(0.0, 1.0)) * inv_k), to_u8((1.0 - m.clamp(0.0, 1.0)) * inv_k), to_u8((1.0 - y.clamp(0.0, 1.0)) * inv_k)]
            }
            Self::Lab { l, a, b } => lab_to_srgb8(l * 100.0, a, b),
        }
    }
}

fn lab_to_srgb8(l_star: f32, a_star: f32, b_star: f32) -> [u8; 3] {
    let fy = (l_star + 16.0) / 116.0;
    let fx = a_star / 500.0 + fy;
    let fz = fy - b_star / 200.0;
    let inv_f = |t: f32| {
        let delta = 6.0 / 29.0;
        if t > delta { t * t * t } else { 3.0 * delta * delta * (t - 4.0 / 29.0) }
    };
    // D65 white point -> linear sRGB
    let x = 0.950_47 * inv_f(fx);
    let y = 1.000_00 * inv_f(fy);
    let z = 1.088_83 * inv_f(fz);
    let r_lin = 3.240_454_2 * x - 1.537_138_5 * y - 0.498_531_4 * z;
    let g_lin = -0.969_266 * x + 1.876_010_8 * y + 0.041_556 * z;
    let b_lin = 0.055_643_4 * x - 0.204_025_9 * y + 1.057_225_2 * z;
    let gamma = |u: f32| {
        let u = u.clamp(0.0, 1.0);
        let s = if u <= 0.003_130_8 { 12.92 * u } else { 1.055 * u.powf(1.0 / 2.4) - 0.055 };
        (s.clamp(0.0, 1.0) * 255.0).round() as u8
    };
    [gamma(r_lin), gamma(g_lin), gamma(b_lin)]
}

/// Swatch ink behavior in `ASEF` (`0 = Global`, `1 = Spot`, `2 = Process`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum AseColorType {
    /// `0` — Global swatch (linked across document objects).
    Global,
    /// `1` — Spot ink swatch.
    Spot,
    /// `2` — Standard process color swatch.
    #[default]
    Process,
}

impl AseColorType {
    /// Wire `u16` code in big-endian `ASEF` color blocks.
    pub const fn to_u16(self) -> u16 {
        match self {
            Self::Global => 0,
            Self::Spot => 1,
            Self::Process => 2,
        }
    }

    /// Parse wire `u16` code from an `ASEF` color block.
    pub const fn from_u16(code: u16) -> Option<Self> {
        match code {
            0 => Some(Self::Global),
            1 => Some(Self::Spot),
            2 => Some(Self::Process),
            _ => None,
        }
    }
}

/// A single named color swatch entry.
#[derive(Clone, Debug, PartialEq)]
pub struct AseSwatch {
    /// Swatch display name (decoded from UTF-16BE).
    pub name: String,
    /// Color space and channel values.
    pub color: AseColor,
    /// Global, Spot, or Process swatch classification.
    pub color_type: AseColorType,
}

/// A named Color Group (`0xC001` .. `0xC002`) containing zero or more [`AseSwatch`] entries.
#[derive(Clone, Debug, PartialEq)]
pub struct AseColorGroup {
    /// Color group display name (decoded from UTF-16BE).
    pub name: String,
    /// Swatches inside this group, in file order.
    pub swatches: Vec<AseSwatch>,
}

/// Top-level item inside an [`AsePalette`]: either an ungrouped swatch or a named color group.
#[derive(Clone, Debug, PartialEq)]
pub enum AseItem {
    /// Top-level swatch entry (`0x0001`).
    Swatch(AseSwatch),
    /// Named color group (`0xC001` .. `0xC002`).
    Group(AseColorGroup),
}

/// Complete Adobe Swatch Exchange (`.ase`) palette (`ASEF` v1.0).
#[derive(Clone, Debug, PartialEq)]
pub struct AsePalette {
    /// `(major, minor)` file version, normally `(1, 0)`.
    pub version: (u16, u16),
    /// Ordered top-level swatches and color groups.
    pub items: Vec<AseItem>,
}

impl Default for AsePalette {
    fn default() -> Self {
        Self::new()
    }
}

impl AsePalette {
    /// Create an empty `ASEF` v1.0 palette.
    pub const fn new() -> Self {
        Self { version: ASE_VERSION, items: Vec::new() }
    }

    /// Append a top-level swatch.
    pub fn push_swatch(&mut self, swatch: AseSwatch) {
        self.items.push(AseItem::Swatch(swatch));
    }

    /// Append a named color group.
    pub fn push_group(&mut self, group: AseColorGroup) {
        self.items.push(AseItem::Group(group));
    }

    /// Iterate over every swatch in the palette (both top-level and inside groups), in order.
    pub fn all_swatches(&self) -> impl Iterator<Item = &AseSwatch> {
        self.items.iter().flat_map(|item| match item {
            AseItem::Swatch(s) => std::slice::from_ref(s).iter(),
            AseItem::Group(g) => g.swatches.iter(),
        })
    }

    /// Iterate over all color groups in the palette.
    pub fn groups(&self) -> impl Iterator<Item = &AseColorGroup> {
        self.items.iter().filter_map(|item| match item {
            AseItem::Group(g) => Some(g),
            AseItem::Swatch(_) => None,
        })
    }

    /// Total number of `ASEF` blocks this palette encodes (`1` per swatch, `2 + swatches.len()` per group).
    pub fn block_count(&self) -> Result<u32> {
        let mut total = 0u32;
        for item in &self.items {
            let add = match item {
                AseItem::Swatch(_) => 1u32,
                AseItem::Group(g) => {
                    let inner = u32::try_from(g.swatches.len()).map_err(|_| Error::InvalidAse("too many swatches in group".into()))?;
                    inner.checked_add(2).ok_or_else(|| Error::InvalidAse("ASE block count overflow".into()))?
                }
            };
            total = total.checked_add(add).ok_or_else(|| Error::InvalidAse("ASE block count overflow".into()))?;
        }
        Ok(total)
    }

    /// Decode an `.ase` binary file using [`Limits::default`].
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        Self::from_bytes_with_limits(bytes, Limits::default())
    }

    /// Decode an `.ase` binary file enforcing explicit [`Limits`].
    pub fn from_bytes_with_limits(bytes: &[u8], limits: Limits) -> Result<Self> {
        let mut cursor = ByteCursor::new(bytes);
        let magic = cursor.read_array::<4>()?;
        if magic != ASE_MAGIC {
            return Err(Error::InvalidAse("missing ASEF signature".into()));
        }
        let major = cursor.read_u16_be()?;
        let minor = cursor.read_u16_be()?;
        if major != 1 {
            return Err(Error::InvalidAse(format!("unsupported ASEF major version {major}.{minor}")));
        }
        let num_blocks = cursor.read_u32_be()?;
        if num_blocks > limits.max_ase_blocks {
            return Err(Error::LimitExceeded { what: "ASE block count", limit: u64::from(limits.max_ase_blocks) });
        }

        let mut items = Vec::new();
        let mut open_group: Option<AseColorGroup> = None;

        for _ in 0..num_blocks {
            let block_type = cursor.read_u16_be()?;
            let block_len = usize::try_from(cursor.read_u32_be()?).map_err(|_| Error::InvalidAse("block length overflow".into()))?;
            let payload = cursor.read_slice(block_len)?;
            parse_ase_block(block_type, payload, &mut items, &mut open_group)?;
        }

        if open_group.is_some() {
            return Err(Error::InvalidAse("unclosed 0xC001 color group block at end of file".into()));
        }
        if !cursor.is_empty() {
            return Err(Error::InvalidAse("trailing bytes after declared ASEF blocks".into()));
        }
        Ok(Self { version: (major, minor), items })
    }

    /// Encode this palette into canonical big-endian `ASEF` v1.0 binary bytes.
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        let count = self.block_count()?;
        let mut out = Vec::with_capacity(12 + (count as usize).saturating_mul(32));
        out.extend_from_slice(&ASE_MAGIC);
        out.extend_from_slice(&self.version.0.to_be_bytes());
        out.extend_from_slice(&self.version.1.to_be_bytes());
        out.extend_from_slice(&count.to_be_bytes());

        for item in &self.items {
            match item {
                AseItem::Swatch(swatch) => write_swatch_block(&mut out, swatch)?,
                AseItem::Group(group) => {
                    write_group_start_block(&mut out, &group.name)?;
                    for swatch in &group.swatches {
                        write_swatch_block(&mut out, swatch)?;
                    }
                    out.extend_from_slice(&BLOCK_GROUP_END.to_be_bytes());
                    out.extend_from_slice(&0u32.to_be_bytes());
                }
            }
        }
        Ok(out)
    }
}

fn parse_ase_block(block_type: u16, payload: &[u8], items: &mut Vec<AseItem>, open_group: &mut Option<AseColorGroup>) -> Result<()> {
    match block_type {
        BLOCK_GROUP_START => {
            if open_group.is_some() {
                return Err(Error::InvalidAse("nested 0xC001 color groups are not valid in ASEF".into()));
            }
            let mut sub = ByteCursor::new(payload);
            let name = sub.read_utf16_name()?;
            *open_group = Some(AseColorGroup { name, swatches: Vec::new() });
            Ok(())
        }
        BLOCK_GROUP_END => {
            let group = open_group.take().ok_or_else(|| Error::InvalidAse("unmatched 0xC002 color group end block".into()))?;
            items.push(AseItem::Group(group));
            Ok(())
        }
        BLOCK_COLOR_ENTRY => {
            let swatch = parse_swatch_payload(payload)?;
            if let Some(group) = open_group.as_mut() {
                group.swatches.push(swatch);
            } else {
                items.push(AseItem::Swatch(swatch));
            }
            Ok(())
        }
        other => Err(Error::InvalidAse(format!("unknown ASEF block type 0x{other:04X}"))),
    }
}

fn parse_swatch_payload(payload: &[u8]) -> Result<AseSwatch> {
    let mut sub = ByteCursor::new(payload);
    let name = sub.read_utf16_name()?;
    let model = sub.read_array::<4>()?;
    let color = match &model {
        b"RGB " => AseColor::Rgb { r: sub.read_f32_be()?, g: sub.read_f32_be()?, b: sub.read_f32_be()? },
        b"CMYK" => AseColor::Cmyk { c: sub.read_f32_be()?, m: sub.read_f32_be()?, y: sub.read_f32_be()?, k: sub.read_f32_be()? },
        b"LAB " => AseColor::Lab { l: sub.read_f32_be()?, a: sub.read_f32_be()?, b: sub.read_f32_be()? },
        b"Gray" => AseColor::Gray { value: sub.read_f32_be()? },
        _ => return Err(Error::InvalidAse(format!("unknown ASEF color model {:?}", String::from_utf8_lossy(&model)))),
    };
    let type_code = sub.read_u16_be()?;
    let color_type = AseColorType::from_u16(type_code).ok_or_else(|| Error::InvalidAse(format!("unknown ASEF color type {type_code}")))?;
    if !sub.is_empty() {
        return Err(Error::InvalidAse("unexpected trailing bytes in color entry block".into()));
    }
    Ok(AseSwatch { name, color, color_type })
}

fn encode_utf16_name(out: &mut Vec<u8>, name: &str) -> Result<()> {
    let units: Vec<u16> = name.encode_utf16().chain(std::iter::once(0u16)).collect();
    let count = u16::try_from(units.len()).map_err(|_| Error::InvalidAse("swatch/group name exceeds u16 UTF-16 length".into()))?;
    out.extend_from_slice(&count.to_be_bytes());
    for unit in units {
        out.extend_from_slice(&unit.to_be_bytes());
    }
    Ok(())
}

fn write_group_start_block(out: &mut Vec<u8>, name: &str) -> Result<()> {
    let mut payload = Vec::new();
    encode_utf16_name(&mut payload, name)?;
    let len = u32::try_from(payload.len()).map_err(|_| Error::InvalidAse("group block length overflow".into()))?;
    out.extend_from_slice(&BLOCK_GROUP_START.to_be_bytes());
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(&payload);
    Ok(())
}

fn write_swatch_block(out: &mut Vec<u8>, swatch: &AseSwatch) -> Result<()> {
    if !swatch.color.is_finite() {
        return Err(Error::InvalidAse("non-finite float channel in swatch color".into()));
    }
    let mut payload = Vec::with_capacity(32);
    encode_utf16_name(&mut payload, &swatch.name)?;
    payload.extend_from_slice(&swatch.color.model_tag());
    match swatch.color {
        AseColor::Rgb { r, g, b } => {
            for ch in [r, g, b] {
                payload.extend_from_slice(&ch.to_be_bytes());
            }
        }
        AseColor::Cmyk { c, m, y, k } => {
            for ch in [c, m, y, k] {
                payload.extend_from_slice(&ch.to_be_bytes());
            }
        }
        AseColor::Lab { l, a, b } => {
            for ch in [l, a, b] {
                payload.extend_from_slice(&ch.to_be_bytes());
            }
        }
        AseColor::Gray { value } => {
            payload.extend_from_slice(&value.to_be_bytes());
        }
    }
    payload.extend_from_slice(&swatch.color_type.to_u16().to_be_bytes());
    let len = u32::try_from(payload.len()).map_err(|_| Error::InvalidAse("swatch block length overflow".into()))?;
    out.extend_from_slice(&BLOCK_COLOR_ENTRY.to_be_bytes());
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(&payload);
    Ok(())
}

struct ByteCursor<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> ByteCursor<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, pos: 0 }
    }

    const fn is_empty(&self) -> bool {
        self.pos >= self.bytes.len()
    }

    fn read_slice(&mut self, len: usize) -> Result<&'a [u8]> {
        let end = self.pos.checked_add(len).ok_or_else(|| Error::InvalidAse("slice offset overflow".into()))?;
        let slice = self.bytes.get(self.pos..end).ok_or_else(|| Error::InvalidAse("unexpected end of ASEF stream".into()))?;
        self.pos = end;
        Ok(slice)
    }

    fn read_array<const N: usize>(&mut self) -> Result<[u8; N]> {
        let slice = self.read_slice(N)?;
        <[u8; N]>::try_from(slice).map_err(|_| Error::InvalidAse("unexpected end of ASEF stream".into()))
    }

    fn read_u16_be(&mut self) -> Result<u16> {
        Ok(u16::from_be_bytes(self.read_array::<2>()?))
    }

    fn read_u32_be(&mut self) -> Result<u32> {
        Ok(u32::from_be_bytes(self.read_array::<4>()?))
    }

    fn read_f32_be(&mut self) -> Result<f32> {
        let val = f32::from_be_bytes(self.read_array::<4>()?);
        if !val.is_finite() {
            return Err(Error::InvalidAse("non-finite IEEE-754 float in ASEF color".into()));
        }
        Ok(val)
    }

    fn read_utf16_name(&mut self) -> Result<String> {
        let unit_count = usize::from(self.read_u16_be()?);
        if unit_count == 0 {
            return Ok(String::new());
        }
        let byte_len = unit_count.checked_mul(2).ok_or_else(|| Error::InvalidAse("UTF-16 length overflow".into()))?;
        let raw = self.read_slice(byte_len)?;
        let mut units: Vec<u16> = raw.as_chunks::<2>().0.iter().copied().map(u16::from_be_bytes).collect();
        if units.last() == Some(&0) {
            units.pop();
        }
        String::from_utf16(&units).map_err(|_| Error::InvalidAse("invalid UTF-16BE string in ASEF block".into()))
    }
}
