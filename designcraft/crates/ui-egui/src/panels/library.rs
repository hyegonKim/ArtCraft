//! Object Library panel: the open library's items; add the selection, place or delete an item;
//! New / Open library.

use serde_json::json;

use crate::DesignApp;
use crate::app::file_dialogs::Bound;
use crate::theme::Tokens;

pub fn show(app: &mut DesignApp, ui: &mut egui::Ui) {
    let t = Tokens::get(ui.ctx());
    library_conveyor(app, ui, t);
    ui.separator();
    library_file_buttons(app, ui);
    let (query, type_filter, sort_by) = library_filter_bar(app, ui);
    let list_params = json!({"query": query, "type": type_filter, "sort": sort_by});
    let Ok(items) = app.session.execute("library.list", &list_params) else {
        crate::rtl::label(ui, egui::RichText::new(crate::i18n::tr(&app.ui.language, "No library open.")).size(11.0).color(t.text_dim));
        return;
    };
    library_asset_actions(app, ui);
    ui.separator();
    let items = items.as_array().cloned().unwrap_or_default();
    if items.is_empty() {
        crate::rtl::label(
            ui,
            crate::rtl::widget(
                ui,
                egui::RichText::new(crate::i18n::tr(&app.ui.language, "The library is empty. Select objects and Add Selection."))
                    .size(11.0)
                    .color(t.text_dim),
            ),
        );
    }
    for it in items {
        library_item_row(app, ui, t, &it);
    }
}

fn library_conveyor(app: &mut DesignApp, ui: &mut egui::Ui, t: Tokens) {
    let conveyor = app.session.conveyor.iter().map(|c| c.0.clone()).collect::<Vec<_>>();
    crate::rtl::label(ui, egui::RichText::new(format!("{} ({})", crate::i18n::tr(&app.ui.language, "Conveyor"), conveyor.len())).strong());
    if conveyor.is_empty() {
        crate::rtl::label(
            ui,
            crate::rtl::widget(
                ui,
                egui::RichText::new(crate::i18n::tr(
                    &app.ui.language,
                    "Collect objects with the Content Collector (B); place them with the Content Placer.",
                ))
                .size(10.5)
                .color(t.text_dim),
            ),
        );
    } else {
        crate::rtl::label(ui, egui::RichText::new(conveyor.join(", ")).size(10.5));
        if ui.small_button(crate::rtl::widget(ui, crate::i18n::tr(&app.ui.language, "Clear"))).clicked() {
            let _ = app.run("conveyor.clear", json!({}));
        }
    }
}

fn library_file_buttons(app: &mut DesignApp, ui: &mut egui::Ui) {
    ui.horizontal_wrapped(|ui| {
        if ui.button(crate::rtl::widget(ui, crate::i18n::tr(&app.ui.language, "New Library…"))).clicked() {
            let asked = app.ask_save("Library.indl", Bound::App, |app, path| app.run("library.new", json!({"path": path})));
            report(app, "Library", asked);
        }
        if ui.button(crate::rtl::widget(ui, crate::i18n::tr(&app.ui.language, "Open…"))).clicked() {
            let asked = app.ask_open("library", Bound::App, |app, path| {
                app.run("library.open", json!({"path": path})).or_else(|e| report_err(app, "Library", e))
            });
            report(app, "Library", asked);
        }
        if app.session.library.is_some()
            && ui.button(crate::rtl::widget(ui, crate::i18n::tr(&app.ui.language, "Save As .indl…"))).clicked()
        {
            let asked = app.ask_save("Library.indl", Bound::App, |app, path| {
                app.run("library.saveAs", json!({"path": path})).or_else(|e| report_err(app, "Library", e))
            });
            report(app, "Library", asked);
        }
    });
}

