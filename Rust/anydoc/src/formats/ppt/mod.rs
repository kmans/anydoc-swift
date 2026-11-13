//! Legacy PowerPoint 97-2003 binary (.ppt): OLE2 container, record stream.
//! Slides resolve through the persist directory (the only default path);
//! text comes from TextHeaderAtom + TextCharsAtom/TextBytesAtom with
//! StyleTextPropAtom runs and TxMasterStyleAtom defaults applied. Raw
//! stream-order scanning exists only as an explicitly labelled recovery for
//! files whose persist directory is unusable. Speaker notes are included
//! (fixed policy), rendered as a quote after their slide.

mod drawing;
mod styletext;
#[cfg(test)]
mod tests;

use crate::error::ConvertError;
use crate::model::{
    Block, Cell, Diagnostic, Document, GridBuilder, Inline, LinkTarget, SourceLocation, Style,
    TableKind, inlines_are_empty,
};
use crate::package::limits;
use crate::shared::binary::{get_u32, read_ole_stream, utf16le_units};
use crate::shared::delta::{StyleDelta, rebase_emphasis};
use crate::shared::list::{ListEntry, ListKey, MarkerKind, flush_list};
use crate::shared::officeart::record_at;
use crate::shared::text::clean_text;
use std::collections::HashMap;
use std::io::Cursor;
use styletext::{CharProps, MasterLevel, StyleRuns};

/// One master's per-text-type level defaults, keyed by TxMasterStyleAtom
/// instance (the text type).
type MasterStyles = HashMap<u16, Vec<MasterLevel>>;

pub fn parse(bytes: &[u8]) -> Result<Document, ConvertError> {
    let cursor = Cursor::new(bytes);
    let mut ole = cfb::CompoundFile::open(cursor)
        .map_err(|e| ConvertError::malformed(format!("not an OLE2 compound file: {e}")))?;
    let data = read_ole_stream(&mut ole, "PowerPoint Document")?;
    let current_user = read_ole_stream(&mut ole, "Current User").unwrap_or_default();
    if get_u32(&current_user, 12) == Some(0xF3D1_C4DF) {
        return Err(ConvertError::Encrypted);
    }

    let mut ex = Extractor::default();
    if !ex.parse_slides(&data, &current_user)? {
        // Labelled recovery path: the persist directory is unusable, so text
        // is taken in raw stream order (may include superseded edits).
        log::warn!("ppt persist directory unusable; recovering text in raw stream order");
        ex = Extractor::default();
        ex.recovering = true;
        ex.walk(&data)?;
        ex.end_segment(None);
    }
    if ex.encrypted {
        return Err(ConvertError::Encrypted);
    }
    let assets = collect_pictures(&mut ole)?;
    let diagnostics = std::mem::take(&mut ex.diagnostics);
    Ok(Document {
        blocks: ex.into_blocks(),
        notes: Vec::new(),
        assets,
        diagnostics,
    })
}

/// Retain the deck's embedded pictures from the `Pictures` stream (OfficeArt
/// BStore file blocks). Pictures are document-level assets; per-slide
/// placement is not resolved. Unsupported formats degrade with a log.
fn collect_pictures<R: std::io::Read + std::io::Seek>(
    ole: &mut cfb::CompoundFile<R>,
) -> Result<Vec<crate::model::Asset>, ConvertError> {
    use crate::shared::officeart;
    let Ok(pictures) = read_ole_stream(ole, "Pictures") else {
        return Ok(Vec::new());
    };
    let mut sink = crate::shared::assets::AssetSink::new();
    let mut pos = 0usize;
    let mut index = 0u32;
    while let Some((ver_inst, rec_type, body)) = officeart::record_at(&pictures, pos) {
        pos += 8 + body.len();
        index += 1;
        if index > 100_000 {
            break;
        }
        let cap = limits::MAX_ENTRY_BYTES as usize;
        let blip = match rec_type {
            0xF007 => officeart::fbse_blip(body, cap),
            _ => officeart::decode_blip(ver_inst, rec_type, body, cap),
        };
        match blip {
            Some(blip) => {
                sink.add(
                    blip.media_type.to_string(),
                    format!("pictures/{index}.{}", blip.extension),
                    &blip.bytes,
                )?;
            }
            None => log::debug!("skipping unsupported Pictures record 0x{rec_type:04X}"),
        }
    }
    Ok(sink.assets)
}

