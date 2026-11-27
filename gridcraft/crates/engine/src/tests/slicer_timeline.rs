//! End-to-end engine tests for PivotTable Slicers and Timeline date-range filters, including
//! placed and unplaced fields, granularity switching (`years`, `quarters`, `months`, `days`),
//! Undo/Redo, and XLSX save/reopen round-trip.

use gridcraft_core::{CellRef, Value};
use gridcraft_model::TimelineLevel;
use serde_json::json;

use super::{s, v};
use crate::Session;

/// Grand Total cell of `SalesPivot` anchored at `F1` (or scans `F1:G10` for the `"Grand Total"` row).
fn grand_total(s: &Session) -> f64 {
    let sh = s.doc().unwrap().wb.active().unwrap();
    for r in 0..15 {
        if sh.value(CellRef::new(r, 5)) == Value::text("Grand Total") {
            if let Value::Number(n) = sh.value(CellRef::new(r, 6)) {
                return n;
            }
        }
    }
    // Single-value PivotTable with no row field puts header at F1 and sum at F2.
    match sh.value(CellRef::new(1, 5)) {
        Value::Number(n) => n,
        other => panic!("expected numeric total, got {other:?}"),
    }
}

#[test]
fn pivot_slicer_filters_placed_and_unplaced_fields_with_undo_redo() {
    let mut s = s();
    s.execute(
        "range.setValues",
        json!({
            "range": "A1",
            "values": [
                ["Date", "Region", "Category", "Sales"],
                ["2024-01-10", "East", "Hardware", 100],
                ["2024-02-15", "East", "Software", 50],
                ["2024-03-20", "West", "Hardware", 200],
                ["2024-05-05", "North", "Software", 300],
            ]
        }),
    )
    .unwrap();

    // Rows: Region (placed). Category is NOT placed on Rows/Columns/Filters initially.
    s.execute(
        "insert.pivotTable",
        json!({
            "source": "A1:D5",
            "destination": "F1",
            "name": "SalesPivot",
            "rows": ["Region"],
            "values": ["Sales"]
        }),
    )
    .unwrap();
    assert_eq!(grand_total(&s), 650.0);

    // Insert slicers for both Region (placed) and Category (unplaced) in one call via `fields`.
    let res = s
        .execute(
            "insert.slicer",
            json!({
                "pivot": "SalesPivot",
                "fields": ["Region", "Category"],
                "at": "I2"
            }),
        )
        .unwrap();
    let ids: Vec<u32> = res["slicers"].as_array().unwrap().iter().map(|x| x.as_u64().unwrap() as u32).collect();
    assert_eq!(ids.len(), 2);
    let (reg_slicer, cat_slicer) = (ids[0], ids[1]);

    // Filter Region to East via `slicer.toggleItem`.
    s.execute("slicer.toggleItem", json!({"slicer": reg_slicer, "item": "East"})).unwrap();
    assert_eq!(grand_total(&s), 150.0);

    // Enable `multiSelect` on Region slicer and toggle West in -> East + West = 350.
    s.execute("slicer.update", json!({"slicer": reg_slicer, "multiSelect": true, "columns": 2, "caption": "Sales Regions"})).unwrap();
    s.execute("slicer.toggle", json!({"slicer": reg_slicer, "value": "West"})).unwrap();
    assert_eq!(grand_total(&s), 350.0);

    // Now filter unplaced Category slicer to Hardware -> East(100) + West(200) = 300, without
    // shifting PivotTable report anchor rows!
    let anchor_before = s.doc().unwrap().wb.active().unwrap().pivots[0].last_range.unwrap().start;
    s.execute("slicer.select", json!({"slicer": cat_slicer, "values": ["Hardware"]})).unwrap();
    let anchor_after = s.doc().unwrap().wb.active().unwrap().pivots[0].last_range.unwrap().start;
    assert_eq!(anchor_before, anchor_after, "unplaced slicer filter must not shift report rows");
    assert_eq!(grand_total(&s), 300.0);

    // Undo Category filter -> back to 350; Redo -> back to 300.
    s.execute("edit.undo", json!({})).unwrap();
    assert_eq!(grand_total(&s), 350.0);
    s.execute("edit.redo", json!({})).unwrap();
    assert_eq!(grand_total(&s), 300.0);

    // Clear Region slicer -> Hardware across all regions = East(100) + West(200) = 300;
    // Clear Category slicer -> all rows = 650.
    s.execute("slicer.clear", json!({"slicer": reg_slicer})).unwrap();
    assert_eq!(grand_total(&s), 300.0);
    s.execute("slicer.clear", json!({"slicer": cat_slicer})).unwrap();
    assert_eq!(grand_total(&s), 650.0);

    // Select North only, then delete slicer -> deleting cleans up filter and restores 650,
    // and Undo brings back both the slicer and the 300 filtered total!
    s.execute("slicer.select", json!({"slicer": reg_slicer, "values": ["North"]})).unwrap();
    assert_eq!(grand_total(&s), 300.0);
    s.execute("slicer.delete", json!({"slicer": reg_slicer})).unwrap();
    assert_eq!(grand_total(&s), 650.0);
    s.execute("edit.undo", json!({})).unwrap();
    assert_eq!(grand_total(&s), 300.0);
}

