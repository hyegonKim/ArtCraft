//! Bounded traversal of DYLD_CHAINED_PTR_64 / _64_OFFSET rebases. Bind slots
//! are never converted into runtime pointers. Unsupported formats remain unknown.
use super::{read_u16_le as u16le, read_u32_le as u32le, read_u64_le as u64le};
use crate::segment::Segment;
use std::collections::{BTreeMap, BTreeSet};
#[derive(Default)]
pub(super) struct Chained {
    pub rebases: BTreeMap<u64, u64>,
    pub binds: BTreeMap<u64, (String, u16)>,
}
fn imports(data: &[u8]) -> Result<Vec<Option<(String, u16)>>, String> {
    let error = || "Malformed chained import table".to_string();
    let count = u32le(data, 16).ok_or_else(error)? as usize;
    if count == 0 {
        return Ok(vec![]);
    }
    let stride = match u32le(data, 20).ok_or_else(error)? {
        1 => 4,
        2 => 8,
        3 => 16,
        _ => return Ok(vec![]),
    };
    if u32le(data, 24) != Some(0) {
        return Ok(vec![]);
    }
    let start = u32le(data, 8).ok_or_else(error)? as usize;
    let symbols = u32le(data, 12).ok_or_else(error)? as usize;
    if start < 28
        || symbols < 28
        || symbols > data.len()
        || count > data.len() / stride
        || start
            .checked_add(count * stride)
            .is_none_or(|end| end > data.len())
    {
        return Err(error());
    }
    let mut out = Vec::new();
    for n in 0..count {
        let at = start + n * stride;
        let (ordinal, weak, name, addend) = if stride == 16 {
            let word = u64le(data, at).ok_or_else(error)?;
            if (word >> 17) & 0x7fff != 0 {
                return Err(error());
            }
            (
                word & 65535,
                word & (1 << 16) != 0,
                word >> 32,
                u64le(data, at + 8).ok_or_else(error)?,
            )
        } else {
            let word = u32le(data, at).ok_or_else(error)? as u64;
            (
                word & 255,
                word & 256 != 0,
                word >> 9,
                if stride == 8 {
                    u32le(data, at + 4).ok_or_else(error)? as u64
                } else {
                    0
                },
            )
        };
        let at = symbols
            .checked_add(usize::try_from(name).map_err(|_| error())?)
            .ok_or_else(error)?;
        let tail = data.get(at..).ok_or_else(error)?;
        let end = tail.iter().position(|&b| b == 0).ok_or_else(error)?;
        let name = std::str::from_utf8(&tail[..end]).map_err(|_| error())?;
        let special = ordinal == 0
            || if stride == 16 {
                ordinal >= 0xfff1
            } else {
                ordinal >= 0xf1
            };
        out.push(if weak || addend != 0 || special || name.is_empty() {
            None
        } else {
            Some((name.to_string(), ordinal as u16))
        });
    }
    Ok(out)
}
#[cfg(test)]
fn rebases(data: &[u8], segments: &[Segment], base: u64) -> Result<BTreeMap<u64, u64>, String> {
    Ok(decode(data, segments, base)?.rebases)
}

