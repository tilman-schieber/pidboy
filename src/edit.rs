//! Span-based source edits: reposition a node by rewriting its `at:` prop.
//!
//! The `.pid` source stays the single source of truth for the interactive
//! editor — a drag becomes a textual edit here, then a fresh compile.
//! Everything outside the edited range is preserved byte-for-byte.

use crate::ast::{DeclForm, DeclKind, PropVal};
use crate::diag::DiagEngine;
use crate::lexer;
use crate::normalize;
use crate::parser;
use crate::span::Span;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditError {
    UnknownId(String),
    NotPositionable { id: String, kind: String },
    FramedGroupMember { id: String, group: String },
}

impl std::fmt::Display for EditError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EditError::UnknownId(id) => write!(f, "no declaration with id `{}`", id),
            EditError::NotPositionable { id, kind } => {
                write!(f, "`{}` is a {} and has no position of its own", id, kind)
            }
            EditError::FramedGroupMember { id, group } => write!(
                f,
                "`{}` belongs to framed module `{}`; its position is set by the module",
                id, group
            ),
        }
    }
}

impl std::error::Error for EditError {}

/// Kinds whose `at:` prop is honoured by layout (see `get_explicit_pos`).
fn is_positionable(kind: DeclKind) -> bool {
    matches!(
        kind,
        DeclKind::Equipment
            | DeclKind::Valve
            | DeclKind::Instrument
            | DeclKind::Junction
            | DeclKind::Note
    )
}

/// Byte offset of the start of every line, so line-relative spans can be
/// mapped into the source string.
fn line_starts(source: &str) -> Vec<usize> {
    let mut starts = vec![0];
    for (i, b) in source.bytes().enumerate() {
        if b == b'\n' {
            starts.push(i + 1);
        }
    }
    starts
}