#[test]
fn timeline_filters_pivot_across_years_quarters_months_days_with_undo_redo_and_xlsx() {
    let mut s = s();
    s.execute(
        "range.setValues",
        json!({
            "range": "A1",
            "values": [
                ["OrderDate", "Region", "Revenue"],
                ["2023-11-15", "East", 40],
                ["2024-01-10", "East", 100],
                ["2024-02-20", "West", 150],
                ["2024-05-12", "North", 200],
                ["2024-08-03", "West", 300],
            ]
        }),
    )
    .unwrap();

    s.execute(
        "insert.pivotTable",
        json!({
            "source": "A1:C6",
            "destination": "F1",
            "name": "RevPivot",
            "rows": ["Region"],
            "values": ["Revenue"]
        }),
    )
    .unwrap();
    assert_eq!(grand_total(&s), 790.0);

    let out = s
        .execute(
            "insert.timeline",
            json!({
                "pivot": "RevPivot",
                "field": "OrderDate",
                "level": "quarters",
                "at": "I2",
                "caption": "Order Period"
            }),
        )
        .unwrap();
    let tl_id = out["timeline"].as_u64().unwrap() as u32;

    // Verify period generation across all four granularities.
    for (lv_str, expected_lv, min_len) in [
        ("years", TimelineLevel::Years, 2),       // 2023, 2024
        ("quarters", TimelineLevel::Quarters, 4), // 2023 Q4 .. 2024 Q3
        ("months", TimelineLevel::Months, 10),    // Nov 2023 .. Aug 2024
        ("days", TimelineLevel::Days, 200),       // ~263 days
    ] {
        s.execute("timeline.setLevel", json!({"timeline": tl_id, "level": lv_str})).unwrap();
        let d = s.doc().unwrap();
        let tl = d.wb.active().unwrap().timelines.iter().find(|t| t.id == tl_id).unwrap();
        assert_eq!(tl.level, expected_lv);
        let periods = crate::cmd::slicer::timeline_periods(&d.wb, tl);
        assert!(periods.len() >= min_len, "{lv_str}: got {} periods", periods.len());
    }

    // Filter to Q1 2024 (2024-01-01 .. 2024-03-31) -> 100 + 150 = 250.
    s.execute("timeline.setLevel", json!({"timeline": tl_id, "level": "quarters"})).unwrap();
    s.execute("timeline.setRange", json!({"timeline": tl_id, "start": "2024-01-01", "end": "2024-03-31"})).unwrap();
    assert_eq!(grand_total(&s), 250.0);

    // Filter to single month May 2024 -> 200.
    s.execute("timeline.setRange", json!({"timeline": tl_id, "start": "2024-05-01", "end": "2024-05-31"})).unwrap();
    assert_eq!(grand_total(&s), 200.0);

    // Undo -> back to Q1 2024 (250); Redo -> back to May 2024 (200).
    s.execute("edit.undo", json!({})).unwrap();
    assert_eq!(grand_total(&s), 250.0);
    s.execute("edit.redo", json!({})).unwrap();
    assert_eq!(grand_total(&s), 200.0);

    // Save as XLSX and reopen in Session: timeline range & filtered PivotTable cells survive!
    let saved = s.execute("file.saveBytes", json!({"format": "xlsx"})).unwrap();
    s.execute("file.open", json!({"name": "timeline_rt.xlsx", "base64": saved["base64"]})).unwrap();
    assert_eq!(grand_total(&s), 200.0);
    let reopened_tl = s.doc().unwrap().wb.active().unwrap().timelines[0].clone();
    assert_eq!(reopened_tl.caption, "Order Period");
    assert_eq!(reopened_tl.level, TimelineLevel::Quarters);
    assert_eq!(reopened_tl.start_date.as_deref(), Some("2024-05-01"));
    assert_eq!(reopened_tl.end_date.as_deref(), Some("2024-05-31"));

    // Clear timeline filter on the reopened workbook -> back to full 790!
    s.execute("timeline.clear", json!({"timeline": reopened_tl.id})).unwrap();
    assert_eq!(grand_total(&s), 790.0);
    assert_eq!(v(&s, "G5"), Value::number(790.0));
}
