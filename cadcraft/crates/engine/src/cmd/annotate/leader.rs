//! LEADER: straight or spline leaders with MText, copied annotation, block reference, or none,
//! matching AutoCAD's hook-line, DIMGAP, DIMTAD, DIMASZ, DIMTXT and DIMSCALE placement rules.

use cadcraft_doc::{EntityKind, Handle, Insert, Leader, MText};
use cadcraft_geom::{Vec2, Vec3};

use super::common::{pick_at, style};
use super::{Accept, EngineError, Input, Interactive, Prompt, Result, Session, Step, v3};
use crate::cmd::curves::KW;
use crate::cmd::machines::number;

const HOOK_ANGLE_RAD: f64 = 15.0 * std::f64::consts::PI / 180.0;

struct DimCfg {
    style_name: String,
    text_style: String,
    arrow_size: f64,
    gap: f64,
    text_height: f64,
    tad: u8,
}

fn dim_cfg(s: &Session) -> Result<DimCfg> {
    let d = s.doc()?;
    let style_name = style(s);
    let base = d.dim_style(&style_name).cloned().unwrap_or_default();
    let st = base.with_overrides(&d.dim_overrides());
    let scale = st.effective_scale(d.header.f64("DIMSCALE", 1.0));
    Ok(DimCfg {
        style_name,
        text_style: if st.text_style.is_empty() { "Standard".into() } else { st.text_style },
        arrow_size: st.arrow_size * scale,
        gap: st.text_gap.abs() * scale,
        text_height: st.text_height * scale,
        tad: st.text_above,
    })
}

fn hooked_vertices(pts: &[Vec2], spline: bool, hook: f64) -> (Vec<Vec2>, f64) {
    let n = pts.len();
    if n < 2 {
        return (pts.to_vec(), 1.0);
    }
    let prev = pts[n - 2];
    let end = pts[n - 1];
    let delta = end - prev;
    let dir = if delta.x < -1e-12 { -1.0 } else { 1.0 };
    let off_horiz = delta.y.abs().atan2(delta.x.abs()) > HOOK_ANGLE_RAD + 1e-12;
    let mut out = pts[..n - 1].to_vec();
    if !spline && off_horiz && hook > 0.0 {
        out.push(end - Vec2::new(dir * hook, 0.0));
    }
    out.push(end);
    (out, dir)
}

fn add_leader_entity(s: &mut Session, verts: &[Vec2], arrow: bool, spline: bool, style_name: String) -> Result<Handle> {
    s.add_entity(EntityKind::Leader(Leader {
        vertices: verts.iter().map(|q| v3(*q)).collect(),
        arrow,
        spline,
        style: style_name,
    }))
}

/// Programmatic `leader` JSON form (`{points, text?}`), using the same hook and text placement as
/// the interactive command.
pub(super) fn json_leader(s: &mut Session, pts: &[Vec2], text: Option<&str>) -> Result<Vec<Handle>> {
    let cfg = dim_cfg(s)?;
    if let Some(contents) = text.filter(|t| !t.is_empty()) {
        place_typed_text(s, pts, true, false, &cfg, &contents.replace('\n', "\\P"))
    } else {
        let h = add_leader_entity(s, pts, true, false, cfg.style_name)?;
        Ok(vec![h])
    }
}