fn library_filter_bar(app: &DesignApp, ui: &mut egui::Ui) -> (String, String, String) {
    let q_id = egui::Id::unique("lib_filter_q");
    let t_id = egui::Id::unique("lib_filter_type");
    let s_id = egui::Id::unique("lib_filter_sort");
    let mut query: String = ui.data(|d| d.get_temp(q_id)).unwrap_or_default();
    let mut obj_type: String = ui.data(|d| d.get_temp(t_id)).unwrap_or_default();
    let mut sort_by: String = ui.data(|d| d.get_temp(s_id)).unwrap_or_else(|| "newest".into());
    if app.session.library.is_none() {
        return (query, obj_type, sort_by);
    }
    let lang = app.ui.language.as_str();
    ui.horizontal_wrapped(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut query)
                .desired_width(110.0)
                .hint_text(crate::i18n::tr(lang, "Search assets…")),
        );
        let type_label = if obj_type.is_empty() { "All Types" } else { obj_type.as_str() };
        craft_ui::ComboBox::from_id_salt("lib_type_combo")
            .selected_text(crate::rtl::widget(ui, crate::i18n::tr(lang, type_label)))
            .width(88.0)
            .show_ui(ui, |ui| {
                for (val, label) in [("", "All Types"), ("text", "text"), ("image", "image"), ("shape", "shape"), ("geometry", "geometry"), ("group", "group")] {
                    if ui.selectable_label(obj_type == val, crate::rtl::widget(ui, crate::i18n::tr(lang, label))).clicked() {
                        obj_type = val.to_string();
                    }
                }
            });
        let sort_label = match sort_by.as_str() {
            "name" => "Sort: Name",
            "type" => "Sort: Type",
            _ => "Sort: Newest",
        };
        craft_ui::ComboBox::from_id_salt("lib_sort_combo")
            .selected_text(crate::rtl::widget(ui, crate::i18n::tr(lang, sort_label)))
            .width(96.0)
            .show_ui(ui, |ui| {
                for (val, label) in [("newest", "Sort: Newest"), ("name", "Sort: Name"), ("type", "Sort: Type")] {
                    if ui.selectable_label(sort_by == val, crate::rtl::widget(ui, crate::i18n::tr(lang, label))).clicked() {
                        sort_by = val.to_string();
                    }
                }
            });
    });
    ui.data_mut(|d| {
        d.insert_temp(q_id, query.clone());
        d.insert_temp(t_id, obj_type.clone());
        d.insert_temp(s_id, sort_by.clone());
    });
    (query, obj_type, sort_by)
}

fn library_asset_actions(app: &mut DesignApp, ui: &mut egui::Ui) {
    let has_sel = app.session.active().is_some_and(|d| !d.selection.items.is_empty());
    ui.horizontal_wrapped(|ui| {
        if ui.add_enabled(has_sel, egui::Button::new(crate::rtl::widget(ui, crate::i18n::tr(&app.ui.language, "Add Selection")))).clicked()
            && let Err(e) = app.run("library.add", json!({}))
        {
            app.status_error("Library", e);
        }
        if ui.button(crate::rtl::widget(ui, crate::i18n::tr(&app.ui.language, "Import IDMS…"))).clicked() {
            let asked = app.ask_open("idms", Bound::App, |app, path| {
                app.run("library.importIdms", json!({"path": path})).or_else(|e| report_err(app, "Library", e))
            });
            report(app, "Library", asked);
        }
    });
}

fn library_item_row(app: &mut DesignApp, ui: &mut egui::Ui, t: Tokens, it: &serde_json::Value) {
    let i = it["index"].clone();
    let name = it["name"].as_str().unwrap_or("").to_string();
    let kind = it["objectType"].as_str().unwrap_or("shape");
    ui.horizontal(|ui| {
        crate::rtl::label(ui, &name);
        crate::rtl::label(ui, egui::RichText::new(format!("[{kind}]")).size(10.0).color(t.text_dim));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.small_button(crate::rtl::widget(ui, crate::i18n::tr(&app.ui.language, "Delete"))).clicked() {
                let _ = app.run("library.remove", json!({"index": i.clone()}));
            }
            if ui.small_button("IDMS").on_hover_text(crate::i18n::tr(&app.ui.language, "Export IDMS Snippet…")).clicked() {
                let idx = i.clone();
                let stem = if name.is_empty() { "Asset".to_string() } else { name.replace(['/', ':', '\\'], "_") };
                let asked = app.ask_save(&format!("{stem}.idms"), Bound::App, move |app, path| {
                    app.run("library.exportIdms", json!({"index": idx, "path": path})).or_else(|e| report_err(app, "Library", e))
                });
                report(app, "Library", asked);
            }
            if ui
                .small_button(crate::rtl::widget(ui, crate::i18n::tr(&app.ui.language, "Place Original")))
                .on_hover_text(crate::i18n::tr(&app.ui.language, "Place Item(s) in Original Position"))
                .clicked()
                && let Err(e) = app.run("library.placeOriginal", json!({"index": i.clone()}))
            {
                app.status_error("Library", e);
            }
            if ui.small_button(crate::rtl::widget(ui, crate::i18n::tr(&app.ui.language, "Place"))).clicked()
                && let Err(e) = app.run("library.place", json!({"index": i}))
            {
                app.status_error("Library", e);
            }
        });
    });
}