/// Iterate the records laid out back to back in `data`.
fn children(data: &[u8]) -> impl Iterator<Item = (u16, u16, &[u8])> {
    let mut pos = 0;
    std::iter::from_fn(move || {
        let (ver_inst, rec_type, body) = record_at(data, pos)?;
        pos += 8 + body.len();
        Some((ver_inst, rec_type, body))
    })
}

fn push_run(inlines: &mut Vec<Inline>, text: String, style: Style, target: Option<LinkTarget>) {
    let text = Inline::Text { text, style };
    if let Some(target) = target {
        if let Some(Inline::Link {
            content,
            target: previous,
        }) = inlines.last_mut()
            && *previous == target
        {
            content.push(text);
            return;
        }
        inlines.push(Inline::Link {
            content: vec![text],
            target,
        });
    } else {
        inlines.push(text);
    }
}

/// A text shape being accumulated: header type, text, then styling.
struct PendingShape {
    tx_type: u8,
    text: String,
    styles: Option<StyleRuns>,
    links: Vec<(usize, usize, Option<LinkTarget>)>,
    interactive: Option<Option<LinkTarget>>,
}

#[derive(Default)]
struct Extractor {
    /// Finished segments: (blocks, pairing id, is_notes).
    segments: Vec<(Vec<Block>, Option<u32>, bool)>,
    current: Vec<Block>,
    current_is_notes: bool,
    list_run: Vec<ListEntry>,
    pending: Option<PendingShape>,
    /// Master style tables in master-list order: (masterId, styles).
    masters: Vec<(u32, MasterStyles)>,
    /// Index into `masters` for the slide being extracted (0 fallback).
    active_master: usize,
    shape_counter: u64,
    encrypted: bool,
    /// Raw-stream recovery: no persist lists, so notes descend inline.
    recovering: bool,
    /// Records visited across the whole extraction, capped.
    records: u64,
    walk_depth: usize,
    hyperlinks: HashMap<u32, Option<LinkTarget>>,
    diagnostics: Vec<Diagnostic>,
}

/// The persist-resolved layout of the presentation: slide/notes lists from
/// the current DocumentContainer, persist id -> offset for every container.
struct DocLayout<'a> {
    document: &'a [u8],
    persist: HashMap<u32, usize>,
    slide_list: &'a [u8],
    notes_list: Option<&'a [u8]>,
    master_list: Option<&'a [u8]>,
}

/// Resolve the UserEditAtom chain into the persist directory and find the
/// DocumentContainer's SlideListWithText instances. `None` means the persist
/// directory is unusable and the caller falls back to raw-order recovery.
fn locate_document<'a>(data: &'a [u8], current_user: &[u8]) -> Option<DocLayout<'a>> {
    let mut persist: HashMap<u32, usize> = HashMap::new();
    let mut doc_persist: Option<u32> = None;
    let mut edit_off = get_u32(current_user, 16)? as usize;
    for _ in 0..100 {
        if edit_off == 0 {
            break;
        }
        let (_, rec_type, body) = record_at(data, edit_off)?;
        if rec_type != 0x0FF5 {
            return None;
        }
        if doc_persist.is_none() {
            doc_persist = get_u32(body, 16);
        }
        let dir_off = get_u32(body, 12)? as usize;
        if let Some((_, 0x1772, dir)) = record_at(data, dir_off) {
            let mut pos = 0;
            while pos + 4 <= dir.len() {
                let head = get_u32(dir, pos)?;
                let id = head & 0xF_FFFF;
                let count = (head >> 20) as usize;
                pos += 4;
                for k in 0..count {
                    // Newer edits win: keep the first offset seen.
                    persist
                        .entry(id + k as u32)
                        .or_insert(get_u32(dir, pos)? as usize);
                    pos += 4;
                }
            }
        }
        let prev = get_u32(body, 8)? as usize;
        if prev == edit_off {
            break;
        }
        edit_off = prev;
    }

    let doc_off = *persist.get(&doc_persist?)?;
    let Some((_, 0x03E8, doc)) = record_at(data, doc_off) else {
        return None;
    };
    let (.., slide_list) =
        children(doc).find(|&(ver_inst, rec_type, _)| rec_type == 0x0FF0 && ver_inst >> 4 == 0)?;
    let notes_list = children(doc)
        .find(|&(ver_inst, rec_type, _)| rec_type == 0x0FF0 && ver_inst >> 4 == 2)
        .map(|(.., body)| body);
    let master_list = children(doc)
        .find(|&(ver_inst, rec_type, _)| rec_type == 0x0FF0 && ver_inst >> 4 == 1)
        .map(|(.., body)| body);
    Some(DocLayout {
        document: doc,
        persist,
        slide_list,
        notes_list,
        master_list,
    })
}