fn place_typed_text(
    s: &mut Session,
    pts: &[Vec2],
    arrow: bool,
    spline: bool,
    cfg: &DimCfg,
    contents: &str,
) -> Result<Vec<Handle>> {
    let Some(&end) = pts.last() else {
        return Ok(Vec::new());
    };
    let (verts, dir) = hooked_vertices(pts, spline, cfg.arrow_size);
    let lh = add_leader_entity(s, &verts, arrow, spline, cfg.style_name.clone())?;
    let left = dir < 0.0;
    let (insert, attach) = if cfg.tad != 0 {
        (end + Vec2::new(dir * cfg.gap, cfg.gap), if left { 9 } else { 7 })
    } else {
        (end + Vec2::new(dir * cfg.gap, 0.0), if left { 6 } else { 4 })
    };
    let mh = s.add_entity(EntityKind::MText(MText {
        insert: v3(insert),
        height: cfg.text_height,
        width: 0.0,
        attach,
        rotation: 0.0,
        style: cfg.text_style.clone(),
        contents: contents.to_string(),
        line_spacing: 1.0,
        line_spacing_exact: false,
        columns: None,
    }))?;
    Ok(vec![lh, mh])
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Stage {
    #[default]
    Points,
    Format,
    FirstLine,
    NextLine,
    Options,
    Copy,
    BlockName,
    BlockPoint,
    BlockXScale,
    BlockYScale,
    BlockRot,
}

pub(super) struct LeaderM {
    pts: Vec<Vec2>,
    spline: bool,
    arrow: bool,
    stage: Stage,
    lines: Vec<String>,
    block_name: String,
    block_at: Vec2,
    block_x: f64,
    block_y: f64,
}

impl Default for LeaderM {
    fn default() -> Self {
        Self {
            pts: Vec::new(),
            spline: false,
            arrow: true,
            stage: Stage::Points,
            lines: Vec::new(),
            block_name: String::new(),
            block_at: Vec2::ZERO,
            block_x: 1.0,
            block_y: 1.0,
        }
    }
}

impl LeaderM {
    fn finish_none(&self, s: &mut Session) -> Result<Step> {
        let cfg = dim_cfg(s)?;
        add_leader_entity(s, &self.pts, self.arrow, self.spline, cfg.style_name)?;
        Ok(Step::Done)
    }

    fn finish_lines(&self, s: &mut Session) -> Result<Step> {
        let cfg = dim_cfg(s)?;
        let contents = self.lines.join("\\P");
        place_typed_text(s, &self.pts, self.arrow, self.spline, &cfg, &contents)?;
        Ok(Step::Done)
    }

    fn finish_copy(&self, s: &mut Session, target: Handle) -> Result<Step> {
        let cfg = dim_cfg(s)?;
        let (contents, height, width, style_name) = {
            let d = s.doc()?;
            let Some(ent) = d.entity(target) else {
                s.echo("Object must be text, mtext, tolerance, or block.");
                return Ok(Step::Continue);
            };
            match &ent.kind {
                EntityKind::Text(t) => (t.value.clone(), t.height, 0.0, t.style.clone()),
                EntityKind::MText(m) => (m.contents.clone(), m.height, m.width, m.style.clone()),
                _ => {
                    s.echo("Object must be text, mtext, tolerance, or block.");
                    return Ok(Step::Continue);
                }
            }
        };
        let Some(&picked_end) = self.pts.last() else {
            return Ok(Step::Cancel);
        };
        let mut adj = self.pts.clone();
        if cfg.tad != 0
            && let Some(last) = adj.last_mut()
        {
            last.y -= height * 0.5;
        }
        let (verts, dir) = hooked_vertices(&adj, self.spline, cfg.arrow_size);
        add_leader_entity(s, &verts, self.arrow, self.spline, cfg.style_name)?;
        let left = dir < 0.0;
        let insert = if cfg.tad != 0 {
            picked_end + Vec2::new(dir * cfg.gap, cfg.gap)
        } else {
            picked_end + Vec2::new(dir * cfg.gap, 0.0)
        };
        s.add_entity(EntityKind::MText(MText {
            insert: v3(insert),
            height,
            width,
            attach: if left { 6 } else { 4 },
            rotation: 0.0,
            style: style_name,
            contents,
            line_spacing: 1.0,
            line_spacing_exact: false,
            columns: None,
        }))?;
        Ok(Step::Done)
    }

    fn finish_block(&self, s: &mut Session, rotation: f64) -> Result<Step> {
        let cfg = dim_cfg(s)?;
        add_leader_entity(s, &self.pts, self.arrow, self.spline, cfg.style_name)?;
        s.add_entity(EntityKind::Insert(Insert {
            block: self.block_name.clone(),
            insert: v3(self.block_at),
            scale: Vec3::new(self.block_x, self.block_y, self.block_x.abs()),
            rotation,
            attribs: Vec::new(),
            cols: 1,
            rows: 1,
            col_spacing: 0.0,
            row_spacing: 0.0,
            clip: None,
        }))?;
        Ok(Step::Done)
    }

    fn step_points(&mut self, i: Input) -> Step {
        match i {
            Input::Point(p) => {
                self.pts.push(p);
                Step::Continue
            }
            Input::Keyword(k) if self.pts.len() >= 2 => {
                match k.as_str() {
                    "Undo" => {
                        self.pts.pop();
                    }
                    "Format" => self.stage = Stage::Format,
                    _ => self.stage = Stage::FirstLine,
                }
                Step::Continue
            }
            Input::Enter if self.pts.len() >= 2 => {
                self.stage = Stage::FirstLine;
                Step::Continue
            }
            Input::Enter => Step::Cancel,
            _ => Step::Continue,
        }
    }

    fn step_format(&mut self, i: Input) {
        if let Input::Keyword(k) = i {
            match k.as_str() {
                "Spline" => self.spline = true,
                "STraight" => self.spline = false,
                "Arrow" => self.arrow = true,
                "None" => self.arrow = false,
                _ => {}
            }
        }
        self.stage = Stage::Points;
    }

    fn step_options(&mut self, s: &mut Session, i: Input) -> Result<Step> {
        match i {
            Input::Enter => self.stage = Stage::FirstLine,
            Input::Keyword(k) => match k.as_str() {
                "Mtext" | "Tolerance" => self.stage = Stage::FirstLine,
                "Copy" => self.stage = Stage::Copy,
                "Block" => self.stage = Stage::BlockName,
                "None" => return self.finish_none(s),
                _ => {}
            },
            Input::Text(_) => s.echo("Invalid option keyword."),
            _ => {}
        }
        Ok(Step::Continue)
    }

    fn step_block(&mut self, s: &mut Session, i: Input) -> Result<Step> {
        match self.stage {
            Stage::BlockName => {
                let name = match i {
                    Input::Text(t) => t.trim().to_string(),
                    Input::Keyword(k) if k == "?" => "?".to_string(),
                    _ => return Ok(Step::Continue),
                };
                if name == "?" {
                    let mut names: Vec<String> =
                        s.doc()?.blocks.values().filter(|b| !b.anonymous && !b.name.starts_with('*')).map(|b| b.name.clone()).collect();
                    names.sort();
                    s.echo(format!("Defined blocks: {}", if names.is_empty() { "(none)".into() } else { names.join(", ") }));
                    return Ok(Step::Continue);
                }
                match s.doc()?.block(&name) {
                    Some(b) if !b.anonymous && !b.name.starts_with('*') => {
                        self.block_name = b.name.clone();
                        self.stage = Stage::BlockPoint;
                    }
                    _ => s.echo(format!("Block \"{name}\" not found.")),
                }
            }
            Stage::BlockPoint => {
                if let Input::Point(p) = i {
                    self.block_at = p;
                    self.stage = Stage::BlockXScale;
                }
            }
            Stage::BlockXScale => match i {
                Input::Enter => {
                    self.block_x = 1.0;
                    self.stage = Stage::BlockYScale;
                }
                Input::Text(t) => {
                    let v = number(&t).filter(|v| *v != 0.0).ok_or_else(|| EngineError::Other("Requires a non-zero number.".into()))?;
                    self.block_x = v;
                    self.stage = Stage::BlockYScale;
                }
                Input::Point(p) => {
                    let d = p - self.block_at;
                    if d.x.abs() > 1e-12 && d.y.abs() > 1e-12 {
                        self.block_x = d.x;
                        self.block_y = d.y;
                        self.stage = Stage::BlockRot;
                    }
                }
                _ => {}
            },
            Stage::BlockYScale => match i {
                Input::Enter => {
                    self.block_y = self.block_x;
                    self.stage = Stage::BlockRot;
                }
                Input::Text(t) => {
                    let v = number(&t).filter(|v| *v != 0.0).ok_or_else(|| EngineError::Other("Requires a non-zero number.".into()))?;
                    self.block_y = v;
                    self.stage = Stage::BlockRot;
                }
                _ => {}
            },
            Stage::BlockRot => {
                let rot = match i {
                    Input::Enter => 0.0,
                    Input::Point(p) => self.block_at.angle_to(p),
                    Input::Text(t) => s.angle_settings().direction(&t).ok_or_else(|| EngineError::Other("Requires a valid angle.".into()))?,
                    _ => return Ok(Step::Continue),
                };
                return self.finish_block(s, rot);
            }
            _ => {}
        }
        Ok(Step::Continue)
    }
}

impl Interactive for LeaderM {
    fn name(&self) -> &'static str {
        "LEADER"
    }

    fn prompt(&self, _s: &Session) -> Prompt {
        match self.stage {
            Stage::Points => match self.pts.as_slice() {
                [] => Prompt::new("Specify leader start point", Accept::POINT),
                [b] => Prompt::new("Specify next point", Accept::POINT).base(*b),
                [.., b] => Prompt::new("Specify next point", Accept::POINT)
                    .kw(&["Annotation", "Format", "Undo"])
                    .default("Annotation")
                    .base(*b),
            },
            Stage::Format => Prompt::new("Enter leader format option", KW)
                .kw(&["Spline", "STraight", "Arrow", "None"])
                .default("Exit"),
            Stage::FirstLine => Prompt::new("Enter first line of annotation text or", Accept::TEXT).default("options"),
            Stage::NextLine => Prompt::new("Enter next line of annotation text", Accept::TEXT),
            Stage::Options => Prompt::new("Enter an annotation option", KW)
                .kw(&["Tolerance", "Copy", "Block", "None", "Mtext"])
                .default("Mtext"),
            Stage::Copy => Prompt::new("Select an object", Accept::SELECT),
            Stage::BlockName => Prompt::new("Enter block name", Accept::TEXT).kw(&["?"]),
            Stage::BlockPoint => Prompt::new("Specify insertion point", Accept::POINT),
            Stage::BlockXScale => Prompt::new("Enter X scale factor, specify opposite corner", Accept::POINT_OR_NUMBER)
                .default("1")
                .base(self.block_at),
            Stage::BlockYScale => Prompt::new("Enter Y scale factor", Accept::NUMBER).default("use X scale factor"),
            Stage::BlockRot => Prompt::new("Specify rotation angle", Accept::POINT_OR_NUMBER).default("0").base(self.block_at),
        }
    }

    fn input(&mut self, s: &mut Session, i: Input) -> Result<Step> {
        match self.stage {
            Stage::Points => Ok(self.step_points(i)),
            Stage::Format => {
                self.step_format(i);
                Ok(Step::Continue)
            }
            Stage::FirstLine => match i {
                Input::Enter => {
                    self.stage = Stage::Options;
                    Ok(Step::Continue)
                }
                Input::Text(t) if t.is_empty() => {
                    self.stage = Stage::Options;
                    Ok(Step::Continue)
                }
                Input::Text(t) => {
                    self.lines.push(t);
                    self.stage = Stage::NextLine;
                    Ok(Step::Continue)
                }
                _ => Ok(Step::Continue),
            },
            Stage::NextLine => match i {
                Input::Enter => self.finish_lines(s),
                Input::Text(t) if t.is_empty() => self.finish_lines(s),
                Input::Text(t) => {
                    self.lines.push(t);
                    Ok(Step::Continue)
                }
                _ => Ok(Step::Continue),
            },
            Stage::Options => self.step_options(s, i),
            Stage::Copy => match i {
                Input::Pick(hs) => {
                    let Some(&h) = hs.first() else { return Ok(Step::Continue) };
                    self.finish_copy(s, h)
                }
                Input::Point(p) => {
                    let Some(h) = pick_at(s, p) else { return Ok(Step::Continue) };
                    self.finish_copy(s, h)
                }
                _ => Ok(Step::Continue),
            },
            Stage::BlockName | Stage::BlockPoint | Stage::BlockXScale | Stage::BlockYScale | Stage::BlockRot => self.step_block(s, i),
        }
    }
}
