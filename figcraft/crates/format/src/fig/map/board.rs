use figcraft_doc::{BoardShape, VectorNetwork};

use super::layer::{own_text, valid_size};
use super::paint::paint;
use super::{Affine, Kv, MAX_DEPTH, Mapper, Obj, Value, View, generated_record, json, num, rows};

/// FigJam `ShapeWithTextType` mapped to a native [`BoardShape`] outline (`None` for the 7 kinds
/// FigCraft has no native outline for, which stay frames with their generated geometry).
pub(crate) fn board_shape(kind: &str) -> Option<BoardShape> {
    Some(match kind {
        "SQUARE" => BoardShape::Rectangle,
        "ROUNDED_RECTANGLE" => BoardShape::RoundedRectangle,
        "ELLIPSE" => BoardShape::Ellipse,
        "DIAMOND" => BoardShape::Diamond,
        "TRIANGLE_UP" => BoardShape::Triangle,
        "TRIANGLE_DOWN" => BoardShape::TriangleDown,
        "PARALLELOGRAM_RIGHT" => BoardShape::ParallelogramRight,
        "PARALLELOGRAM_LEFT" => BoardShape::ParallelogramLeft,
        "TRAPEZOID" => BoardShape::Trapezoid,
        "PENTAGON" => BoardShape::Pentagon,
        "HEXAGON" => BoardShape::Hexagon,
        "OCTAGON" => BoardShape::Octagon,
        "STAR" => BoardShape::Star,
        "PLUS" => BoardShape::Plus,
        "ARROW_LEFT" => BoardShape::ArrowLeft,
        "ARROW_RIGHT" => BoardShape::ArrowRight,
        "CHEVRON" => BoardShape::Chevron,
        "SPEECH_BUBBLE" => BoardShape::SpeechBubble,
        "ENG_DATABASE" => BoardShape::Database,
        "DOCUMENT_SINGLE" => BoardShape::Document,
        "PREDEFINED_PROCESS" => BoardShape::PredefinedProcess,
        "MANUAL_INPUT" => BoardShape::ManualInput,
        "INTERNAL_STORAGE" => BoardShape::InternalStorage,
        _ => return None,
    })
}

fn valid_child<'a>(v: &View<'a>, local_id: u32) -> Option<View<'a>> {
    let style = generated_record(v, "nodeGenerationData", local_id)?;
    let bounds = generated_record(v, "derivedImmutableFrameData", local_id)?;
    let child = View::of(style).with([(bounds, 1.0)]);
    (valid_size(child.get("size")).is_some() && child.transform().is_some()).then_some(child)
}

fn raw_text<'a>(v: &View<'a>) -> Option<&'a str> {
    v.get("textData").and_then(|t| t.get("characters")).and_then(Kv::str)
}

impl<'a> Mapper<'a> {
    /// Convert a FigJam whiteboard's `STICKY` or `SHAPE_WITH_TEXT` into a native [`figcraft_doc::BoardItem`]
    /// frame when its generated records are complete and supported; otherwise warn and return `None`
    /// so `board_snapshot` keeps the generated layers as in a `.fig`.
    pub(super) fn native_board_item(&mut self, own: &str, o: &mut Obj, v: &View<'a>, rest_id: &str, depth: usize) -> Option<Vec<Value>> {
        let out = match own {
            "STICKY" => self.native_sticky(o, v, rest_id, depth),
            "SHAPE_WITH_TEXT" => self.native_shape_with_text(o, v, rest_id, depth),
            _ => return None,
        };
        if out.is_none() {
            self.warn(format!("{} layers import as frames with their text", own.to_ascii_lowercase().replace('_', " ")));
        }
        out
    }

    fn native_sticky(&mut self, o: &mut Obj, v: &View<'a>, rest_id: &str, depth: usize) -> Option<Vec<Value>> {
        if v.get("nodeGenerationData").is_none()
            || v.get("derivedImmutableFrameData").and_then(|d| num(d, "version")) != Some(1.0)
            || depth >= MAX_DEPTH
        {
            return None;
        }
        let (_, raw_h) = valid_size(v.get("size"))?;
        let h = raw_h.clamp(0.0, crate::rest::props::MAX_SIZE);
        let bg = valid_child(v, 0)?;
        if !bg.list("strokePaints").is_empty() {
            return None;
        }
        let mut paint_warned = false;
        let bg_fills: Vec<Value> = bg.list("fillPaints").iter().filter_map(|p| paint(p, &mut |_| paint_warned = true)).collect();
        if paint_warned || bg_fills.len() != 1 || bg_fills.first().and_then(|p| p.get("type")).and_then(Value::as_str) != Some("SOLID") {
            return None;
        }
        let body = valid_child(v, 1)?;
        let note_chars = own_text(&body)?;
        let author_child = match generated_record(v, "nodeGenerationData", 2) {
            Some(style) => {
                let bounds = generated_record(v, "derivedImmutableFrameData", 2)?;
                let auth = View::of(style).with([(bounds, 1.0)]);
                if valid_size(auth.get("size")).is_none() || auth.transform().is_none() {
                    return None;
                }
                own_text(&auth).map(|chars| (auth, chars))
            }
            None => None,
        };
        let needed = 1 + usize::from(author_child.is_some());
        if self.count.saturating_add(needed) > self.budget() {
            return None;
        }
        let author_name = author_child.as_ref().map_or("", |(_, c)| *c);
        o.insert("fills".into(), Value::Array(bg_fills));
        o.insert("strokes".into(), json!([]));
        o.insert("clipsContent".into(), json!(false));
        o.insert("layoutMode".into(), json!("VERTICAL"));
        o.insert("itemSpacing".into(), json!(8.0));
        for side in ["paddingTop", "paddingRight", "paddingBottom", "paddingLeft"] {
            o.insert(side.into(), json!(20.0));
        }
        o.insert("primaryAxisAlignItems".into(), json!("SPACE_BETWEEN"));
        o.insert("layoutSizingHorizontal".into(), json!("FIXED"));
        o.insert("layoutSizingVertical".into(), json!("HUG"));
        o.insert("minHeight".into(), json!(h));
        o.insert("board".into(), json!({"type": "sticky", "author": author_name}));

        let mut children = Vec::with_capacity(needed);
        let note_name: String = note_chars.chars().take(40).collect();
        children.push(self.native_board_text(&body, &format!("{rest_id};note"), &note_name, None));
        if let Some((auth, _)) = author_child {
            children.push(self.native_board_text(&auth, &format!("{rest_id};author"), "Author", None));
        }
        Some(children)
    }