/// One master's TxMasterStyleAtoms, keyed by text-type instance.
fn master_styles(master: &[u8]) -> MasterStyles {
    let mut styles = MasterStyles::new();
    for (ver_inst, rec_type, body) in children(master) {
        if rec_type == 0x0FA3 {
            let instance = ver_inst >> 4;
            styles
                .entry(instance)
                .or_insert_with(|| styletext::parse_master_style(body, instance));
        }
    }
    styles
}

/// Masters in master-list order (MasterPersistAtoms: persistIdRef at 0,
/// masterId at 12); falls back to a persist-directory scan when the list is
/// absent so single-master decks still get their defaults.
fn collect_masters(
    master_list: Option<&[u8]>,
    persist: &HashMap<u32, usize>,
    data: &[u8],
) -> Vec<(u32, MasterStyles)> {
    let mut out = Vec::new();
    if let Some(list) = master_list {
        for (_, rec_type, body) in children(list) {
            if rec_type != 0x03F3 {
                continue;
            }
            let (Some(persist_ref), Some(master_id)) = (get_u32(body, 0), get_u32(body, 12)) else {
                continue;
            };
            if let Some(&off) = persist.get(&persist_ref)
                && let Some((_, 0x03F8, master)) = record_at(data, off)
            {
                out.push((master_id, master_styles(master)));
            }
        }
    }
    if out.is_empty() {
        let mut offs: Vec<usize> = persist.values().copied().collect();
        offs.sort_unstable();
        for off in offs {
            if let Some((_, 0x03F8, master)) = record_at(data, off) {
                out.push((0, master_styles(master)));
            }
        }
    }
    out
}

impl Extractor {
    fn notice(&mut self, code: &str) {
        // One notice per kind/slide keeps adversarial shapes from producing
        // unbounded diagnostic output, while retaining a useful location.
        let object = format!("segment-{}", self.segments.len() + 1);
        if !self
            .diagnostics
            .iter()
            .rev()
            .take_while(|d| d.location.as_ref().and_then(|l| l.object.as_ref()) == Some(&object))
            .any(|d| d.code == code)
        {
            self.diagnostics.push(Diagnostic {
                code: code.into(),
                severity: "warning".into(),
                location: Some(SourceLocation {
                    part: "PowerPoint Document".into(),
                    sheet: None,
                    object: Some(object),
                }),
                arguments: Vec::new(),
            });
        }
    }

    fn interactive(&mut self, ver_inst: u16, body: &[u8]) {
        // Click actions only. Mouseover actions cannot be represented by a
        // Markdown link and must not overwrite a click target.
        if ver_inst >> 4 != 0 {
            self.notice("presentation.linkUnavailable");
            return;
        }
        let action = children(body)
            .find(|&(_, t, _)| t == 0x0FF3)
            .map(|(.., b)| b);
        let target = action
            .filter(|b| b.get(8) == Some(&4) && b.get(12) == Some(&8))
            .and_then(|b| get_u32(b, 4))
            .and_then(|id| self.hyperlinks.get(&id))
            .cloned()
            .flatten();
        if let Some(shape) = &mut self.pending {
            // An action with no text range is reported when the shape ends.
            shape.interactive = Some(target);
        } else {
            self.notice("presentation.linkUnavailable");
        }
    }

