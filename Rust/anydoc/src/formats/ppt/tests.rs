use super::*;
use crate::model::{CellSlot, inlines_to_plain_text};

fn record(t: u16, vi: u16, body: &[u8]) -> Vec<u8> {
    [
        vi.to_le_bytes().as_slice(),
        t.to_le_bytes().as_slice(),
        (body.len() as u32).to_le_bytes().as_slice(),
        body,
    ]
    .concat()
}
fn container(t: u16, body: Vec<u8>) -> Vec<u8> {
    record(t, 15, &body)
}
fn text(value: &str) -> Vec<u8> {
    [
        record(0x0F9F, 0, &[4, 0, 0, 0]),
        record(
            0x0FA0,
            0,
            &value
                .encode_utf16()
                .flat_map(u16::to_le_bytes)
                .collect::<Vec<_>>(),
        ),
    ]
    .concat()
}
fn cell(rect: [i32; 4], value: &str) -> Vec<u8> {
    container(
        0xF004,
        [
            record(
                0xF00F,
                0,
                &rect
                    .into_iter()
                    .flat_map(i32::to_le_bytes)
                    .collect::<Vec<_>>(),
            ),
            container(0xF00D, text(value)),
        ]
        .concat(),
    )
}
fn table(cells: Vec<Vec<u8>>) -> Vec<u8> {
    let flag = container(0xF004, record(0xF122, 3 | (1 << 4), &[0x9f, 3, 1, 0, 0, 0]));
    container(0xF003, [vec![flag], cells].concat().concat())
}
fn extract(bytes: &[u8]) -> Extractor {
    let mut ex = Extractor::default();
    ex.walk(bytes).unwrap();
    ex.flush_shape();
    ex
}
fn hyperlink(id: u32, target: &str) -> Vec<u8> {
    container(
        0x0FD7,
        [
            record(0x0FD3, 0, &id.to_le_bytes()),
            record(
                0x0FBA,
                1 << 4,
                &target
                    .encode_utf16()
                    .flat_map(u16::to_le_bytes)
                    .collect::<Vec<_>>(),
            ),
        ]
        .concat(),
    )
}
fn action(id: u32, start: u32, end: u32) -> Vec<u8> {
    let mut atom = vec![0; 16];
    atom[4..8].copy_from_slice(&id.to_le_bytes());
    atom[8] = 4;
    atom[12] = 8;
    [
        container(0x0FF2, record(0x0FF3, 0, &atom)),
        record(
            0x0FDF,
            0,
            &[start.to_le_bytes(), end.to_le_bytes()].concat(),
        ),
    ]
    .concat()
}