/// Rewrite `source` so the declaration `id` carries `at: (x,y)`, replacing an
/// existing `at:` prop or inserting one. Returns the full modified source.
pub fn set_node_position(source: &str, id: &str, x: i32, y: i32) -> Result<String, EditError> {
    // Throwaway diagnostics: unrelated errors elsewhere in the file must not
    // block repositioning a node that parses fine.
    let mut diags = DiagEngine::new();
    let lex_lines = lexer::lex(source, &mut diags);
    let doc = parser::parse(&lex_lines, &mut diags);

    let decl = doc
        .decls
        .iter()
        .find(|d| d.id == id)
        .ok_or_else(|| EditError::UnknownId(id.to_string()))?;

    let diagram = normalize::normalize(&doc, &mut diags);

    // Framed groups are positionable as whole modules; anything else must
    // be a kind whose `at:` layout honours.
    if decl.kind == DeclKind::Group {
        let framed = diagram.groups.get(id).map(|g| g.frame).unwrap_or(false);
        if !framed {
            return Err(EditError::NotPositionable {
                id: id.to_string(),
                kind: "group without frame".to_string(),
            });
        }
    } else if !is_positionable(decl.kind) {
        return Err(EditError::NotPositionable {
            id: id.to_string(),
            kind: decl.kind.to_string(),
        });
    }

    // Layout ignores `at:` on framed-group members (the module places them),
    // so refuse the edit instead of writing a dead prop.
    for group in diagram.groups.values() {
        if group.frame && group.members.iter().any(|m| m == id) {
            return Err(EditError::FramedGroupMember {
                id: id.to_string(),
                group: group.id.clone(),
            });
        }
    }

    let starts = line_starts(source);
    // Spans store byte offsets relative to their own line.
    let abs = |sp: &Span| -> (usize, usize) {
        let base = starts
            .get((sp.line as usize).saturating_sub(1))
            .copied()
            .unwrap_or(0);
        (base + sp.start, base + sp.end)
    };

    let existing = decl
        .form
        .props()
        .iter()
        .find(|p| p.key == "at" && matches!(p.value, PropVal::Value(_)));

    let mut out = String::with_capacity(source.len() + 16);
    match (existing, &decl.form) {
        // Replace the whole `at ... )` range; a trailing same-line comment
        // survives because the prop span ends at the closing paren.
        (Some(prop), DeclForm::Block(_)) => {
            let (s, e) = abs(&prop.span);
            out.push_str(&source[..s]);
            out.push_str(&format!("at: ({},{})", x, y));
            out.push_str(&source[e..]);
        }
        (Some(prop), DeclForm::Inline(_)) => {
            let (s, e) = abs(&prop.span);
            out.push_str(&source[..s]);
            out.push_str(&format!("at=({},{})", x, y));
            out.push_str(&source[e..]);
        }
        // Insert a new prop line directly under the declaration header.
        (None, DeclForm::Block(_)) => {
            let header_line = decl.span.line as usize; // 1-based
            match starts.get(header_line).copied() {
                Some(pos) => {
                    out.push_str(&source[..pos]);
                    out.push_str(&format!("  at: ({},{})\n", x, y));
                    out.push_str(&source[pos..]);
                }
                None => {
                    // Header is the last line of the file.
                    out.push_str(source);
                    if !source.ends_with('\n') {
                        out.push('\n');
                    }
                    out.push_str(&format!("  at: ({},{})\n", x, y));
                }
            }
        }
        // Append after the last inline prop (or the id when there are none),
        // staying ahead of any trailing comment.
        (None, DeclForm::Inline(props)) => {
            let anchor = props
                .last()
                .map(|p| abs(&p.span).1)
                .unwrap_or_else(|| abs(&decl.span).1);
            out.push_str(&source[..anchor]);
            out.push_str(&format!(" at=({},{})", x, y));
            out.push_str(&source[anchor..]);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::GridPos;

    fn grid_pos_of(source: &str, id: &str) -> Option<GridPos> {
        let (_doc, diagram, diags) = crate::compile::analyze(source, false);
        assert!(!diags.has_errors(), "sample must compile: {:?}", diags.diagnostics);
        diagram.equipment.get(id).and_then(|e| e.pos.clone()).or_else(|| {
            diagram.valves.get(id).and_then(|v| v.pos.clone()).or_else(|| {
                diagram
                    .instruments
                    .get(id)
                    .and_then(|i| i.pos.clone())
                    .or_else(|| diagram.junctions.get(id).and_then(|j| j.pos.clone()))
            })
        })
    }

    /// Assert the edit changed nothing outside `[s..e)` of the original.
    fn assert_only_range_changed(before: &str, after: &str, changed_line: u32) {
        let b: Vec<&str> = before.lines().collect();
        let a: Vec<&str> = after.lines().collect();
        for (i, (bl, al)) in b.iter().zip(a.iter()).enumerate() {
            if (i + 1) as u32 != changed_line {
                assert_eq!(bl, al, "line {} must be untouched", i + 1);
            }
        }
    }

    const BLOCK: &str = "\
equipment T1:
  type: tank
  ports:
    out: east
  at: (2,3)

equipment P1: # trailing header comment
  type: pump_centrifugal
  ports:
    in: west

line L1:
  class: process
  from: T1.out
  to: P1.in
";

    #[test]
    fn replace_block_form() {
        let out = set_node_position(BLOCK, "T1", 7, -4).unwrap();
        assert!(out.contains("at: (7,-4)"));
        assert!(!out.contains("at: (2,3)"));
        assert_only_range_changed(BLOCK, &out, 5);
        assert_eq!(grid_pos_of(&out, "T1").map(|p| (p.x, p.y)), Some((7, -4)));
    }

    #[test]
    fn insert_block_form_after_header() {
        let out = set_node_position(BLOCK, "P1", 5, 3).unwrap();
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines[6], "equipment P1: # trailing header comment");
        assert_eq!(lines[7], "  at: (5,3)");
        assert_eq!(lines[8], "  type: pump_centrifugal");
        assert_eq!(grid_pos_of(&out, "P1").map(|p| (p.x, p.y)), Some((5, 3)));
    }

    #[test]
    fn replace_preserves_trailing_comment() {
        let src = "equipment T1:\n  type: tank\n  at: (2,3) # keep me\n";
        let out = set_node_position(src, "T1", 9, 9).unwrap();
        assert!(out.contains("at: (9,9) # keep me"));
    }

    #[test]
    fn replace_inline_form() {
        let src = "junction J1 at=(4,4)\nequipment T1:\n  type: tank\n";
        let out = set_node_position(src, "J1", 6, 2).unwrap();
        assert!(out.starts_with("junction J1 at=(6,2)\n"));
        assert_eq!(grid_pos_of(&out, "J1").map(|p| (p.x, p.y)), Some((6, 2)));
    }

    #[test]
    fn insert_inline_form_after_last_prop() {
        let src = "equipment T9 type=tank # comment\n";
        let out = set_node_position(src, "T9", 3, 4).unwrap();
        assert!(out.starts_with("equipment T9 type=tank at=(3,4) # comment"), "got: {out}");
    }

    #[test]
    fn bare_header_decl_gets_block_prop() {
        // A header without colon or props parses as an empty block decl,
        // so the new prop lands indented on the next line.
        let src = "junction J1\njunction J2\n";
        let out = set_node_position(src, "J1", 1, 2).unwrap();
        assert!(out.starts_with("junction J1\n  at: (1,2)\njunction J2\n"), "got: {out}");
    }

    #[test]
    fn insert_when_header_is_last_line_without_newline() {
        let src = "equipment T1:\n  type: tank\n\nequipment X9:";
        let out = set_node_position(src, "X9", 1, 1).unwrap();
        assert!(out.ends_with("equipment X9:\n  at: (1,1)\n"), "got: {out}");
    }

    #[test]
    fn malformed_at_is_repaired() {
        let src = "equipment T1:\n  type: tank\n  at: junk\n";
        let out = set_node_position(src, "T1", 2, 2).unwrap();
        assert!(out.contains("at: (2,2)"));
        assert!(!out.contains("junk"));
    }

    #[test]
    fn duplicate_at_edits_the_first() {
        let src = "equipment T1:\n  type: tank\n  at: (1,1)\n  at: (5,5)\n";
        let out = set_node_position(src, "T1", 8, 8).unwrap();
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines[2], "  at: (8,8)");
        assert_eq!(lines[3], "  at: (5,5)");
    }

    #[test]
    fn unknown_id_rejected() {
        assert_eq!(
            set_node_position(BLOCK, "NOPE", 1, 1),
            Err(EditError::UnknownId("NOPE".into()))
        );
    }

    #[test]
    fn line_is_not_positionable() {
        assert!(matches!(
            set_node_position(BLOCK, "L1", 1, 1),
            Err(EditError::NotPositionable { .. })
        ));
    }

    #[test]
    fn framed_group_member_rejected() {
        let src = "\
equipment T1:
  type: tank

equipment T2:
  type: tank

group M1:
  members: T1
  frame: true
  label: \"Module\"
";
        assert_eq!(
            set_node_position(src, "T1", 1, 1),
            Err(EditError::FramedGroupMember { id: "T1".into(), group: "M1".into() })
        );
        // Non-members stay editable.
        assert!(set_node_position(src, "T2", 1, 1).is_ok());
    }

    #[test]
    fn framed_group_itself_is_movable() {
        let src = "\
equipment T1:
  type: tank

group M1:
  members: T1
  frame: true
";
        let out = set_node_position(src, "M1", 7, 3).unwrap();
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines[3], "group M1:");
        assert_eq!(lines[4], "  at: (7,3)");

        // Unframed groups are not positionable.
        let plain = "equipment T1:\n  type: tank\n\ngroup G1:\n  members: T1\n";
        assert!(matches!(
            set_node_position(plain, "G1", 1, 1),
            Err(EditError::NotPositionable { .. })
        ));
    }

    #[test]
    fn edit_survives_unrelated_errors() {
        let src = "equipment T1:\n  type: tank\n  at: (2,3)\n\nline L9:\n  from: A.b\n  to: C.d\n";
        let out = set_node_position(src, "T1", 4, 4).unwrap();
        assert!(out.contains("at: (4,4)"));
    }
}