    fn table(&mut self, body: &[u8]) -> Result<bool, ConvertError> {
        let Some(layout) = drawing::table_layout(body) else {
            return Ok(false);
        };
        self.flush_shape();
        flush_list(&mut self.current, &mut self.list_run);
        let parent = std::mem::take(&mut self.current);
        let mut contents = Vec::with_capacity(layout.shapes.len());
        for shape in &layout.shapes {
            self.walk(shape)?;
            self.flush_shape();
            flush_list(&mut self.current, &mut self.list_run);
            contents.push(std::mem::take(&mut self.current));
        }
        self.current = parent;
        let mut grid = GridBuilder::new();
        for row in 0..layout.rows {
            grid.next_row();
            for col in 0..layout.cols {
                let index = layout.slots[row * layout.cols + col];
                let (top, left, height, width) = layout.positions[index];
                if (row, col) == (top, left) {
                    grid.place(Cell::spanning(
                        std::mem::take(&mut contents[index]),
                        width as u32,
                        height as u32,
                    ))?;
                } else {
                    grid.covered();
                }
            }
        }
        let mut table = grid.finish(TableKind::Data);
        table.header_rows = crate::shared::header::resolve_header_rows(&table, 0);
        self.current.push(Block::Table(table));
        Ok(true)
    }

    /// Walk slides in presentation order: the UserEditAtom chain yields the
    /// persist directory, the DocumentContainer's SlideListWithText yields
    /// slide order and outline text, each slide container its own textboxes.
    /// `Ok(false)` means the persist directory was unusable.
    fn parse_slides(&mut self, data: &[u8], current_user: &[u8]) -> Result<bool, ConvertError> {
        let Some(layout) = locate_document(data, current_user) else {
            return Ok(false);
        };
        self.masters = collect_masters(layout.master_list, &layout.persist, data);
        self.hyperlinks = drawing::hyperlinks(layout.document);
        self.walk_slide_list(layout.slide_list, &layout.persist, data, false, 0x03EE)?;
        if let Some(notes_list) = layout.notes_list {
            self.walk_slide_list(notes_list, &layout.persist, data, true, 0x03F0)?;
        }
        Ok(true)
    }

    fn walk_slide_list(
        &mut self,
        list: &[u8],
        persist: &HashMap<u32, usize>,
        data: &[u8],
        is_notes: bool,
        container_type: u16,
    ) -> Result<(), ConvertError> {
        // (persistIdRef, slideId) of the page whose container is pending.
        let mut pending: Option<(u32, u32)> = None;
        for (ver_inst, rec_type, body) in children(list) {
            match rec_type {
                // SlidePersistAtom: the next slide/notes page begins.
                0x03F3 => {
                    let id =
                        self.finish_slide(pending.take(), persist, data, container_type, is_notes)?;
                    self.end_segment(id);
                    self.current_is_notes = is_notes;
                    pending = get_u32(body, 0).map(|p| (p, get_u32(body, 12).unwrap_or(0)));
                    if !is_notes {
                        self.select_master(pending.map(|(p, _)| p), persist, data);
                    }
                }
                _ => self.record(ver_inst, rec_type, body)?,
            }
        }
        let id = self.finish_slide(pending, persist, data, container_type, is_notes)?;
        self.end_segment(id);
        Ok(())
    }