/// A dialog that couldn't be asked for, in the status line as `feature`'s error.
fn report(app: &mut DesignApp, feature: &str, asked: Result<serde_json::Value, String>) {
    if let Err(e) = asked {
        app.status_error(feature, e);
    }
}

/// A command run with a dialog's answer failed: the status line names `feature`.
fn report_err(app: &mut DesignApp, feature: &str, e: String) -> Result<serde_json::Value, String> {
    app.status_error(feature, e);
    Ok(serde_json::Value::Null)
}

/// Scripts › Load…: the chosen script becomes a new, selected one. Read through the host's
/// reader, like every other file the UI opens: this crate has no file system of its own.
fn load_script(app: &mut DesignApp, path: &str, sel_id: egui::Id) {
    let read = app.services.read.as_mut().map_or_else(|| Err("no file access".to_string()), |r| r(path));
    match read.and_then(|bytes| String::from_utf8(bytes).map_err(|e| e.to_string())) {
        Ok(text) => {
            let name = std::path::Path::new(path).file_stem().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "Script".into());
            app.ui.scripts.push((name, text));
            let sel = app.ui.scripts.len().saturating_sub(1);
            if let Some(ctx) = &app.egui_ctx {
                ctx.data_mut(|d| d.insert_temp(sel_id, sel));
            }
        }
        Err(e) => app.status_error("Scripts", e),
    }
}

fn book_open_buttons(app: &mut DesignApp, ui: &mut egui::Ui) {
    ui.horizontal_wrapped(|ui| {
        if ui.button(crate::rtl::widget(ui, crate::i18n::tr(&app.ui.language, "New Book…"))).clicked() {
            let asked = app
                .ask_save("Book.indb", Bound::App, |app, path| app.run("book.new", json!({"path": path})).or_else(|e| report_err(app, "Book", e)));
            report(app, "Book", asked);
        }
        if ui.button(crate::rtl::widget(ui, crate::i18n::tr(&app.ui.language, "Open…"))).clicked() {
            let asked =
                app.ask_open("book", Bound::App, |app, path| app.run("book.open", json!({"path": path})).or_else(|e| report_err(app, "Book", e)));
            report(app, "Book", asked);
        }
        if app.session.book.is_some()
            && ui.button(crate::rtl::widget(ui, crate::i18n::tr(&app.ui.language, "Save As .indb…"))).clicked()
        {
            let asked = app.ask_save("Book.indb", Bound::App, |app, path| {
                app.run("book.saveAs", json!({"path": path})).or_else(|e| report_err(app, "Book", e))
            });
            report(app, "Book", asked);
        }
    });
}

fn book_options_bar(app: &mut DesignApp, ui: &mut egui::Ui, info: &serde_json::Value) {
    let lang = app.ui.language.clone();
    let mode = info["pageNumbering"].as_str().unwrap_or("continue").to_string();
    let mut blank = info["insertBlankPage"].as_bool().unwrap_or(false);
    let mut lists = info["continueLists"].as_bool().unwrap_or(true);
    let mode_label = match mode.as_str() {
        "nextOdd" => "Continue on Next Odd Page",
        "nextEven" => "Continue on Next Even Page",
        _ => "Continue from Previous Document",
    };
    ui.horizontal_wrapped(|ui| {
        crate::rtl::label(ui, crate::i18n::tr(&lang, "Numbering:"));
        craft_ui::ComboBox::from_id_salt("book_num_mode")
            .selected_text(crate::rtl::widget(ui, crate::i18n::tr(&lang, mode_label)))
            .width(190.0)
            .show_ui(ui, |ui| {
                for (val, label) in [
                    ("continue", "Continue from Previous Document"),
                    ("nextOdd", "Continue on Next Odd Page"),
                    ("nextEven", "Continue on Next Even Page"),
                ] {
                    if ui.selectable_label(mode == val, crate::rtl::widget(ui, crate::i18n::tr(&lang, label))).clicked() {
                        let _ = app.run("book.options", json!({"pageNumbering": val}));
                    }
                }
            });
        if ui.checkbox(&mut blank, crate::rtl::widget(ui, crate::i18n::tr(&lang, "Insert Blank Page"))).changed() {
            let _ = app.run("book.options", json!({"insertBlankPage": blank}));
        }
        if ui.checkbox(&mut lists, crate::rtl::widget(ui, crate::i18n::tr(&lang, "Continue Lists"))).changed() {
            let _ = app.run("book.options", json!({"continueLists": lists}));
        }
    });

    egui::CollapsingHeader::new(crate::i18n::tr(&lang, "Synchronize Options"))
        .id_salt("book_sync_opts")
        .default_open(false)
        .show(ui, |ui| {
            let sync = &info["syncOptions"];
            let cats = [
                ("paragraphStyles", "Paragraph Styles"),
                ("characterStyles", "Character Styles"),
                ("objectStyles", "Object Styles"),
                ("tableStyles", "Table Styles"),
                ("cellStyles", "Cell Styles"),
                ("swatches", "Swatches"),
                ("parentPages", "Parent Pages"),
                ("numberedLists", "Numbered Lists"),
                ("textVariables", "Text Variables"),
                ("conditionalText", "Conditional Text"),
            ];
            ui.horizontal_wrapped(|ui| {
                for (key, label) in cats {
                    let mut checked = sync[key].as_bool().unwrap_or(true);
                    if ui.checkbox(&mut checked, crate::rtl::widget(ui, crate::i18n::tr(&lang, label))).changed() {
                        let mut p = serde_json::Map::new();
                        p.insert(key.into(), json!(checked));
                        let _ = app.run("book.options", serde_json::Value::Object(p));
                    }
                }
            });
        });
}

