//! True HTML5 + CSS3 Fixed-Layout EPUB 3.0 scene emission (`rendition:layout` = `pre-paginated`).
//!
//! Emits positioned `<div>` frames, live selectable `<span>` text runs with `@font-face` embedded
//! fonts and `<a>` hyperlinks, inline `<svg>` vector paths/gradients, and `<img>` placed raster
//! assets inside each `<meta name="viewport" content="width=W, height=H"/>` XHTML5 page.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fmt::Write as _;

use crate::EpubOptions;
use crate::fonts::{EmbeddedFont, css_weight_style, font_file, generic_family};
use crate::markup::{EpubError, css_string, language_tag, xml_attr, xml_text};
use crate::package::{capped_title, contents_title, metadata};
use designcraft_doc::Document;

/// One pre-paginated page's structured HTML5 + CSS3 scene.
#[derive(Clone, Debug, Default)]
pub struct FixedPageScene {
    pub label: String,
    pub width: f64,
    pub height: f64,
    pub background_css: Option<String>,
    pub elements: Vec<FixedElement>,
}

/// A positioned scene-graph node on a fixed-layout EPUB page (coordinates relative to page top-left
/// in CSS pixels = points at 1:1 viewport scale).
#[derive(Clone, Debug)]
pub enum FixedElement {
    /// Rectangular or rounded frame (`<div>`), optionally wrapping nested child elements.
    Box(FixedBox),
    /// Text frame (`<div>`) containing rule/shading boxes and positioned Unicode `<span>` runs.
    TextFrame {
        frame: FixedBox,
        rules: Vec<FixedBox>,
        spans: Vec<FixedTextSpan>,
    },
    /// Non-rectangular vector path, line, ellipse, or polygon (`<svg>`).
    Svg(FixedVectorSvg),
    /// Placed raster image (`<img>`), optionally clipped by an outer frame box.
    Image(FixedRaster),
}

#[derive(Clone, Debug, Default)]
pub struct FixedBox {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    /// Optional 2D affine matrix `[a, b, c, d, e, f]` applied about `(0, 0)` when non-identity.
    pub matrix: Option<[f64; 6]>,
    pub background: Option<String>,
    pub border: Option<String>,
    pub border_radius: Option<String>,
    pub opacity: f32,
    pub blend_mode: Option<&'static str>,
    pub clip: bool,
    pub href: Option<String>,
    pub children: Vec<FixedElement>,
}

#[derive(Clone, Debug)]
pub struct FixedTextSpan {
    pub x: f64,
    pub y: f64,
    pub text: String,
    pub font_family: String,
    pub font_style: String,
    pub font_size: f64,
    pub color: String,
    pub letter_spacing: f64,
    pub underline: bool,
    pub strikethrough: bool,
    pub rtl: bool,
    pub href: Option<String>,
}

#[derive(Clone, Debug)]
pub struct FixedVectorSvg {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub view_box: [f64; 4],
    pub matrix: Option<[f64; 6]>,
    pub path_d: String,
    pub fill: String,
    pub gradient_defs: String,
    pub stroke: Option<String>,
    pub stroke_width: f64,
    pub stroke_dasharray: Option<String>,
    pub stroke_linecap: &'static str,
    pub stroke_linejoin: &'static str,
    pub opacity: f64,
    pub blend_mode: Option<&'static str>,
    pub href: Option<String>,
}

#[derive(Clone, Debug)]
pub struct FixedRaster {
    pub frame: FixedBox,
    pub img_x: f64,
    pub img_y: f64,
    pub img_width: f64,
    pub img_height: f64,
    pub img_matrix: Option<[f64; 6]>,
    pub asset_key: String,
    pub mime: String,
    pub data: Vec<u8>,
    pub alt: String,
}

fn fixed_stylesheet(fonts: &[EmbeddedFont]) -> String {
    let mut css = String::new();
    for (k, f) in fonts.iter().enumerate() {
        let (path, _) = font_file(k, f);
        let (weight, style) = css_weight_style(&f.style);
        let _ = writeln!(
            css,
            "@font-face {{ font-family: \"{}\"; src: url(\"{path}\"); font-weight: {weight}; font-style: {style}; }}",
            css_string(&f.family)
        );
    }
    css.push_str(
        "html, body { margin: 0; padding: 0; overflow: hidden; background: #ffffff; }\n\
         .dc-page { position: relative; overflow: hidden; box-sizing: border-box; background: #ffffff; }\n\
         .dc-frame { position: absolute; box-sizing: border-box; transform-origin: 0 0; }\n\
         .dc-span { position: absolute; white-space: pre; line-height: 1; transform-origin: 0 0; margin: 0; padding: 0; }\n\
         .dc-svg { position: absolute; overflow: visible; transform-origin: 0 0; pointer-events: none; }\n\
         .dc-img { position: absolute; display: block; max-width: none; transform-origin: 0 0; }\n\
         a.dc-link { color: inherit; text-decoration: none; }\n",
    );
    css
}