    /// Emit a slide's own textboxes after its outline text. Returns the
    /// segment's pairing id: the slideId for slides, or the owning slide's
    /// id (NotesAtom slideIdRef) for notes pages.
    fn finish_slide(
        &mut self,
        pending: Option<(u32, u32)>,
        persist: &HashMap<u32, usize>,
        data: &[u8],
        container_type: u16,
        is_notes: bool,
    ) -> Result<Option<u32>, ConvertError> {
        let Some((persist_ref, slide_id)) = pending else {
            return Ok(None);
        };
        let mut id = if is_notes {
            None
        } else {
            (slide_id != 0).then_some(slide_id)
        };
        if let Some(off) = persist.get(&persist_ref)
            && let Some((_, t, body)) = record_at(data, *off)
            && t == container_type
        {
            if is_notes {
                // NotesAtom.slideIdRef names the owning slide (0 = none).
                id = children(body)
                    .find(|&(_, t, _)| t == 0x03F1)
                    .and_then(|(.., atom)| get_u32(atom, 0))
                    .filter(|&v| v != 0);
            }
            self.walk(body)?;
        }
        Ok(id)
    }

    fn end_segment(&mut self, id: Option<u32>) {
        self.flush_shape();
        flush_list(&mut self.current, &mut self.list_run);
        if !self.current.is_empty() {
            let blocks = std::mem::take(&mut self.current);
            self.segments.push((blocks, id, self.current_is_notes));
        }
    }

    fn into_blocks(mut self) -> Vec<Block> {
        self.end_segment(None);
        let mut slides: Vec<(Option<u32>, Vec<Block>)> = Vec::new();
        let mut notes: Vec<(Option<u32>, Vec<Block>)> = Vec::new();
        for (blocks, id, is_notes) in self.segments {
            if is_notes {
                notes.push((id, blocks));
            } else {
                slides.push((id, blocks));
            }
        }
        // Notes pages pair to slides by their stored slide id, not by list
        // position: the notes list may be sparse (notes on only some
        // slides), which order-based zipping would misattribute.
        let mut used = vec![false; notes.len()];
        let mut out = Vec::new();
        for (sid, blocks) in slides {
            out.extend(blocks);
            for (i, (nid, nblocks)) in notes.iter_mut().enumerate() {
                if !used[i] && sid.is_some() && *nid == sid {
                    used[i] = true;
                    out.push(Block::BlockQuote(std::mem::take(nblocks)));
                }
            }
        }
        // Notes without a resolvable owner keep document order at the end.
        for (used, (_, nblocks)) in used.into_iter().zip(notes) {
            if !used && !nblocks.is_empty() {
                out.push(Block::BlockQuote(nblocks));
            }
        }
        out
    }

    /// Pick the master the slide references (SlideAtom.masterIdRef at
    /// offset 12); the first listed master is the deterministic fallback.
    fn select_master(&mut self, slide: Option<u32>, persist: &HashMap<u32, usize>, data: &[u8]) {
        self.active_master = slide
            .and_then(|id| persist.get(&id))
            .and_then(|&off| record_at(data, off))
            .filter(|&(_, t, _)| t == 0x03EE)
            .and_then(|(_, _, body)| children(body).find(|&(_, rec_type, _)| rec_type == 0x03EF))
            .and_then(|(.., atom)| get_u32(atom, 12))
            .and_then(|mid| self.masters.iter().position(|&(id, _)| id == mid))
            .unwrap_or(0);
    }