fn book_action_buttons(app: &mut DesignApp, ui: &mut egui::Ui) {
    ui.horizontal_wrapped(|ui| {
        if ui.button(crate::rtl::widget(ui, crate::i18n::tr(&app.ui.language, "Add Document…"))).clicked() {
            let asked =
                app.ask_open("open", Bound::App, |app, path| app.run("book.add", json!({"path": path})).or_else(|e| report_err(app, "Book", e)));
            report(app, "Book", asked);
        }
        if ui.button(crate::rtl::widget(ui, crate::i18n::tr(&app.ui.language, "Update Numbering"))).clicked() {
            let _ = app.run("book.paginate", json!({}));
        }
        if ui.button(crate::rtl::widget(ui, crate::i18n::tr(&app.ui.language, "Synchronize"))).clicked() {
            let _ = app.run("book.syncStyles", json!({}));
        }
        if ui.button(crate::rtl::widget(ui, crate::i18n::tr(&app.ui.language, "Preflight Book"))).clicked() {
            match app.run("book.preflight", json!({})) {
                Ok(r) => {
                    let errs = r["errors"].as_u64().unwrap_or(0);
                    let warns = r["warnings"].as_u64().unwrap_or(0);
                    app.status(format!("{}: {errs} errors, {warns} warnings", crate::i18n::tr(&app.ui.language, "Preflight Book")));
                }
                Err(e) => app.status_error("Book", e),
            }
        }
        if ui.button(crate::rtl::widget(ui, crate::i18n::tr(&app.ui.language, "Export PDF…"))).clicked() {
            let asked = app.ask_save("Book.pdf", Bound::App, |app, path| {
                app.run("book.exportPdf", json!({"path": path})).or_else(|e| report_err(app, "Book", e))
            });
            report(app, "Book", asked);
        }
        if ui.button(crate::rtl::widget(ui, crate::i18n::tr(&app.ui.language, "Export EPUB…"))).clicked() {
            let asked = app.ask_save("Book.epub", Bound::App, |app, path| {
                app.run("book.exportEpub", json!({"path": path})).or_else(|e| report_err(app, "Book", e))
            });
            report(app, "Book", asked);
        }
    });
}

/// Book panel: the open book's documents with their page ranges; numbering, synchronising and
/// exporting the whole book.
pub fn book(app: &mut DesignApp, ui: &mut egui::Ui) {
    let t = Tokens::get(ui.ctx());
    book_open_buttons(app, ui);
    let Ok(info) = app.session.execute("book.list", &json!({})) else {
        crate::rtl::label(ui, egui::RichText::new(crate::i18n::tr(&app.ui.language, "No book open.")).size(11.0).color(t.text_dim));
        return;
    };
    let source = info["styleSource"].as_u64().unwrap_or(0) as usize;
    let docs = info["documents"].as_array().cloned().unwrap_or_default();
    let count = docs.len();
    ui.separator();
    for (i, d) in docs.iter().enumerate() {
        book_chapter_row(app, ui, t, i, count, source, d);
    }
    ui.separator();
    book_options_bar(app, ui, &info);
    ui.separator();
    book_action_buttons(app, ui);
}