#[test]
fn explicitly_flagged_grid_preserves_row_column_order_and_infers_header() {
    // Deliberately stored in non-row order; geometry, not stream order, wins.
    let bytes = table(vec![
        cell([10, 10, 20, 20], "12"),
        cell([0, 0, 10, 10], "Fruit"),
        cell([0, 10, 10, 20], "Apple"),
        cell([10, 0, 20, 10], "Quantity"),
    ]);
    let ex = extract(&bytes);
    let [Block::Table(t)] = ex.current.as_slice() else {
        panic!("{:?}", ex.current);
    };
    assert_eq!(t.header_rows, 1);
    assert_eq!(t.grid.len(), 2);
    let CellSlot::Origin(c) = &t.grid[1][1] else {
        panic!()
    };
    assert!(matches!(&c.blocks[0], Block::Paragraph(v) if inlines_to_plain_text(v) == "12"));
    assert!(ex.diagnostics.is_empty());
}
#[test]
fn merged_cells_have_one_origin_and_covered_positions() {
    let ex = extract(&table(vec![
        cell([0, 0, 20, 10], "Shared title"),
        cell([0, 10, 10, 20], "A"),
        cell([10, 10, 20, 20], "B"),
    ]));
    let Block::Table(t) = &ex.current[0] else {
        panic!()
    };
    assert_eq!(t.header_rows, 0);
    assert!(matches!(&t.grid[0][0], CellSlot::Origin(c) if c.col_span == 2));
    assert!(matches!(t.grid[0][1], CellSlot::Covered { .. }));
}
#[test]
fn overlaps_and_gaps_preserve_text_with_a_notice_instead_of_guessing() {
    for second in [[5, 0, 15, 10], [20, 0, 30, 10]] {
        let ex = extract(&table(vec![
            cell([0, 0, 10, 10], "First"),
            cell(second, "Second"),
        ]));
        assert_eq!(ex.current.len(), 2);
        assert!(ex.current.iter().all(|b| matches!(b, Block::Paragraph(_))));
        assert_eq!(ex.diagnostics[0].code, "presentation.tableFlattened");
    }
}
#[test]
fn ordinary_groups_are_not_promoted_to_tables() {
    let bytes = container(
        0xF003,
        [cell([0, 0, 10, 10], "A"), cell([10, 0, 20, 10], "B")].concat(),
    );
    let ex = extract(&bytes);
    assert_eq!(ex.current.len(), 2);
    assert!(ex.diagnostics.is_empty());
}
#[test]
fn links_use_current_document_targets_and_utf16_ranges_across_paragraphs() {
    let doc = container(0x0409, hyperlink(21, "https://example.org/fruit"));
    // hyperlinks expects a DocumentContainer body, including ExObjList.
    let mut ex = Extractor {
        hyperlinks: drawing::hyperlinks(&doc),
        ..Default::default()
    };
    let source = "Visit 🍎 fruit\rAgain fruit";
    ex.walk(&container(
        0xF004,
        [container(
            0xF00D,
            [text(source), action(21, 6, 14), action(21, 21, 26)].concat(),
        )]
        .concat(),
    ))
    .unwrap();
    ex.flush_shape();
    assert_eq!(ex.current.len(), 2);
    let labels: Vec<_> = ex
        .current
        .iter()
        .flat_map(|b| match b {
            Block::Paragraph(v) => v.as_slice(),
            _ => &[],
        })
        .filter_map(|i| match i {
            Inline::Link { content, .. } => Some(inlines_to_plain_text(content)),
            _ => None,
        })
        .collect();
    assert_eq!(labels, ["🍎 fruit", "fruit"]);
    assert!(ex.diagnostics.is_empty());
}
#[test]
fn invalid_overlapping_unsafe_and_missing_links_keep_all_text() {
    for (target, start, end) in [
        (Some("https://example.org"), 1, 2), // inside emoji surrogate pair
        (Some("https://example.org"), 0, 200),
        (Some("javascript:alert(1)"), 0, 2),
        (None, 0, 2),
    ] {
        let mut ex = Extractor::default();
        if let Some(target) = target {
            ex.hyperlinks = drawing::hyperlinks(&container(0x0409, hyperlink(1, target)));
        }
        ex.walk(&container(
            0xF004,
            container(0xF00D, [text("🍎 fruit"), action(1, start, end)].concat()),
        ))
        .unwrap();
        ex.flush_shape();
        let Block::Paragraph(v) = &ex.current[0] else {
            panic!()
        };
        assert_eq!(inlines_to_plain_text(v), "🍎 fruit");
        assert!(!v.iter().any(|i| matches!(i, Inline::Link { .. })));
        assert_eq!(ex.diagnostics[0].code, "presentation.linkUnavailable");
    }
}
#[test]
fn duplicate_ids_are_not_assigned_arbitrarily_and_shapes_do_not_share_actions() {
    let map = drawing::hyperlinks(&container(
        0x0409,
        [
            hyperlink(1, "https://one.org"),
            hyperlink(1, "https://two.org"),
        ]
        .concat(),
    ));
    assert!(map[&1].is_none());
    let mut ex = Extractor::default();
    ex.walk(
        &[
            container(0xF004, container(0xF00D, text("First"))),
            container(0xF004, action(1, 0, 5)),
        ]
        .concat(),
    )
    .unwrap();
    ex.flush_shape();
    let Block::Paragraph(v) = &ex.current[0] else {
        panic!()
    };
    assert_eq!(inlines_to_plain_text(v), "First");
    assert!(
        ex.diagnostics
            .iter()
            .any(|d| d.code == "presentation.linkUnavailable")
    );
}
