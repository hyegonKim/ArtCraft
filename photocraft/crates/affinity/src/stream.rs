//! The tagged object stream inside `doc.dat`: a self-describing tree of objects whose fields carry
//! a type code and a four-character tag. Parsing needs no schema; unknown fields are kept, so the
//! document mapping can name what it could not import.

use crate::Error;
use crate::container::Cursor;

/// A four-character tag in its readable order (`Desc`, `Chld`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Tag(pub u32);

impl Tag {
    pub const fn of(s: &[u8; 4]) -> Self {
        Self(u32::from_be_bytes(*s))
    }
}

impl std::fmt::Debug for Tag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", String::from_utf8_lossy(&self.0.to_be_bytes()))
    }
}

impl std::fmt::Display for Tag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(self, f)
    }
}

/// Index of an object in [`Stream::objects`].
pub type ObjId = usize;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Int(i64),
    UInt(u64),
    Float(f64),
    Bool(bool),
    Enum {
        id: u16,
        version: u16,
    },
    Str(String),
    /// Fixed-length vectors of i32/f32/f64 (points, rectangles, transforms).
    Ints(Vec<i64>),
    Floats(Vec<f64>),
    /// Fixed-size record (curve nodes, ids) or fixed-size struct (colour components).
    Bytes(Vec<u8>),
    Blob(Vec<u8>),
    /// Name of another archive entry (raster tiles, embedded files).
    Entry(String),
    Flags(u64),
    /// `None` is a null object.
    Obj(Option<ObjId>),
    Array(Vec<Value>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Root,
    /// Defined once and referenced by id elsewhere.
    Shared,
    Inline,
    /// Untagged fields identified by position (curve data).
    Positional,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Object {
    pub class: Tag,
    pub kind: Kind,
    /// Class chain, most derived first (`ShpN`, `VNod`, `Node`).
    pub chain: Vec<Tag>,
    /// Fields in stream order; positional fields have tag 0.
    pub fields: Vec<(Tag, Value)>,
    /// First-occurrence index of each non-positional tag, so `get` is O(1). A shared object
    /// can be referenced by many sibling layers, and each reference used to rescan all of its
    /// fields; indexing once keeps repeated lookups from multiplying into quadratic work.
    index: std::collections::HashMap<Tag, usize>,
}

impl Object {
    pub fn new(class: Tag, kind: Kind, chain: Vec<Tag>, fields: Vec<(Tag, Value)>) -> Self {
        let mut index = std::collections::HashMap::with_capacity(fields.len());
        for (i, (t, _)) in fields.iter().enumerate() {
            // Positional fields share tag 0 and are not looked up by tag; keep first-match
            // semantics for real tags.
            if t.0 != 0 {
                index.entry(*t).or_insert(i);
            }
        }
        Object { class, kind, chain, fields, index }
    }

    pub fn get(&self, tag: Tag) -> Option<&Value> {
        self.index.get(&tag).map(|&i| &self.fields[i].1)
    }

    /// Replaces the fields (the root object is filled in after it is pushed) and rebuilds the
    /// tag index so `get` stays correct.
    fn set_fields(&mut self, fields: Vec<(Tag, Value)>) {
        self.index.clear();
        self.index.reserve(fields.len());
        for (i, (t, _)) in fields.iter().enumerate() {
            if t.0 != 0 {
                self.index.entry(*t).or_insert(i);
            }
        }
        self.fields = fields;
    }
}

#[derive(Debug)]
pub struct Stream {
    pub root: ObjId,
    pub objects: Vec<Object>,
}

/// Deepest object nesting accepted; real documents stay well below (each layer level adds three).
pub const MAX_DEPTH: usize = 384;

const MAX_OBJECTS: usize = 8_000_000;

/// Most values (fields and array elements) one stream may decode. A value takes about 40 bytes
/// in memory however few it took in the file (a bool array packs eight in a byte), so the size
/// of the input alone doesn't bound what parsing allocates: a small compressed entry could
/// otherwise ask for tens of gigabytes. Real documents stay far below.
pub const MAX_VALUES: usize = 1 << 24;

struct Parser<'a> {
    c: Cursor<'a>,
    objects: Vec<Object>,
    /// Shared ids, defined at first use and referenced backwards.
    shared: std::collections::HashMap<u32, ObjId>,
    depth: usize,
    /// Values decoded so far, against [`MAX_VALUES`].
    values: usize,
}