pub(super) fn decode(data: &[u8], segments: &[Segment], base: u64) -> Result<Chained, String> {
    let fail = || "Malformed dyld chained fixups".to_string();
    if data.len() < 28 || u32le(data, 0) != Some(0) {
        return Err(fail());
    }
    let start = u32le(data, 4).ok_or_else(fail)? as usize;
    if start < 28 {
        return Err(fail());
    }
    let count = u32le(data, start).ok_or_else(fail)? as usize;
    if count != segments.len() || count > (data.len().saturating_sub(start + 4)) / 4 {
        return Err(fail());
    }
    let table_end = start + 4 + count * 4;
    let imports = imports(data)?;
    let import_count = u32le(data, 16).unwrap() as u64;
    let mut binds = BTreeMap::new();
    let mut result = BTreeMap::new();
    let mut visited = BTreeSet::new();
    let mut metadata = Vec::new();
    for (index, segment) in segments.iter().enumerate() {
        let rel = u32le(data, start + 4 + index * 4).ok_or_else(fail)? as usize;
        if rel == 0 {
            continue;
        }
        let at = start.checked_add(rel).ok_or_else(fail)?;
        let size = u32le(data, at).ok_or_else(fail)? as usize;
        let end = at
            .checked_add(size)
            .filter(|e| *e <= data.len())
            .ok_or_else(fail)?;
        if size < 22 || at < table_end || metadata.iter().any(|&(a, b)| at < b && end > a) {
            return Err(fail());
        }
        metadata.push((at, end));
        let page = u16le(data, at + 4).ok_or_else(fail)? as u64;
        let format = u16le(data, at + 6).ok_or_else(fail)?;
        let offset = u64le(data, at + 8).ok_or_else(fail)?;
        let pages = u16le(data, at + 20).ok_or_else(fail)? as usize;
        if !matches!(page, 4096 | 16384)
            || 22 + pages * 2 > size
            || base.checked_add(offset) != Some(segment.start_address)
        {
            return Err(fail());
        }
        if !matches!(format, 2 | 6) {
            continue;
        }
        for n in 0..pages {
            let first = u16le(data, at + 22 + 2 * n).ok_or_else(fail)? as u64;
            if first == 65535 {
                continue;
            }
            // MULTI chains are deliberately not implemented; no guessed decoding.
            if first & 0x8000 != 0 {
                continue;
            }
            let page_start = segment
                .start_address
                .checked_add((n as u64).checked_mul(page).ok_or_else(fail)?)
                .ok_or_else(fail)?;
            let page_end = page_start.checked_add(page).ok_or_else(fail)?;
            let mut address = page_start.checked_add(first).ok_or_else(fail)?;
            loop {
                if !address.is_multiple_of(4)
                    || address.checked_add(8).is_none_or(|a| a > page_end)
                    || !visited.insert(address)
                {
                    return Err(fail());
                }
                let word = segment.read_u64_le(address).ok_or_else(fail)?;
                let next = (word >> 51) & 4095;
                if word >> 63 == 0 {
                    if (word >> 44) & 127 != 0 {
                        return Err(fail());
                    }
                    let low = word & 0x0000000fffffffff;
                    let target = low | (((word >> 36) & 255) << 56);
                    let target = if format == 6 {
                        base.checked_add(target).ok_or_else(fail)?
                    } else {
                        target
                    };
                    result.insert(address, target);
                }
                if word >> 63 != 0 {
                    if (word >> 32) & 0x7ffff != 0 || word & 0xffffff >= import_count {
                        return Err(fail());
                    }
                    if (word >> 24) & 255 == 0 {
                        if let Some(Some(binding)) = imports.get((word & 0xffffff) as usize) {
                            binds.insert(address, binding.clone());
                        }
                    }
                }
                if next == 0 {
                    break;
                }
                address = address.checked_add(next * 4).ok_or_else(fail)?;
            }
        }
    }
    Ok(Chained {
        rebases: result,
        binds,
    })
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn import_formats_weak_addends_and_reserved_fields_are_conservative() {
        for (format, stride) in [(1u32, 4usize), (2, 8), (3, 16)] {
            let mut d = vec![0; 96];
            p32(&mut d, 8, 28);
            p32(&mut d, 12, 64);
            p32(&mut d, 16, 1);
            p32(&mut d, 20, format);
            d[64..66].copy_from_slice(b"f\0");
            p32(&mut d, 28, 2);
            assert_eq!(imports(&d).unwrap(), vec![Some(("f".into(), 2))]);
            let mut weak = d.clone();
            p32(
                &mut weak,
                28,
                2 | if stride == 16 { 1 << 16 } else { 1 << 8 },
            );
            assert_eq!(imports(&weak).unwrap(), vec![None]);
            if stride > 4 {
                let mut add = d.clone();
                p32(&mut add, if stride == 16 { 36 } else { 32 }, 1);
                assert_eq!(imports(&add).unwrap(), vec![None]);
            }
            if stride == 16 {
                p32(&mut d, 28, 1 << 17);
                assert!(imports(&d).is_err());
            }
        }
    }
    fn p16(b: &mut [u8], at: usize, n: u16) {
        b[at..at + 2].copy_from_slice(&n.to_le_bytes());
    }
    fn p32(b: &mut [u8], at: usize, n: u32) {
        b[at..at + 4].copy_from_slice(&n.to_le_bytes());
    }
    fn p64(b: &mut [u8], at: usize, n: u64) {
        b[at..at + 8].copy_from_slice(&n.to_le_bytes());
    }
    fn fixture() -> (Vec<u8>, Vec<Segment>) {
        let mut d = vec![0; 128];
        p32(&mut d, 4, 28);
        p32(&mut d, 8, 60);
        p32(&mut d, 12, 64);
        p32(&mut d, 16, 1);
        p32(&mut d, 20, 1);
        p32(&mut d, 60, 1);
        d[64..66].copy_from_slice(b"f\0");
        p32(&mut d, 28, 1);
        p32(&mut d, 32, 8);
        p32(&mut d, 36, 24);
        p16(&mut d, 40, 4096);
        p16(&mut d, 42, 6);
        p16(&mut d, 56, 1);
        p16(&mut d, 58, 0x80);
        let mut bytes = vec![0; 512];
        p64(&mut bytes, 0x80, (2 << 51) | 0x120);
        p64(&mut bytes, 0x88, (2 << 51) | (1 << 63));
        p64(&mut bytes, 0x90, 0x130);
        (d, vec![Segment::new("data", 0x100000000, 0, bytes)])
    }
    #[test]
    fn chain_membership_bind_bits_and_base_are_not_guessed() {
        let (d, s) = fixture();
        let map = rebases(&d, &s, 0x100000000).unwrap();
        assert_eq!(map.get(&0x100000080), Some(&0x100000120));
        assert_eq!(map.get(&0x100000090), Some(&0x100000130));
        assert!(!map.contains_key(&0x100000088));
        let mut absolute = d.clone();
        p16(&mut absolute, 42, 2);
        assert_eq!(
            rebases(&absolute, &s, 0x100000000)
                .unwrap()
                .get(&0x100000080),
            Some(&0x120)
        );
        let mut unknown = d;
        p16(&mut unknown, 42, 12);
        assert!(rebases(&unknown, &s, 0x100000000).unwrap().is_empty());
    }
    #[test]
    fn malformed_headers_bounds_steps_and_reserved_bits_fail() {
        let (d, s) = fixture();
        for (at, n) in [(4, u32::MAX), (28, 100), (32, 4), (36, 22)] {
            let mut bad = d.clone();
            p32(&mut bad, at, n);
            assert!(rebases(&bad, &s, 0x100000000).is_err());
        }
        for (at, n) in [(40, 123), (56, 100), (58, 4092)] {
            let mut bad = d.clone();
            p16(&mut bad, at, n);
            assert!(rebases(&bad, &s, 0x100000000).is_err());
        }
        for word in [(1u64 << 44) | 0x120, (4095u64 << 51) | 0x120] {
            let mut bad = s.clone();
            p64(&mut bad[0].data, 0x80, word);
            assert!(rebases(&d, &bad, 0x100000000).is_err());
        }
    }
}
