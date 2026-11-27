use gridcraft_core::{CellRef, RangeRef, Value};
use gridcraft_model::{
    Anchor, AnchorMode, Cell, PivotDateFilter, PivotField, PivotFilter, PivotFunc, PivotTable, PivotValue, Slicer, Table, TableColumn, Timeline,
    TimelineLevel, TotalsFn, Workbook,
};

use crate::{read_xlsx, write_xlsx};

fn zip_part_names(bytes: &[u8]) -> Vec<String> {
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut out = Vec::with_capacity(z.len());
    for i in 0..z.len() {
        out.push(z.by_index(i).unwrap().name().to_string());
    }
    out
}

#[test]
fn workbook_without_slicers_or_timelines_emits_no_slicer_parts() {
    let wb = Workbook::new();
    let bytes = write_xlsx(&wb).unwrap();
    let names = zip_part_names(&bytes);
    assert!(!names.iter().any(|n| n.contains("slicer") || n.contains("timeline")), "{names:?}");
}

#[test]
fn table_slicer_roundtrips_through_xlsx() {
    let mut wb = Workbook::new();
    {
        let sh = wb.sheet_mut(0).unwrap();
        sh.cells.set(CellRef::new(0, 0), Cell::value(Value::text("Region")));
        sh.cells.set(CellRef::new(0, 1), Cell::value(Value::text("Amount")));
        sh.cells.set(CellRef::new(1, 0), Cell::value(Value::text("East")));
        sh.cells.set(CellRef::new(1, 1), Cell::value(Value::number(10.0)));
        sh.cells.set(CellRef::new(2, 0), Cell::value(Value::text("West")));
        sh.cells.set(CellRef::new(2, 1), Cell::value(Value::number(20.0)));
        sh.cells.set(CellRef::new(3, 0), Cell::value(Value::text("North")));
        sh.cells.set(CellRef::new(3, 1), Cell::value(Value::number(30.0)));
        sh.tables.push(Table {
            id: 1,
            name: "SalesTable".into(),
            range: RangeRef::parse("A1:B4").unwrap(),
            header_row: true,
            totals_row: false,
            columns: vec![
                TableColumn { name: "Region".into(), totals: TotalsFn::None, totals_label: None, formula: None, unique_name: None, xml: None },
                TableColumn { name: "Amount".into(), totals: TotalsFn::None, totals_label: None, formula: None, unique_name: None, xml: None },
            ],
            style: "TableStyleMedium2".into(),
            banded_rows: true,
            banded_cols: false,
            first_col: false,
            last_col: false,
            filter_button: true,
            filter: vec![],
            table_type: None,
            connection_id: None,
        });
        sh.slicers.push(Slicer {
            id: 7,
            name: "Slicer_Region".into(),
            table: "SalesTable".into(),
            pivots: vec![],
            column: "Region".into(),
            caption: "Filter Region".into(),
            anchor: Anchor { cell: CellRef::parse("D2").unwrap(), dx: 6.0, dy: 8.0, width: 180.0, height: 210.0, mode: AnchorMode::MoveOnly },
            selected: Some(vec!["East".into(), "North".into()]),
            columns: 2,
            multi_select: true,
            style: "SlicerStyleLight2".into(),
        });
    }

    let bytes = write_xlsx(&wb).unwrap();
    let names = zip_part_names(&bytes);
    assert!(names.contains(&"xl/slicers/slicer1.xml".to_string()), "{names:?}");
    assert!(names.contains(&"xl/slicerCaches/slicerCache1.xml".to_string()), "{names:?}");

    let (reopened, report) = read_xlsx(&bytes).unwrap();
    assert!(report.warnings.is_empty(), "{:?}", report.warnings);
    let sh = reopened.sheet(0).unwrap();
    assert_eq!(sh.slicers.len(), 1);
    let sl = &sh.slicers[0];
    assert_eq!(sl.table, "SalesTable");
    assert!(!sl.is_pivot());
    assert_eq!(sl.column, "Region");
    assert_eq!(sl.caption, "Filter Region");
    assert_eq!(sl.selected, Some(vec!["East".into(), "North".into()]));
    assert_eq!(sl.columns, 2);
    assert!(sl.multi_select);
    assert_eq!(sl.style, "SlicerStyleLight2");
    assert_eq!(sl.anchor.cell, CellRef::parse("D2").unwrap());
    assert!((sl.anchor.width - 180.0).abs() < 1.0);
    assert!((sl.anchor.height - 210.0).abs() < 1.0);
}