    fn native_shape_with_text(&mut self, o: &mut Obj, v: &View<'a>, rest_id: &str, depth: usize) -> Option<Vec<Value>> {
        let kind = v.str("shapeWithTextType").unwrap_or("SQUARE");
        let Some(shape) = board_shape(kind) else {
            self.warn(format!("shapeWithTextType `{kind}` has no FigCraft outline; importing as a frame"));
            return None;
        };
        if v.get("nodeGenerationData").is_none()
            || v.get("derivedImmutableFrameData").and_then(|d| num(d, "version")) != Some(1.0)
            || depth >= MAX_DEPTH
        {
            return None;
        }
        let (raw_w, raw_h) = valid_size(v.get("size"))?;
        let (cw, ch) = (raw_w.clamp(0.0, crate::rest::props::MAX_SIZE), raw_h.clamp(0.0, crate::rest::props::MAX_SIZE));
        let bg = valid_child(v, 0)?;
        let body = valid_child(v, 1)?;
        let chars = raw_text(&body)?;
        if chars.trim().is_empty() && own_text(v).is_some() {
            return None;
        }
        if self.count.saturating_add(2) > self.budget() {
            return None;
        }
        let [t, r, b, l] = shape.text_insets();
        o.insert("fills".into(), json!([]));
        o.insert("strokes".into(), json!([]));
        o.insert("clipsContent".into(), json!(false));
        o.insert("layoutMode".into(), json!("VERTICAL"));
        o.insert("paddingTop".into(), json!(t * ch));
        o.insert("paddingRight".into(), json!(r * cw));
        o.insert("paddingBottom".into(), json!(b * ch));
        o.insert("paddingLeft".into(), json!(l * cw));
        o.insert("primaryAxisAlignItems".into(), json!("CENTER"));
        o.insert("counterAxisAlignItems".into(), json!("CENTER"));
        o.insert("layoutSizingHorizontal".into(), json!("FIXED"));
        o.insert("layoutSizingVertical".into(), json!("FIXED"));
        o.insert("board".into(), json!({"type": "shape", "shape": shape.id()}));

        let mut outline = Obj::new();
        outline.insert("id".into(), json!(format!("{rest_id};outline")));
        outline.insert("type".into(), json!("VECTOR"));
        outline.insert("name".into(), json!("Outline"));
        self.common(&mut outline, &bg, "VECTOR", Affine::IDENTITY, None);
        outline.insert("size".into(), json!({"x": cw, "y": ch}));
        outline.insert("relativeTransform".into(), rows(Affine::IDENTITY));
        outline.insert("layoutPositioning".into(), json!("ABSOLUTE"));
        outline.insert("constraints".into(), json!({"horizontal": "SCALE", "vertical": "SCALE"}));
        let net = VectorNetwork::from_path(&shape.path(cw, ch));
        self.networks.push(Some(net));
        outline.insert("vectorNetworkRef".into(), json!(self.networks.len() - 1));
        self.count += 1;

        let name = if chars.trim().is_empty() { "Text".to_string() } else { chars.chars().take(40).collect() };
        let label = self.native_board_text(&body, &format!("{rest_id};label"), &name, Some("CENTER"));
        Some(vec![Value::Object(outline), label])
    }

    fn native_board_text(&mut self, child: &View<'a>, id: &str, name: &str, align_h: Option<&str>) -> Value {
        let mut layer = Obj::new();
        layer.insert("id".into(), json!(id));
        layer.insert("type".into(), json!("TEXT"));
        layer.insert("name".into(), json!(name));
        self.common(&mut layer, child, "TEXT", Affine::IDENTITY, None);
        layer.insert("relativeTransform".into(), rows(Affine::IDENTITY));
        layer.insert("layoutSizingHorizontal".into(), json!("FILL"));
        self.text(&mut layer, child);
        if let Some(Value::Object(style)) = layer.get_mut("style") {
            style.insert("textAutoResize".into(), json!("HEIGHT"));
            if let Some(align) = align_h {
                style.insert("textAlignHorizontal".into(), json!(align));
            }
        }
        self.count += 1;
        Value::Object(layer)
    }
}