fn image_ext(mime: &str, data: &[u8]) -> (&'static str, &'static str) {
    if mime == "image/jpeg" || data.starts_with(&[0xFF, 0xD8, 0xFF]) {
        ("jpg", "image/jpeg")
    } else if mime == "image/webp" || (data.len() >= 12 && &data[0..4] == b"RIFF" && &data[8..12] == b"WEBP") {
        ("webp", "image/webp")
    } else if mime == "image/gif" || data.starts_with(b"GIF8") {
        ("gif", "image/gif")
    } else if mime == "image/svg+xml" {
        ("svg", "image/svg+xml")
    } else {
        ("png", "image/png")
    }
}

struct AssetRegistry {
    by_key: BTreeMap<String, (String, &'static str, Vec<u8>)>,
}

impl AssetRegistry {
    fn new() -> Self {
        Self { by_key: BTreeMap::new() }
    }

    fn register(&mut self, key: &str, mime: &str, data: &[u8]) -> String {
        if let Some((path, _, _)) = self.by_key.get(key) {
            return path.clone();
        }
        let idx = self.by_key.len();
        let (ext, clean_mime) = image_ext(mime, data);
        let path = format!("images/asset_{idx}.{ext}");
        self.by_key.insert(key.to_string(), (path.clone(), clean_mime, data.to_vec()));
        path
    }
}

fn write_box_style(b: &FixedBox, style: &mut String) {
    let _ = write!(style, "left:{:.2}px;top:{:.2}px;width:{:.2}px;height:{:.2}px;", b.x, b.y, b.width.max(0.0), b.height.max(0.0));
    if let Some([m0, m1, m2, m3, m4, m5]) = b.matrix {
        let _ = write!(style, "transform:matrix({m0:.5},{m1:.5},{m2:.5},{m3:.5},{m4:.2},{m5:.2});");
    }
    if let Some(bg) = &b.background {
        let _ = write!(style, "background:{bg};");
    }
    if let Some(border) = &b.border {
        let _ = write!(style, "border:{border};");
    }
    if let Some(radius) = &b.border_radius {
        let _ = write!(style, "border-radius:{radius};");
    }
    if b.opacity < 0.999 {
        let _ = write!(style, "opacity:{:.3};", b.opacity.clamp(0.0, 1.0));
    }
    if let Some(bm) = b.blend_mode {
        let _ = write!(style, "mix-blend-mode:{bm};");
    }
    if b.clip {
        style.push_str("overflow:hidden;");
    }
}

fn render_element(
    el: &FixedElement,
    generics: &BTreeMap<String, String>,
    assets: &mut AssetRegistry,
    out: &mut String,
) {
    match el {
        FixedElement::Box(b) => {
            if let Some(href) = &b.href {
                let _ = write!(out, "<a class=\"dc-link\" href=\"{}\">", xml_attr(href));
            }
            let mut style = String::new();
            write_box_style(b, &mut style);
            let _ = write!(out, "<div class=\"dc-frame\" style=\"{}\">", xml_attr(&style));
            for child in &b.children {
                render_element(child, generics, assets, out);
            }
            out.push_str("</div>");
            if b.href.is_some() {
                out.push_str("</a>");
            }
        }
        FixedElement::TextFrame { frame, rules, spans } => {
            if let Some(href) = &frame.href {
                let _ = write!(out, "<a class=\"dc-link\" href=\"{}\">", xml_attr(href));
            }
            let mut style = String::new();
            write_box_style(frame, &mut style);
            let _ = write!(out, "<div class=\"dc-frame\" style=\"{}\">", xml_attr(&style));
            for r in rules {
                let mut rs = String::new();
                write_box_style(r, &mut rs);
                let _ = write!(out, "<div class=\"dc-frame\" style=\"{}\"></div>", xml_attr(&rs));
            }
            for sp in spans {
                let (weight, fstyle) = css_weight_style(&sp.font_style);
                let generic = generic_family(&sp.font_family, generics);
                let mut ss = String::new();
                let _ = write!(
                    ss,
                    "left:{:.2}px;top:{:.2}px;font-family:\"{}\", {generic};font-size:{:.2}px;font-weight:{weight};font-style:{fstyle};color:{};",
                    sp.x,
                    sp.y,
                    css_string(&sp.font_family),
                    sp.font_size.max(1.0),
                    sp.color,
                );
                if sp.letter_spacing.abs() > 0.005 {
                    let _ = write!(ss, "letter-spacing:{:.3}px;", sp.letter_spacing);
                }
                if sp.underline || sp.strikethrough {
                    let dec = match (sp.underline, sp.strikethrough) {
                        (true, true) => "underline line-through",
                        (true, false) => "underline",
                        (false, true) => "line-through",
                        (false, false) => "none",
                    };
                    let _ = write!(ss, "text-decoration:{dec};");
                }
                let dir_attr = if sp.rtl { " dir=\"rtl\"" } else { "" };
                if let Some(href) = &sp.href {
                    let _ = write!(
                        out,
                        "<a class=\"dc-link\" href=\"{}\"><span class=\"dc-span\"{dir_attr} style=\"{}\">{}</span></a>",
                        xml_attr(href),
                        xml_attr(&ss),
                        xml_text(&sp.text)
                    );
                } else {
                    let _ = write!(
                        out,
                        "<span class=\"dc-span\"{dir_attr} style=\"{}\">{}</span>",
                        xml_attr(&ss),
                        xml_text(&sp.text)
                    );
                }
            }
            for child in &frame.children {
                render_element(child, generics, assets, out);
            }
            out.push_str("</div>");
            if frame.href.is_some() {
                out.push_str("</a>");
            }
        }
        FixedElement::Svg(svg) => {
            if let Some(href) = &svg.href {
                let _ = write!(out, "<a class=\"dc-link\" href=\"{}\">", xml_attr(href));
            }
            let mut style = format!(
                "left:{:.2}px;top:{:.2}px;width:{:.2}px;height:{:.2}px;",
                svg.x,
                svg.y,
                svg.width.max(1.0),
                svg.height.max(1.0)
            );
            if let Some([m0, m1, m2, m3, m4, m5]) = svg.matrix {
                let _ = write!(style, "transform:matrix({m0:.5},{m1:.5},{m2:.5},{m3:.5},{m4:.2},{m5:.2});");
            }
            if svg.opacity < 0.999 {
                let _ = write!(style, "opacity:{:.3};", svg.opacity.clamp(0.0, 1.0));
            }
            if let Some(bm) = svg.blend_mode {
                let _ = write!(style, "mix-blend-mode:{bm};");
            }
            let [vx, vy, vw, vh] = svg.view_box;
            let _ = write!(
                out,
                "<svg xmlns=\"http://www.w3.org/2000/svg\" class=\"dc-svg\" viewBox=\"{vx:.2} {vy:.2} {:.2} {:.2}\" style=\"{}\">",
                vw.max(1.0),
                vh.max(1.0),
                xml_attr(&style),
            );
            if !svg.gradient_defs.is_empty() {
                let _ = write!(out, "<defs>{}</defs>", svg.gradient_defs);
            }
            let _ = write!(out, "<path d=\"{}\" fill=\"{}\"", xml_attr(&svg.path_d), xml_attr(&svg.fill));
            if let Some(stroke) = &svg.stroke
                && svg.stroke_width > 0.0
            {
                let _ = write!(
                    out,
                    " stroke=\"{}\" stroke-width=\"{:.2}\" stroke-linecap=\"{}\" stroke-linejoin=\"{}\"",
                    xml_attr(stroke),
                    svg.stroke_width,
                    svg.stroke_linecap,
                    svg.stroke_linejoin
                );
                if let Some(dash) = &svg.stroke_dasharray {
                    let _ = write!(out, " stroke-dasharray=\"{}\"", xml_attr(dash));
                }
            } else {
                out.push_str(" stroke=\"none\"");
            }
            out.push_str("/></svg>");
            if svg.href.is_some() {
                out.push_str("</a>");
            }
        }
        FixedElement::Image(ras) => {
            let src = assets.register(&ras.asset_key, &ras.mime, &ras.data);
            if let Some(href) = &ras.frame.href {
                let _ = write!(out, "<a class=\"dc-link\" href=\"{}\">", xml_attr(href));
            }
            let mut fstyle = String::new();
            write_box_style(&ras.frame, &mut fstyle);
            let _ = write!(out, "<div class=\"dc-frame\" style=\"{}\">", xml_attr(&fstyle));
            let mut istyle = format!(
                "left:{:.2}px;top:{:.2}px;width:{:.2}px;height:{:.2}px;",
                ras.img_x,
                ras.img_y,
                ras.img_width.max(0.0),
                ras.img_height.max(0.0)
            );
            if let Some([m0, m1, m2, m3, m4, m5]) = ras.img_matrix {
                let _ = write!(istyle, "transform:matrix({m0:.5},{m1:.5},{m2:.5},{m3:.5},{m4:.2},{m5:.2});");
            }
            let _ = write!(
                out,
                "<img class=\"dc-img\" src=\"{}\" alt=\"{}\" style=\"{}\"/>",
                xml_attr(&src),
                xml_attr(&ras.alt),
                xml_attr(&istyle),
            );
            out.push_str("</div>");
            if ras.frame.href.is_some() {
                out.push_str("</a>");
            }
        }
    }
}

/// Exports a pre-paginated EPUB 3.0 package using true HTML5 + CSS3 scene markup per page.
pub fn export_fixed_epub_scene(
    doc: &Document,
    pages: &[FixedPageScene],
    opts: &EpubOptions,
) -> Result<Vec<u8>, EpubError> {
    let title = capped_title(opts.title.clone().unwrap_or_else(|| doc.metadata_title().to_string()));
    let lang = language_tag(&opts.language);
    let mut resources = Vec::new();
    let css = fixed_stylesheet(&opts.fonts);
    resources.push(craft_epub::Resource::new("fixed-css", "fixed.css", "text/css", css.into_bytes()));

    for (index, font) in opts.fonts.iter().enumerate() {
        let (path, mime) = font_file(index, font);
        resources.push(craft_epub::Resource::new(
            format!("font{index}"),
            path,
            mime,
            Cow::Borrowed(font.data.as_slice()),
        ));
    }

    let mut assets = AssetRegistry::new();
    let mut spine = Vec::with_capacity(pages.len());
    let mut navigation = Vec::with_capacity(pages.len());

    for (index, page) in pages.iter().enumerate() {
        let number = index + 1;
        let width = page.width.round().max(1.0) as u32;
        let height = page.height.round().max(1.0) as u32;
        let label = if page.label.is_empty() { number.to_string() } else { page.label.clone() };
        let mut body_inner = String::new();
        for el in &page.elements {
            render_element(el, &opts.generic_families, &mut assets, &mut body_inner);
        }
        let mut page_style = format!("width:{width}px;height:{height}px;");
        if let Some(bg) = &page.background_css {
            let _ = write!(page_style, "background:{bg};");
        }
        let xhtml = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
             <!DOCTYPE html>\n\
             <html xmlns=\"http://www.w3.org/1999/xhtml\" xmlns:epub=\"http://www.idpf.org/2007/ops\" xml:lang=\"{lang}\" lang=\"{lang}\">\n\
             <head>\n\
               <meta charset=\"utf-8\"/>\n\
               <meta name=\"viewport\" content=\"width={width}, height={height}\"/>\n\
               <title>{} — {}</title>\n\
               <link rel=\"stylesheet\" type=\"text/css\" href=\"fixed.css\"/>\n\
             </head>\n\
             <body id=\"p{number}\">\n\
               <div class=\"dc-page\" style=\"{}\">\n{body_inner}\n  </div>\n\
             </body>\n\
             </html>\n",
            xml_text(&title),
            xml_text(&label),
            xml_attr(&page_style),
        );
        resources.push(craft_epub::Resource::new(
            format!("page{number}"),
            format!("page{number}.xhtml"),
            "application/xhtml+xml",
            xhtml.into_bytes(),
        ));
        spine.push(format!("page{number}"));
        navigation.push(craft_epub::NavItem {
            href: format!("page{number}.xhtml"),
            label,
            level: 0,
        });
    }