    /// Iterative container walk over an explicit stack with fixed depth and
    /// record-count bounds — nesting or record counts beyond any real
    /// presentation are attack shapes and hard-fail.
    fn walk(&mut self, data: &[u8]) -> Result<(), ConvertError> {
        self.walk_depth += 1;
        if self.walk_depth > limits::MAX_RECORD_DEPTH {
            return Err(ConvertError::ResourceLimit {
                limit: "max_record_depth",
                detail: "nested drawing tables".into(),
            });
        }
        let mut stack: Vec<(&[u8], usize)> = vec![(data, 0)];
        while let Some((buf, pos)) = stack.last_mut() {
            let Some((ver_inst, rec_type, body)) = record_at(buf, *pos) else {
                stack.pop();
                continue;
            };
            *pos += 8 + body.len();
            self.charge_record()?;
            if ver_inst & 0xF != 0xF {
                self.atom(rec_type, body);
                continue;
            }
            if rec_type == 0xF004 {
                self.flush_shape();
                flush_list(&mut self.current, &mut self.list_run);
            }
            match rec_type {
                0xF003 if drawing::is_table(body) => {
                    if self.table(body)? {
                        continue;
                    }
                    self.notice("presentation.tableFlattened");
                    if stack.len() >= limits::MAX_RECORD_DEPTH {
                        return Err(ConvertError::ResourceLimit {
                            limit: "max_record_depth",
                            detail: "table group nesting".into(),
                        });
                    }
                    stack.push((body, 0));
                }
                0x0FF2 => self.interactive(ver_inst, body),
                // CryptSession10Container: the stream is encrypted.
                0x2F14 => self.encrypted = true,
                // Notes containers are walked via the notes list; recovery
                // has no lists, so their text is taken inline (as notes).
                // A NotesAtom slideIdRef in the masters range (high bit
                // set) marks the notes master: template chrome, excluded.
                0x03F0 if self.recovering => {
                    let master = children(body)
                        .find(|&(_, t, _)| t == 0x03F1)
                        .and_then(|(.., atom)| get_u32(atom, 0))
                        .is_some_and(|id| id & 0x8000_0000 != 0);
                    if !master {
                        self.end_segment(None);
                        self.current_is_notes = true;
                        self.walk(body)?;
                        self.end_segment(None);
                        self.current_is_notes = false;
                    }
                }
                // Notes, master, and handout containers are walked via their
                // own lists, not inline.
                0x03F0 | 0x03F8 | 0x0FC9 => {}
                // Only instance 0 of SlideListWithText holds slide text here.
                0x0FF0 if ver_inst >> 4 != 0 => {}
                _ => {
                    if stack.len() >= limits::MAX_RECORD_DEPTH {
                        return Err(ConvertError::ResourceLimit {
                            limit: "max_record_depth",
                            detail: format!("record nesting exceeds {}", limits::MAX_RECORD_DEPTH),
                        });
                    }
                    stack.push((body, 0));
                }
            }
        }
        self.walk_depth -= 1;
        Ok(())
    }

    /// One record outside `walk` (slide-list traversal): containers descend
    /// through the bounded walk, atoms extract directly.
    fn record(&mut self, ver_inst: u16, rec_type: u16, body: &[u8]) -> Result<(), ConvertError> {
        self.charge_record()?;
        if ver_inst & 0xF == 0xF {
            match rec_type {
                0x0FF2 => self.interactive(ver_inst, body),
                0x2F14 => self.encrypted = true,
                0x03F0 | 0x03F8 | 0x0FC9 => {}
                0x0FF0 if ver_inst >> 4 != 0 => {}
                _ => self.walk(body)?,
            }
        } else {
            self.atom(rec_type, body);
        }
        Ok(())
    }

    fn charge_record(&mut self) -> Result<(), ConvertError> {
        self.records += 1;
        if self.records > limits::MAX_RECORDS {
            return Err(ConvertError::ResourceLimit {
                limit: "max_records",
                detail: format!("record stream exceeds {} records", limits::MAX_RECORDS),
            });
        }
        Ok(())
    }