// Pending operations are explicit, so supported nesting never consumes native call frames.
// At most four continuations per active field list are needed; arrays schedule one element
// at a time. Parsed objects reference their children by ObjId, keeping error cleanup flat too.
const MAX_WORK: usize = MAX_DEPTH * 4 + 8;

enum Work {
    Fields { tagged: bool, class: Tag, fields: Vec<(Tag, Value)> },
    FieldValue { tagged: bool, class: Tag, fields: Vec<(Tag, Value)>, tag: Tag },
    Value { code: u8, tagged: bool, class: Tag, inline_class: Option<Tag> },
    Object { class: Tag, kind: Kind },
    Shared { id: ObjId, chain: Vec<Tag>, fields: Vec<(Tag, Value)> },
    SharedFields { id: ObjId, chain: Vec<Tag>, fields: Vec<(Tag, Value)>, final_fields: bool },
    Array { code: u8, class: Tag, inline_class: Option<Tag>, left: usize, values: Vec<Value> },
    ArrayValue { code: u8, class: Tag, inline_class: Option<Tag>, left: usize, values: Vec<Value> },
}

enum Produced {
    Value(Value),
    Fields(Vec<(Tag, Value)>),
}

/// Parse a stream (`00 FF 4B 53` header). Bytes after the root's terminator are ignored.
pub fn parse(bytes: &[u8]) -> Result<Stream, Error> {
    let mut c = Cursor::new(bytes, 0);
    if c.take(4)? != b"\x00\xffKS" {
        return Err(Error::Malformed("document stream signature"));
    }
    let version = c.u16()?;
    if version > 2 {
        return Err(Error::Unsupported("document stream version"));
    }
    let class = Tag(c.u32()?);
    c.u16()?;
    if version == 2 {
        c.u32()?;
    }
    let mut p = Parser { c, objects: Vec::new(), shared: std::collections::HashMap::new(), depth: 0, values: 0 };
    let root = p.push(Object::new(class, Kind::Root, vec![class], Vec::new()))?;
    let fields = p.fields(true, class)?;
    if let Some(o) = p.objects.get_mut(root) {
        o.set_fields(fields);
    }
    Ok(Stream { root, objects: p.objects })
}

