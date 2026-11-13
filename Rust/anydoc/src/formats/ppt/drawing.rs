//! Explicit OfficeArt table geometry and current-document hyperlink records.
//! Never infer a table from arbitrary aligned text boxes.
use super::children;
use crate::{
    model::LinkTarget,
    package::limits,
    shared::binary::{get_u32, utf16le_units},
};
use std::collections::HashMap;

pub(super) fn hyperlinks(document: &[u8]) -> HashMap<u32, Option<LinkTarget>> {
    let mut links = HashMap::new();
    for (_, _, objects) in children(document).filter(|&(_, t, _)| t == 0x0409) {
        for (_, _, link) in children(objects).filter(|&(_, t, _)| t == 0x0FD7) {
            let id = children(link)
                .find(|&(_, t, _)| t == 0x0FD3)
                .and_then(|(.., b)| get_u32(b, 0));
            let target = children(link)
                .find(|&(v, t, _)| t == 0x0FBA && v >> 4 == 1)
                .and_then(|(.., b)| String::from_utf16(&utf16le_units(b).collect::<Vec<_>>()).ok());
            if let Some(id) = id {
                let target = target
                    .filter(|s| {
                        !s.chars().any(char::is_control)
                            && s.split_once(':').is_some_and(|(scheme, rest)| {
                                !rest.is_empty()
                                    && ["http", "https", "mailto"]
                                        .iter()
                                        .any(|v| scheme.eq_ignore_ascii_case(v))
                            })
                    })
                    .map(LinkTarget::External);
                // Conflicting IDs are ambiguous; preserve their text without a link.
                if links.contains_key(&id) {
                    links.insert(id, None);
                } else {
                    links.insert(id, target);
                }
            }
        }
    }
    links
}

/// A flagged table's text-bearing shapes, in source order, plus a rectangular
/// grid mapping each covered position to its originating shape.
pub(super) struct TableLayout<'a> {
    pub shapes: Vec<&'a [u8]>,
    pub positions: Vec<(usize, usize, usize, usize)>,
    pub slots: Vec<usize>,
    pub rows: usize,
    pub cols: usize,
}

fn table_flag(shape: &[u8]) -> bool {
    children(shape)
        .filter(|&(_, t, _)| matches!(t, 0xF00B | 0xF121 | 0xF122))
        .any(|(v, _, props)| {
            (0..((v >> 4) as usize).min(props.len() / 6)).any(|i| {
                let at = i * 6;
                props
                    .get(at..at + 2)
                    .is_some_and(|b| u16::from_le_bytes([b[0], b[1]]) == 0x039F)
                    && get_u32(props, at + 2).is_some_and(|value| value & 1 != 0)
            })
        })
}

pub(super) fn is_table(group: &[u8]) -> bool {
    children(group)
        .next()
        .is_some_and(|(_, t, b)| t == 0xF004 && table_flag(b))
}

pub(super) fn table_layout(group: &[u8]) -> Option<TableLayout<'_>> {
    let mut shapes = Vec::new();
    let mut bounds = Vec::new();
    for (_, t, shape) in children(group).skip(1) {
        if t != 0xF004 {
            return None;
        }
        if !children(shape).any(|(_, t, _)| t == 0xF00D) {
            continue;
        }
        let (_, _, anchor) = children(shape).find(|&(_, t, _)| t == 0xF00F)?;
        let coord = |at| get_u32(anchor, at).map(|v| v as i32);
        let rect = [coord(0)?, coord(4)?, coord(8)?, coord(12)?];
        if rect[0] >= rect[2] || rect[1] >= rect[3] {
            return None;
        }
        // Nested groups/tables in a cell are not part of this simple grid.
        if children(shape).any(|(_, t, _)| t == 0xF003) {
            return None;
        }
        shapes.push(shape);
        bounds.push(rect);
    }
    if shapes.is_empty() {
        return None;
    }
    let mut xs: Vec<_> = bounds.iter().flat_map(|r| [r[0], r[2]]).collect();
    let mut ys: Vec<_> = bounds.iter().flat_map(|r| [r[1], r[3]]).collect();
    xs.sort_unstable();
    xs.dedup();
    ys.sort_unstable();
    ys.dedup();
    let cols = xs.len().checked_sub(1)?;
    let rows = ys.len().checked_sub(1)?;
    let count = rows.checked_mul(cols)?;
    if count == 0 || count as u64 > limits::MAX_GRID_SLOTS {
        return None;
    }
    let mut slots = vec![usize::MAX; count];
    let mut positions = Vec::new();
    for (index, rect) in bounds.iter().enumerate() {
        let left = xs.binary_search(&rect[0]).ok()?;
        let top = ys.binary_search(&rect[1]).ok()?;
        let right = xs.binary_search(&rect[2]).ok()?;
        let bottom = ys.binary_search(&rect[3]).ok()?;
        for row in top..bottom {
            for col in left..right {
                let slot = &mut slots[row * cols + col];
                if *slot != usize::MAX {
                    return None;
                }
                *slot = index;
            }
        }
        positions.push((top, left, bottom - top, right - left));
    }
    // Gaps/overlaps mean the geometry is not an unambiguous complete table.
    if slots.contains(&usize::MAX) {
        return None;
    }
    Some(TableLayout {
        shapes,
        positions,
        slots,
        rows,
        cols,
    })
}