    fn atom(&mut self, rec_type: u16, body: &[u8]) {
        match rec_type {
            // TextHeaderAtom: a new text shape begins.
            0x0F9F => {
                self.flush_shape();
                self.pending = Some(PendingShape {
                    tx_type: body.first().copied().unwrap_or(1),
                    text: String::new(),
                    styles: None,
                    links: Vec::new(),
                    interactive: None,
                });
            }
            // TextCharsAtom: UTF-16LE.
            0x0FA0 => {
                let units = utf16le_units(body);
                let text: String = char::decode_utf16(units)
                    .map(|r| r.unwrap_or('\u{fffd}'))
                    .collect();
                self.push_text(text);
            }
            // TextBytesAtom: low bytes of UTF-16 code units.
            0x0FA8 => {
                let text: String = body.iter().map(|&b| b as char).collect();
                self.push_text(text);
            }
            // StyleTextPropAtom: styling for the pending shape's text.
            0x0FA1 => {
                if let Some(pending) = &mut self.pending {
                    let len = pending.text.chars().map(char::len_utf16).sum();
                    pending.styles = Some(styletext::parse_style_text(body, len));
                }
            }
            0x0FDF => {
                if let Some(pending) = &mut self.pending {
                    if let Some(target) = pending.interactive.take() {
                        pending.links.push((
                            get_u32(body, 0).unwrap_or(u32::MAX) as usize,
                            get_u32(body, 4).unwrap_or(0) as usize,
                            target,
                        ));
                    }
                }
            }
            _ => {}
        }
    }

    fn push_text(&mut self, text: String) {
        match &mut self.pending {
            Some(pending) => pending.text.push_str(&text),
            None => {
                self.pending = Some(PendingShape {
                    tx_type: 1,
                    text,
                    styles: None,
                    links: Vec::new(),
                    interactive: None,
                });
            }
        }
    }