    for (idx, (_, (path, mime, data))) in assets.by_key.into_iter().enumerate() {
        resources.push(craft_epub::Resource::new(format!("fimg{idx}"), path, mime, data).stored());
    }

    if let Some(cover) = &opts.cover {
        let (path, mime) = if cover.starts_with(&[0xFF, 0xD8, 0xFF]) {
            ("images/cover.jpg", "image/jpeg")
        } else {
            ("images/cover.png", "image/png")
        };
        resources.push(
            craft_epub::Resource::new("cover-img", path, mime, Cow::Borrowed(cover.as_slice()))
                .with_properties("cover-image")
                .stored(),
        );
    }

    let (width, height) = pages
        .first()
        .map_or((612, 792), |p| (p.width.round().max(1.0) as u32, p.height.round().max(1.0) as u32));
    let heading = opts
        .toc_title
        .as_deref()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| contents_title(&opts.language));
    let book = craft_epub::Book {
        metadata: metadata(doc, opts, &title, lang),
        layout: craft_epub::Layout::PrePaginated { width, height },
        resources,
        spine,
        nav: craft_epub::Nav {
            kind: craft_epub::NavKind::PageList,
            heading: heading.to_owned(),
            items: navigation,
        },
        ncx: false,
    };
    craft_epub::write(&book).map_err(|error| EpubError(error.to_string()))
}