fn book_chapter_row(
    app: &mut DesignApp,
    ui: &mut egui::Ui,
    t: Tokens,
    i: usize,
    count: usize,
    source: usize,
    d: &serde_json::Value,
) {
    let path = d["path"].as_str().unwrap_or("").to_string();
    let name = std::path::Path::new(&path).file_stem().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let (first, pages) = (d["firstPage"].as_u64().unwrap_or(1), d["pages"].as_u64().unwrap_or(0));
    ui.horizontal(|ui| {
        if ui
            .selectable_label(i == source, if i == source { "◆" } else { "◇" })
            .on_hover_ui(|ui| {
                crate::rtl::label(ui, crate::i18n::tr(&app.ui.language, "Style source"));
            })
            .clicked()
        {
            let _ = app.run("book.styleSource", json!({"index": i}));
        }
        if ui
            .selectable_label(false, &name)
            .on_hover_ui(|ui| {
                crate::rtl::label(ui, crate::i18n::tr(&app.ui.language, "Open"));
            })
            .clicked()
        {
            let _ = app.run("file.open", json!({"path": path}));
        }
        crate::rtl::label(ui, egui::RichText::new(format!("{first}–{}", first + pages.saturating_sub(1))).size(10.5).color(t.text_dim));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.small_button(crate::rtl::widget(ui, crate::i18n::tr(&app.ui.language, "Remove"))).clicked() {
                let _ = app.run("book.remove", json!({"index": i}));
            }
            if ui.add_enabled(i + 1 < count, egui::Button::new("↓").small()).clicked() {
                let _ = app.run("book.move", json!({"from": i, "to": i + 1}));
            }
            if ui.add_enabled(i > 0, egui::Button::new("↑").small()).clicked() {
                let _ = app.run("book.move", json!({"from": i, "to": i - 1}));
            }
        });
    });
}

/// Scripts panel: saved command scripts — run (one undo step), edit, add, delete, load.
pub fn scripts(app: &mut DesignApp, ui: &mut egui::Ui) {
    let t = Tokens::get(ui.ctx());
    let sel_id = egui::Id::unique("scripts_sel");
    let mut sel: usize = ui.data(|d| d.get_temp(sel_id)).unwrap_or(0);
    if app.ui.scripts.is_empty() {
        crate::rtl::label(
            ui,
            crate::rtl::widget(ui, egui::RichText::new(crate::i18n::tr(&app.ui.language, "No scripts. Add one below.")).size(11.0).color(t.text_dim)),
        );
    }
    let mut run: Option<usize> = None;
    for (i, (name, _)) in app.ui.scripts.iter().enumerate() {
        let r = ui.selectable_label(i == sel, name);
        if r.clicked() {
            sel = i;
        }
        if r.double_clicked() {
            run = Some(i);
        }
    }
    ui.separator();
    ui.horizontal(|ui| {
        if ui.add_enabled(!app.ui.scripts.is_empty(), egui::Button::new(crate::rtl::widget(ui, crate::i18n::tr(&app.ui.language, "Run")))).clicked() {
            run = Some(sel);
        }
        if ui.button(crate::rtl::widget(ui, crate::i18n::tr(&app.ui.language, "New"))).clicked() {
            app.ui.scripts.push((format!("Script {}", app.ui.scripts.len() + 1), "# command.id {json} per line\n".into()));
            sel = app.ui.scripts.len() - 1;
        }
        if ui.button(crate::rtl::widget(ui, crate::i18n::tr(&app.ui.language, "Load…"))).clicked() {
            let asked = app.ask_open("script", Bound::App, move |app, path| {
                load_script(app, &path, sel_id);
                Ok(serde_json::Value::Null)
            });
            report(app, "Scripts", asked);
        }
        if ui
            .add_enabled(!app.ui.scripts.is_empty(), egui::Button::new(crate::rtl::widget(ui, crate::i18n::tr(&app.ui.language, "Delete"))))
            .clicked()
            && sel < app.ui.scripts.len()
        {
            app.ui.scripts.remove(sel);
            sel = sel.saturating_sub(1);
        }
    });
    if let Some((name, text)) = app.ui.scripts.get_mut(sel) {
        ui.add(egui::TextEdit::singleline(name).desired_width(f32::INFINITY));
        ui.add(egui::TextEdit::multiline(text).code_editor().desired_rows(8).desired_width(f32::INFINITY));
    }
    if let Some(i) = run
        && let Some((name, text)) = app.ui.scripts.get(i).cloned()
    {
        match app.run("script.run", json!({"text": text})) {
            Ok(r) => app.status(
                crate::i18n::tr(&app.ui.language, "{name}: {count} step(s) done")
                    .replace("{name}", &name)
                    .replace("{count}", &r["steps"].to_string()),
            ),
            Err(e) => app.status(format!("{name}: {e}")),
        }
    }
    ui.data_mut(|d| d.insert_temp(sel_id, sel));
}