impl Parser<'_> {
    fn push(&mut self, o: Object) -> Result<ObjId, Error> {
        if self.objects.len() >= MAX_OBJECTS {
            return Err(Error::Limit("too many objects"));
        }
        self.objects.push(o);
        Ok(self.objects.len() - 1)
    }

    /// Counts `n` more values against [`MAX_VALUES`], before anything is allocated for them.
    fn spend(&mut self, n: usize) -> Result<(), Error> {
        self.values = self.values.checked_add(n).filter(|&v| v <= MAX_VALUES).ok_or(Error::Limit("too many values in the document stream"))?;
        Ok(())
    }

    /// An empty array with room for `n` values, paid for first; an allocation that still fails
    /// is an error, not an abort.
    fn vec<T>(&mut self, n: usize) -> Result<Vec<T>, Error> {
        self.spend(n)?;
        let mut v = Vec::new();
        v.try_reserve_exact(n).map_err(|_| Error::Limit("not enough memory for the document stream"))?;
        Ok(v)
    }

    /// Element count of an array: each element takes at least `min` bytes, so a count the
    /// remaining input cannot hold is rejected before anything is allocated.
    fn count(&mut self, min: usize) -> Result<usize, Error> {
        let n = usize::try_from(self.c.u32()?).map_err(|_| Error::Malformed("array length"))?;
        if n.checked_mul(min.max(1)).is_none_or(|bytes| bytes > self.c.remaining()) {
            return Err(Error::Malformed("array longer than its data"));
        }
        Ok(n)
    }

    fn fields(&mut self, tagged: bool, class: Tag) -> Result<Vec<(Tag, Value)>, Error> {
        let mut work = Vec::new();
        work.try_reserve_exact(MAX_WORK).map_err(|_| Error::Limit("not enough memory for the document stream"))?;
        self.begin_fields(&mut work, tagged, class)?;
        let mut produced = None;
        while let Some(next) = work.pop() {
            match next {
                Work::Fields { tagged, class, fields } => {
                    let t = self.c.u8()?;
                    let code = t & 0x7f;
                    if code == 0 {
                        self.depth -= 1;
                        produced = Some(Produced::Fields(fields));
                    } else {
                        self.spend(1)?;
                        let tag = if tagged { Tag(self.c.u32()?) } else { Tag(0) };
                        Self::schedule(&mut work, Work::FieldValue { tagged, class, fields, tag })?;
                        if t & 0x80 != 0 && matches!(code, 0x30..=0x32) {
                            let left = self.count(1)?;
                            let inline_class = if code == 0x32 {
                                let class = Tag(self.c.u32()?);
                                self.c.u16()?;
                                Some(class)
                            } else {
                                None
                            };
                            let values = self.vec(left)?;
                            Self::schedule(&mut work, Work::Array { code, class, inline_class, left, values })?;
                        } else if t & 0x80 != 0 {
                            produced = Some(Produced::Value(self.array(code)?));
                        } else {
                            Self::schedule(&mut work, Work::Value { code, tagged, class, inline_class: None })?;
                        }
                    }
                }
                Work::FieldValue { tagged, class, mut fields, tag } => {
                    fields.push((tag, Self::take_value(&mut produced)?));
                    Self::schedule(&mut work, Work::Fields { tagged, class, fields })?;
                }
                Work::Value { code, tagged, class, inline_class } => match code {
                    0x30 => {
                        Self::schedule(&mut work, Work::Object { class, kind: Kind::Positional })?;
                        self.begin_fields(&mut work, false, class)?;
                    }
                    0x31 | 0x32 if !tagged => return Err(Error::Malformed("object without a field tag")),
                    0x31 => {
                        let flag = self.c.u8()?;
                        if flag == 0 {
                            produced = Some(Produced::Value(Value::Obj(None)));
                            continue;
                        }
                        let key = self.c.u32()?;
                        match flag {
                            2 => {
                                let id = self.shared.get(&key).copied().ok_or(Error::Malformed("reference to an undefined object"))?;
                                produced = Some(Produced::Value(Value::Obj(Some(id))));
                            }
                            1 => {
                                if self.shared.contains_key(&key) {
                                    return Err(Error::Malformed("object defined twice"));
                                }
                                // Register before fields, so self-references keep the same meaning.
                                let id = self.push(Object::new(Tag(0), Kind::Shared, Vec::new(), Vec::new()))?;
                                self.shared.insert(key, id);
                                Self::schedule(&mut work, Work::Shared { id, chain: Vec::new(), fields: Vec::new() })?;
                            }
                            _ => return Err(Error::Malformed("object marker")),
                        }
                    }
                    0x32 => {
                        match self.c.u8()? {
                            0 => {
                                produced = Some(Produced::Value(Value::Obj(None)));
                                continue;
                            }
                            1 => {}
                            _ => return Err(Error::Malformed("object marker")),
                        }
                        let class = match inline_class {
                            Some(class) => class,
                            None => {
                                let class = Tag(self.c.u32()?);
                                self.c.u16()?;
                                class
                            }
                        };
                        Self::schedule(&mut work, Work::Object { class, kind: Kind::Inline })?;
                        self.begin_fields(&mut work, true, class)?;
                    }
                    _ => produced = Some(Produced::Value(self.scalar(code)?)),
                },
                Work::Object { class, kind } => {
                    let fields = Self::take_fields(&mut produced)?;
                    let chain = if kind == Kind::Inline { vec![class] } else { Vec::new() };
                    let id = self.push(Object::new(class, kind, chain, fields))?;
                    produced = Some(Produced::Value(Value::Obj(Some(id))));
                }
                Work::Shared { id, mut chain, fields } => {
                    let final_fields = match self.c.u8()? {
                        0 => {
                            let tag = Tag(self.c.u32()?);
                            self.c.u16()?;
                            chain.push(tag);
                            Self::schedule(&mut work, Work::SharedFields { id, chain, fields, final_fields: false })?;
                            self.begin_fields(&mut work, true, tag)?;
                            continue;
                        }
                        1 => {
                            chain.push(Tag(self.c.u32()?));
                            true
                        }
                        2 => true,
                        _ => return Err(Error::Malformed("object class chain")),
                    };
                    let class = chain.first().copied().unwrap_or(Tag(0));
                    Self::schedule(&mut work, Work::SharedFields { id, chain, fields, final_fields })?;
                    self.begin_fields(&mut work, true, class)?;
                }
                Work::SharedFields { id, chain, mut fields, final_fields } => {
                    fields.extend(Self::take_fields(&mut produced)?);
                    if final_fields {
                        let class = chain.first().copied().unwrap_or(Tag(0));
                        if let Some(object) = self.objects.get_mut(id) {
                            *object = Object::new(class, Kind::Shared, chain, fields);
                        }
                        produced = Some(Produced::Value(Value::Obj(Some(id))));
                    } else {
                        if chain.len() > 64 {
                            return Err(Error::Limit("object class chain too long"));
                        }
                        Self::schedule(&mut work, Work::Shared { id, chain, fields })?;
                    }
                }
                Work::Array { code, class, inline_class, left, values } => {
                    if left == 0 {
                        produced = Some(Produced::Value(Value::Array(values)));
                    } else {
                        Self::schedule(&mut work, Work::ArrayValue { code, class, inline_class, left: left - 1, values })?;
                        Self::schedule(&mut work, Work::Value { code, tagged: true, class, inline_class })?;
                    }
                }
                Work::ArrayValue { code, class, inline_class, left, mut values } => {
                    values.push(Self::take_value(&mut produced)?);
                    Self::schedule(&mut work, Work::Array { code, class, inline_class, left, values })?;
                }
            }
        }
        Self::take_fields(&mut produced)
    }

    fn begin_fields(&mut self, work: &mut Vec<Work>, tagged: bool, class: Tag) -> Result<(), Error> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return Err(Error::Limit("objects nested too deeply"));
        }
        Self::schedule(work, Work::Fields { tagged, class, fields: Vec::new() })
    }

    fn schedule(work: &mut Vec<Work>, next: Work) -> Result<(), Error> {
        if work.len() >= MAX_WORK {
            return Err(Error::Limit("objects nested too deeply"));
        }
        // Capacity for the bounded work stack was reserved fallibly before parsing.
        work.push(next);
        Ok(())
    }

    fn take_value(produced: &mut Option<Produced>) -> Result<Value, Error> {
        match produced.take() {
            Some(Produced::Value(value)) => Ok(value),
            _ => Err(Error::Malformed("document stream decoder state")),
        }
    }

    fn take_fields(produced: &mut Option<Produced>) -> Result<Vec<(Tag, Value)>, Error> {
        match produced.take() {
            Some(Produced::Fields(fields)) => Ok(fields),
            _ => Err(Error::Malformed("document stream decoder state")),
        }
    }

    fn string(&mut self) -> Result<String, Error> {
        let n = usize::try_from(self.c.u32()?).map_err(|_| Error::Malformed("string length"))?;
        Ok(String::from_utf8_lossy(self.c.take(n)?).into_owned())
    }

    fn floats(&mut self, n: usize, wide: bool) -> Result<Value, Error> {
        let mut v = Vec::with_capacity(n);
        for _ in 0..n {
            v.push(if wide { f64::from_le_bytes(self.c.array()?) } else { f64::from(f32::from_le_bytes(self.c.array()?)) });
        }
        Ok(Value::Floats(v))
    }

    fn scalar(&mut self, code: u8) -> Result<Value, Error> {
        let c = &mut self.c;
        Ok(match code {
            0x01 => Value::UInt(c.u8()?.into()),
            0x02 => Value::UInt(c.u16()?.into()),
            0x03 | 0x2f | 0x34 => Value::UInt(c.u32()?.into()),
            0x04 => Value::UInt(c.u64()?),
            0x05 => Value::Int(i8::from_le_bytes(c.array()?).into()),
            0x06 => Value::Int(i16::from_le_bytes(c.array()?).into()),
            0x07 => Value::Int(i32::from_le_bytes(c.array()?).into()),
            0x08 => Value::Int(i64::from_le_bytes(c.array()?)),
            0x09 => Value::Float(f32::from_le_bytes(c.array()?).into()),
            0x0a => Value::Float(f64::from_le_bytes(c.array()?)),
            0x15..=0x19 => {
                let n = usize::from(code - 0x13);
                let mut v = Vec::with_capacity(n);
                for _ in 0..n {
                    v.push(i32::from_le_bytes(c.array()?).into());
                }
                Value::Ints(v)
            }
            0x1f..=0x23 => return self.floats(usize::from(code - 0x1d), false),
            0x24..=0x28 => return self.floats(usize::from(code - 0x22), true),
            0x29 => Value::Bool(c.u8()? != 0),
            0x2a => Value::Enum { id: c.u16()?, version: c.u16()? },
            0x2b | 0x2e => Value::Str(self.string()?),
            0x2c => {
                let n = usize::from(c.u16()?);
                Value::Bytes(c.take(n)?.to_vec())
            }
            0x35..=0x74 => Value::Bytes(c.take(usize::from(code - 0x34))?.to_vec()),
            0x2d => {
                let n = usize::try_from(c.u32()?).map_err(|_| Error::Malformed("blob length"))?;
                Value::Blob(c.take(n)?.to_vec())
            }
            0x33 => {
                c.u32()?;
                let n = usize::try_from(c.u32()?).map_err(|_| Error::Malformed("entry name length"))?;
                Value::Entry(String::from_utf8_lossy(c.take(n)?).into_owned())
            }
            0x75 => {
                c.u16()?;
                let n = usize::from(c.u8()?);
                if n > 8 {
                    return Err(Error::Malformed("flags wider than 64 bits"));
                }
                let mut b = [0u8; 8];
                b.get_mut(..n).ok_or(Error::Malformed("flags"))?.copy_from_slice(c.take(n)?);
                Value::Flags(u64::from_le_bytes(b))
            }
            _ => return Err(Error::Unsupported("unknown field type in document stream")),
        })
    }

    fn array(&mut self, code: u8) -> Result<Value, Error> {
        let min = match code {
            0x01 | 0x05 | 0x29 => 1,
            0x02 | 0x06 => 2,
            0x03 | 0x07 | 0x09 | 0x2f | 0x34 => 4,
            0x04 | 0x08 | 0x0a => 8,
            0x15..=0x19 => 4 * usize::from(code - 0x13),
            0x1f..=0x23 => 4 * usize::from(code - 0x1d),
            0x24..=0x28 => 8 * usize::from(code - 0x22),
            0x35..=0x74 => usize::from(code - 0x34),
            _ => 1,
        };
        match code {
            0x29 => {
                let n = usize::try_from(self.c.u32()?).map_err(|_| Error::Malformed("array length"))?;
                let bits = self.c.take(n.div_ceil(8))?;
                let mut v = self.vec(n)?;
                v.extend((0..n).map(|i| Value::Bool(bits.get(i / 8).is_some_and(|b| b >> (i % 8) & 1 != 0))));
                Ok(Value::Array(v))
            }
            0x2a => {
                let n = self.count(2)?;
                let version = self.c.u16()?;
                let mut v = self.vec(n)?;
                for _ in 0..n {
                    v.push(Value::Enum { id: self.c.u16()?, version });
                }
                Ok(Value::Array(v))
            }
            0x2b | 0x2e => {
                self.c.u32()?;
                let n = self.count(4)?;
                let mut v = self.vec(n)?;
                for _ in 0..n {
                    v.push(Value::Str(self.string()?));
                }
                Ok(Value::Array(v))
            }
            0x2c => {
                let n = usize::try_from(self.c.u32()?).map_err(|_| Error::Malformed("array length"))?;
                let size = usize::from(self.c.u16()?);
                // A zero width passes the length check for any count, then costs a Value per record.
                if size == 0 || n.checked_mul(size).is_none_or(|b| b > self.c.remaining()) {
                    return Err(Error::Malformed("array longer than its data"));
                }
                let mut v = self.vec(n)?;
                for _ in 0..n {
                    v.push(Value::Bytes(self.c.take(size)?.to_vec()));
                }
                Ok(Value::Array(v))
            }
            0x2d | 0x33 | 0x75 => Err(Error::Malformed("array of an unarrayable type")),
            _ => {
                let n = self.count(min)?;
                let mut v = self.vec(n)?;
                for _ in 0..n {
                    v.push(self.scalar(code)?);
                }
                Ok(Value::Array(v))
            }
        }
    }
}