#[test]
fn pivot_slicer_and_timeline_roundtrip_through_xlsx() {
    let mut wb = Workbook::new();
    {
        let sh = wb.sheet_mut(0).unwrap();
        for (r, (dt, reg, amt)) in
            [("2024-01-15", "East", 100.0), ("2024-04-20", "West", 250.0), ("2024-08-10", "North", 400.0)].into_iter().enumerate()
        {
            if r == 0 {
                sh.cells.set(CellRef::new(0, 0), Cell::value(Value::text("OrderDate")));
                sh.cells.set(CellRef::new(0, 1), Cell::value(Value::text("Region")));
                sh.cells.set(CellRef::new(0, 2), Cell::value(Value::text("Sales")));
            }
            let row = (r + 1) as u32;
            sh.cells.set(CellRef::new(row, 0), Cell::value(Value::text(dt)));
            sh.cells.set(CellRef::new(row, 1), Cell::value(Value::text(reg)));
            sh.cells.set(CellRef::new(row, 2), Cell::value(Value::number(amt)));
        }

        sh.pivots.push(PivotTable {
            id: 1,
            name: "SalesPivot".into(),
            source: "Sheet1!$A$1:$C$4".into(),
            anchor: CellRef::parse("E2").unwrap(),
            rows: vec![PivotField { source_col: "Region".into(), ..PivotField::default() }],
            columns: vec![],
            values: vec![PivotValue { source_col: "Sales".into(), func: PivotFunc::Sum, name: "Sum of Sales".into(), ..PivotValue::default() }],
            filters: vec![PivotFilter { source_col: "Region".into(), selected: Some(vec!["East".into(), "North".into()]) }],
            date_filters: vec![PivotDateFilter {
                source_col: "OrderDate".into(),
                start_date: Some("2024-01-01".into()),
                end_date: Some("2024-06-30".into()),
            }],
            last_range: Some(RangeRef::parse("E2:F5").unwrap()),
            ..PivotTable::default()
        });

        sh.slicers.push(Slicer {
            id: 10,
            name: "Slicer_Region".into(),
            table: String::new(),
            pivots: vec!["SalesPivot".into()],
            column: "Region".into(),
            caption: "Sales Region".into(),
            anchor: Anchor { cell: CellRef::parse("H2").unwrap(), dx: 4.0, dy: 4.0, width: 172.0, height: 196.0, mode: AnchorMode::MoveOnly },
            selected: Some(vec!["East".into(), "North".into()]),
            columns: 1,
            multi_select: true,
            style: "SlicerStyleDark1".into(),
        });

        sh.timelines.push(Timeline {
            id: 11,
            name: "Timeline_OrderDate".into(),
            caption: "Order Timeline".into(),
            pivots: vec!["SalesPivot".into()],
            field: "OrderDate".into(),
            level: TimelineLevel::Quarters,
            start_date: Some("2024-01-01".into()),
            end_date: Some("2024-06-30".into()),
            style: "TimeSlicerStyleLight2".into(),
            anchor: Anchor { cell: CellRef::parse("K2").unwrap(), dx: 4.0, dy: 4.0, width: 320.0, height: 108.0, mode: AnchorMode::MoveOnly },
        });
    }

    let bytes = write_xlsx(&wb).unwrap();
    let names = zip_part_names(&bytes);
    assert!(names.contains(&"xl/slicers/slicer1.xml".to_string()), "{names:?}");
    assert!(names.contains(&"xl/slicerCaches/slicerCache1.xml".to_string()), "{names:?}");
    assert!(names.contains(&"xl/timelines/timeline1.xml".to_string()), "{names:?}");
    assert!(names.contains(&"xl/timelineCaches/timelineCache1.xml".to_string()), "{names:?}");

    let (reopened, report) = read_xlsx(&bytes).unwrap();
    assert!(report.warnings.is_empty(), "{:?}", report.warnings);
    let sh = reopened.sheet(0).unwrap();
    assert_eq!(sh.slicers.len(), 1);
    assert_eq!(sh.timelines.len(), 1);

    let sl = &sh.slicers[0];
    assert!(sl.is_pivot());
    assert_eq!(sl.pivots, vec!["SalesPivot".to_string()]);
    assert_eq!(sl.column, "Region");
    assert_eq!(sl.caption, "Sales Region");
    assert_eq!(sl.selected, Some(vec!["East".into(), "North".into()]));
    assert!(sl.multi_select);
    assert_eq!(sl.style, "SlicerStyleDark1");
    assert_eq!(sl.anchor.cell, CellRef::parse("H2").unwrap());
    assert!((sl.anchor.width - 172.0).abs() < 1.0);
    assert!((sl.anchor.height - 196.0).abs() < 1.0);

    let tl = &sh.timelines[0];
    assert_eq!(tl.pivots, vec!["SalesPivot".to_string()]);
    assert_eq!(tl.field, "OrderDate");
    assert_eq!(tl.caption, "Order Timeline");
    assert_eq!(tl.level, TimelineLevel::Quarters);
    assert_eq!(tl.start_date.as_deref(), Some("2024-01-01"));
    assert_eq!(tl.end_date.as_deref(), Some("2024-06-30"));
    assert_eq!(tl.style, "TimeSlicerStyleLight2");
    assert_eq!(tl.anchor.cell, CellRef::parse("K2").unwrap());
    assert!((tl.anchor.width - 320.0).abs() < 1.0);
    assert!((tl.anchor.height - 108.0).abs() < 1.0);

    let pt = &sh.pivots[0];
    assert_eq!(
        pt.date_filters,
        vec![PivotDateFilter { source_col: "OrderDate".into(), start_date: Some("2024-01-01".into()), end_date: Some("2024-06-30".into()) }]
    );
    assert_eq!(pt.filters, vec![PivotFilter { source_col: "Region".into(), selected: Some(vec!["East".into(), "North".into()]) }]);
}