    /// Emit the pending shape: paragraphs split on CR, styled by the
    /// character runs, listed by paragraph depth/bullet with master defaults.
    fn flush_shape(&mut self) {
        let Some(mut shape) = self.pending.take() else {
            return;
        };
        if shape.text.is_empty() {
            return;
        }
        // Ranges use the original UTF-16 text, including CR paragraph
        // separators. Reject boundaries inside surrogate pairs and overlaps.
        let mut boundaries = vec![0];
        let mut offset = 0;
        for c in shape.text.chars() {
            offset += c.len_utf16();
            boundaries.push(offset);
        }
        shape.links.sort_by_key(|&(start, end, _)| (start, end));
        let mut end = 0;
        let mut unavailable = shape.interactive.is_some();
        shape.links.retain(|(start, stop, target)| {
            let valid = *start < *stop
                && *start >= end
                && boundaries.binary_search(start).is_ok()
                && boundaries.binary_search(stop).is_ok()
                && target.is_some();
            if valid {
                end = *stop;
            } else {
                unavailable = true;
            }
            valid
        });
        if unavailable {
            self.notice("presentation.linkUnavailable");
        }
        self.shape_counter += 1;
        let shape_id = self.shape_counter;
        let is_title = matches!(shape.tx_type, 0 | 6);
        let styles = shape.styles.unwrap_or_default();
        // The active master's per-level defaults for this text type; local
        // exceptions are tri-state and resolve over these.
        let master_levels: Vec<MasterLevel> = self
            .masters
            .get(self.active_master)
            .and_then(|(_, m)| m.get(&(shape.tx_type as u16)))
            .cloned()
            .unwrap_or_default();
        let level_default = |depth: u16| {
            master_levels
                .get(depth as usize)
                .copied()
                .unwrap_or_default()
        };

        // Cursors over the style runs, counted in UTF-16 units.
        let mut char_runs = styles.chars.iter();
        let mut char_run: Option<&CharProps> = char_runs.next();
        let mut char_left = char_run.map(|r| r.count).unwrap_or(usize::MAX);
        let mut para_runs = styles.paragraphs.iter();
        let mut para_run = para_runs.next();
        let mut para_left = para_run.map(|r| r.count).unwrap_or(usize::MAX);

        let mut paragraphs: Vec<(Vec<Inline>, u16, Option<bool>)> = Vec::new();
        let mut inlines: Vec<Inline> = Vec::new();
        let mut run_text = String::new();
        let mut run_style = Style::PLAIN;
        let mut run_link = None;
        let mut text_offset = 0;
        let mut link_cursor = 0;
        let para_props = |r: Option<&styletext::ParaProps>| match r {
            Some(p) => (p.depth, p.bullet),
            None => (0, None),
        };

        for c in shape.text.chars() {
            while shape
                .links
                .get(link_cursor)
                .is_some_and(|(_, end, _)| *end <= text_offset)
            {
                link_cursor += 1;
            }
            let link = shape
                .links
                .get(link_cursor)
                .filter(|(start, _, _)| *start <= text_offset)
                .map(|_| link_cursor);
            let d = level_default(para_props(para_run).0);
            let style = Style {
                script: crate::model::Script::Baseline,
                bold: char_run.and_then(|r| r.bold).or(d.bold).unwrap_or(false),
                italic: char_run
                    .and_then(|r| r.italic)
                    .or(d.italic)
                    .unwrap_or(false),
                strike: false,
                code: false,
            };
            if c == '\r' {
                if !run_text.is_empty() {
                    let text = clean_text(&std::mem::take(&mut run_text));
                    if !text.is_empty() {
                        push_run(
                            &mut inlines,
                            text,
                            run_style,
                            run_link.and_then(|i: usize| shape.links[i].2.clone()),
                        );
                    }
                }
                let (depth, bullet) = para_props(para_run);
                paragraphs.push((std::mem::take(&mut inlines), depth, bullet));
            } else if c == '\u{b}' {
                if !run_text.is_empty() {
                    let text = clean_text(&std::mem::take(&mut run_text));
                    if !text.is_empty() {
                        push_run(
                            &mut inlines,
                            text,
                            run_style,
                            run_link.and_then(|i: usize| shape.links[i].2.clone()),
                        );
                    }
                }
                inlines.push(Inline::LineBreak);
            } else {
                if (style != run_style || link != run_link) && !run_text.is_empty() {
                    let text = clean_text(&std::mem::take(&mut run_text));
                    if !text.is_empty() {
                        push_run(
                            &mut inlines,
                            text,
                            run_style,
                            run_link.and_then(|i: usize| shape.links[i].2.clone()),
                        );
                    }
                }
                run_style = style;
                run_link = link;
                run_text.push(c);
            }
            // Advance run cursors by the character's UTF-16 width.
            let width = c.len_utf16();
            text_offset += width;
            char_left = char_left.saturating_sub(width);
            if char_left == 0 {
                char_run = char_runs.next();
                char_left = char_run.map(|r| r.count).unwrap_or(usize::MAX);
            }
            para_left = para_left.saturating_sub(width);
            if para_left == 0 {
                para_run = para_runs.next();
                para_left = para_run.map(|r| r.count).unwrap_or(usize::MAX);
            }
        }
        if !run_text.is_empty() {
            let text = clean_text(&run_text);
            if !text.is_empty() {
                push_run(
                    &mut inlines,
                    text,
                    run_style,
                    run_link.and_then(|i: usize| shape.links[i].2.clone()),
                );
            }
        }
        if !inlines.is_empty() {
            let (depth, bullet) = para_props(para_run);
            paragraphs.push((inlines, depth, bullet));
        }

        for (mut inlines, depth, bullet) in paragraphs {
            if inlines_are_empty(&inlines) {
                flush_list(&mut self.current, &mut self.list_run);
                continue;
            }
            if is_title {
                flush_list(&mut self.current, &mut self.list_run);
                let d = level_default(depth);
                let base = StyleDelta {
                    bold: d.bold,
                    italic: d.italic,
                    ..Default::default()
                };
                rebase_emphasis(&mut inlines, base.resolve());
                let anchor = Some(crate::model::inlines_to_plain_text(&inlines));
                self.current.push(Block::Heading {
                    level: 2,
                    anchor,
                    content: inlines,
                });
                continue;
            }
            let bullet = bullet.or(level_default(depth).bullet).unwrap_or(false);
            if bullet {
                self.list_run.push(ListEntry {
                    level: depth as usize,
                    key: ListKey {
                        instance: shape_id,
                        marker: MarkerKind::Bullet,
                    },
                    number: 0,
                    label: None,
                    blocks: vec![Block::Paragraph(inlines)],
                });
            } else {
                flush_list(&mut self.current, &mut self.list_run);
                self.current.push(Block::Paragraph(inlines));
            }
        }
    }
}
