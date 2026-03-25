use crate::ast::*;
use crate::diag::{DiagEngine, Diagnostic};
use crate::lexer::{LexLine, SpannedToken, Token};
use crate::span::Span;

pub fn parse(lines: &[LexLine], diags: &mut DiagEngine) -> Document {
    let mut decls = Vec::new();
    let mut i = 0;

    while i < lines.len() {
        let line = &lines[i];

        // Skip blanks and comments
        if line.is_blank || line.is_comment {
            i += 1;
            continue;
        }

        // Top-level declarations must be at indent 0
        if line.indent != 0 {
            let span = line.tokens.first().map(|t| t.span).unwrap_or_default();
            diags.emit(
                Diagnostic::error("unexpected indentation at top level")
                    .with_span(span)
                    .with_help("top-level declarations must start at column 1"),
            );
            i += 1;
            continue;
        }

        if line.tokens.is_empty() {
            i += 1;
            continue;
        }

        // Parse a declaration
        match parse_decl(lines, &mut i, diags) {
            Some(decl) => decls.push(decl),
            None => {
                i += 1; // skip to avoid infinite loop
            }
        }
    }

    Document { decls }
}

fn parse_decl(lines: &[LexLine], i: &mut usize, diags: &mut DiagEngine) -> Option<Decl> {
    let line = &lines[*i];
    let tokens = &line.tokens;

    if tokens.is_empty() {
        return None;
    }

    // First token: kind keyword
    let (kind, kind_span) = match &tokens[0].token {
        Token::Ident(s) => {
            match DeclKind::from_str(s) {
                Some(k) => (k, tokens[0].span),
                None => {
                    diags.emit(
                        Diagnostic::error(format!("unknown declaration kind `{}`", s))
                            .with_span(tokens[0].span)
                            .with_help("expected one of: equipment, valve, line, instrument, signal, group, area, note, junction"),
                    );
                    *i += 1;
                    return None;
                }
            }
        }
        _ => {
            diags.emit(
                Diagnostic::error("expected declaration kind keyword")
                    .with_span(tokens[0].span),
            );
            *i += 1;
            return None;
        }
    };

    // Second token: identifier
    if tokens.len() < 2 {
        diags.emit(
            Diagnostic::error(format!("expected identifier after `{}`", kind))
                .with_span(kind_span),
        );
        *i += 1;
        return None;
    }

    let (id, id_span) = match &tokens[1].token {
        Token::Ident(s) => (s.clone(), tokens[1].span),
        _ => {
            diags.emit(
                Diagnostic::error("expected identifier (declaration ID)")
                    .with_span(tokens[1].span),
            );
            *i += 1;
            return None;
        }
    };

    let decl_span = kind_span.merge(id_span);

    // Remaining tokens determine inline vs block
    // Check: is the last meaningful token a colon after the ID (block form)?
    // Or are there `key=value` pairs (inline form)?

    let rest = &tokens[2..];

    // Determine form
    if rest.is_empty() {
        // Could be a block header without colon, or just an empty decl
        // Peek at next line to see if indented
        *i += 1;
        let block_props = collect_block_props(lines, i, 1, diags);
        if block_props.is_empty() {
            // Treat as empty block
        }
        return Some(Decl {
            kind,
            id,
            form: DeclForm::Block(block_props),
            span: decl_span,
        });
    }

    // Check if it ends with a colon => block form
    let last_tok = rest.last().unwrap();
    if last_tok.token == Token::Colon {
        // Block form: rest should be empty before the colon (no inline props mixed)
        if rest.len() > 1 {
            // There are tokens before the colon - mixed form error
            diags.emit(
                Diagnostic::error(format!(
                    "declaration `{} {}` mixes inline and block syntax",
                    kind, id
                ))
                .with_span(decl_span)
                .with_help("use either inline (key=value) or block (colon + indented lines), not both"),
            );
        }
        *i += 1;
        let block_props = collect_block_props(lines, i, 1, diags);
        return Some(Decl {
            kind,
            id,
            form: DeclForm::Block(block_props),
            span: decl_span,
        });
    }

    // Inline form: parse key=value pairs
    let inline_props = parse_inline_props(rest, diags);
    *i += 1;
    Some(Decl {
        kind,
        id,
        form: DeclForm::Inline(inline_props),
        span: decl_span,
    })
}

/// Parse inline properties: key=value key=value ...
fn parse_inline_props(tokens: &[SpannedToken], diags: &mut DiagEngine) -> Vec<Prop> {
    let mut props = Vec::new();
    let mut pos = 0;

    while pos < tokens.len() {
        // Expect: ident = value
        let key_tok = &tokens[pos];
        let key = match &key_tok.token {
            Token::Ident(s) => s.clone(),
            _ => {
                diags.emit(
                    Diagnostic::error(format!("expected property key, found `{}`", key_tok.token))
                        .with_span(key_tok.span),
                );
                pos += 1;
                continue;
            }
        };
        let key_span = key_tok.span;
        pos += 1;

        // Expect =
        if pos >= tokens.len() || tokens[pos].token != Token::Equals {
            let sp = if pos < tokens.len() { tokens[pos].span } else { key_span };
            diags.emit(
                Diagnostic::error(format!("expected `=` after property key `{}`", key))
                    .with_span(sp),
            );
            continue;
        }
        pos += 1;

        // Parse value
        let (value, consumed, val_span) = parse_value(&tokens[pos..], diags);
        pos += consumed;

        let span = key_span.merge(val_span);
        props.push(Prop {
            key,
            value: PropVal::Value(value),
            span,
        });
    }

    props
}

/// Collect block properties at the given indent level.
fn collect_block_props(
    lines: &[LexLine],
    i: &mut usize,
    expected_indent: usize,
    diags: &mut DiagEngine,
) -> Vec<Prop> {
    let mut props = Vec::new();

    while *i < lines.len() {
        let line = &lines[*i];

        if line.is_blank || line.is_comment {
            *i += 1;
            continue;
        }

        // If indent is less than expected, we're done with this block
        if line.indent < expected_indent * 2 {
            break;
        }

        // If indent is exactly what we expect
        if line.indent == expected_indent * 2 {
            match parse_block_prop(lines, i, expected_indent, diags) {
                Some(prop) => props.push(prop),
                None => {
                    *i += 1;
                }
            }
        } else {
            // Deeper indentation than expected
            let span = line.tokens.first().map(|t| t.span).unwrap_or_default();
            diags.emit(
                Diagnostic::error(format!(
                    "unexpected indentation level {} (expected {})",
                    line.indent,
                    expected_indent * 2
                ))
                .with_span(span),
            );
            *i += 1;
        }
    }

    props
}

fn parse_block_prop(
    lines: &[LexLine],
    i: &mut usize,
    expected_indent: usize,
    diags: &mut DiagEngine,
) -> Option<Prop> {
    let line = &lines[*i];
    let tokens = &line.tokens;

    if tokens.is_empty() {
        return None;
    }

    // Expect: key ":"  [value]
    let key_tok = &tokens[0];
    let key = match &key_tok.token {
        Token::Ident(s) => s.clone(),
        _ => {
            diags.emit(
                Diagnostic::error(format!("expected property key, found `{}`", key_tok.token))
                    .with_span(key_tok.span),
            );
            return None;
        }
    };
    let key_span = key_tok.span;

    if tokens.len() < 2 || tokens[1].token != Token::Colon {
        let sp = if tokens.len() > 1 { tokens[1].span } else { key_span };
        diags.emit(
            Diagnostic::error(format!("expected `:` after property key `{}`", key))
                .with_span(sp),
        );
        *i += 1;
        return None;
    }

    let rest = &tokens[2..];

    // If rest is empty, this might be a nested block
    if rest.is_empty() {
        *i += 1;
        // Check if next lines are at deeper indent -> nested block
        let sub_props = collect_block_props(lines, i, expected_indent + 1, diags);
        let span = key_span;
        return Some(Prop {
            key,
            value: PropVal::Block(sub_props),
            span,
        });
    }

    // Parse value from rest tokens
    let (value, _consumed, val_span) = parse_value(rest, diags);
    *i += 1;

    let span = key_span.merge(val_span);
    Some(Prop {
        key,
        value: PropVal::Value(value),
        span,
    })
}

/// Parse a value from token slice. Returns (value, tokens_consumed, span).
fn parse_value(tokens: &[SpannedToken], _diags: &mut DiagEngine) -> (Value, usize, Span) {
    if tokens.is_empty() {
        return (Value::Str(String::new()), 0, Span::default());
    }

    // Tuple: ( ... )
    if tokens[0].token == Token::LParen {
        return parse_tuple(tokens);
    }

    // Quoted string
    if let Token::Quoted(s) = &tokens[0].token {
        let span = tokens[0].span;
        return (Value::Str(s.clone()), 1, span);
    }

    // Ident (possibly a ref or list)
    if let Token::Ident(s) = &tokens[0].token {
        let span0 = tokens[0].span;

        // Check for dot notation: ident.ident -> Ref
        if tokens.len() >= 3
            && tokens[1].token == Token::Dot
        {
            if let Token::Ident(port) = &tokens[2].token {
                let port = port.clone();
                let span = span0.merge(tokens[2].span);
                // After the ref, check if there's a comma -> this is part of a list
                // For now, return the ref (lists are handled at property level)
                // Check for comma after ref to detect list
                if tokens.len() > 3 && tokens[3].token == Token::Comma {
                    // It's a list starting with a ref - treat all as strings
                    return parse_list_of_strings(tokens);
                }
                return (Value::Ref(s.clone(), Some(port)), 3, span);
            }
        }

        // Check for comma -> list
        if tokens.len() > 1 && tokens[1].token == Token::Comma {
            return parse_list_of_strings(tokens);
        }

        // Single ident: could be a bare ref (no port) or simple string
        // We'll treat it as Ref with no port - normalize will sort out
        let span = span0;
        // Check if the ident looks like it could be a plain string value
        // (e.g., "process", "pump", "closed" etc.)
        // We use Ref for identifiers that could be object IDs, and Str for everything else.
        // Actually, we'll always use Ref and let normalize decide.
        return (Value::Ref(s.clone(), None), 1, span);
    }

    // Fallback
    let span = tokens[0].span;
    (Value::Str(tokens[0].token.to_string()), 1, span)
}

/// Parse a tuple like (10, 8) or (0, -2)
fn parse_tuple(tokens: &[SpannedToken]) -> (Value, usize, Span) {
    // tokens[0] is LParen
    let start_span = tokens[0].span;
    let mut pos = 1;
    let mut nums = Vec::new();
    let mut end_span = start_span;

    while pos < tokens.len() {
        let tok = &tokens[pos];
        match &tok.token {
            Token::RParen => {
                end_span = tok.span;
                pos += 1;
                break;
            }
            Token::Comma => {
                pos += 1;
            }
            Token::Ident(s) => {
                // Try to parse as integer (possibly negative via separate minus sign)
                let n: i32 = s.parse().unwrap_or(0);
                nums.push(n);
                end_span = tok.span;
                pos += 1;
            }
            _ => {
                // skip unexpected
                pos += 1;
            }
        }
    }

    let span = start_span.merge(end_span);
    (Value::Tuple(nums), pos, span)
}

/// Parse a comma-separated list of strings/idents.
fn parse_list_of_strings(tokens: &[SpannedToken]) -> (Value, usize, Span) {
    let mut items = Vec::new();
    let mut pos = 0;
    let start_span = tokens[0].span;
    let mut end_span = start_span;

    loop {
        if pos >= tokens.len() {
            break;
        }
        match &tokens[pos].token {
            Token::Ident(s) => {
                items.push(s.clone());
                end_span = tokens[pos].span;
                pos += 1;
            }
            Token::Quoted(s) => {
                items.push(s.clone());
                end_span = tokens[pos].span;
                pos += 1;
            }
            Token::Comma => {
                pos += 1;
            }
            Token::Dot => {
                // Part of a dotted ref in a list - append to last item
                pos += 1;
                if let Some(last) = items.last_mut() {
                    if pos < tokens.len() {
                        if let Token::Ident(s) = &tokens[pos].token {
                            last.push('.');
                            last.push_str(s);
                            end_span = tokens[pos].span;
                            pos += 1;
                        }
                    }
                }
            }
            _ => break,
        }
    }

    let span = start_span.merge(end_span);
    if items.len() == 1 {
        // Single item is not really a list
        (Value::Str(items.remove(0)), pos, span)
    } else {
        (Value::List(items), pos, span)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diag::DiagEngine;
    use crate::lexer::lex;

    fn parse_src(src: &str) -> (Document, DiagEngine) {
        let mut diags = DiagEngine::new();
        let lines = lex(src, &mut diags);
        let doc = parse(&lines, &mut diags);
        (doc, diags)
    }

    #[test]
    fn test_inline_decl() {
        let (doc, diags) = parse_src("line L100 class=process from=P101.out to=CV101.in");
        assert!(!diags.has_errors(), "{:?}", diags.diagnostics);
        assert_eq!(doc.decls.len(), 1);
        let decl = &doc.decls[0];
        assert_eq!(decl.id, "L100");
        assert!(matches!(decl.form, DeclForm::Inline(_)));
        let props = decl.form.props();
        assert_eq!(props.len(), 3);
        assert_eq!(props[0].key, "class");
        assert_eq!(props[1].key, "from");
        assert_eq!(props[2].key, "to");
    }

    #[test]
    fn test_block_decl() {
        let src = "line L100:\n  class: process\n  from: P101.out\n  to: CV101.in\n";
        let (doc, diags) = parse_src(src);
        assert!(!diags.has_errors(), "{:?}", diags.diagnostics);
        assert_eq!(doc.decls.len(), 1);
        let props = doc.decls[0].form.props();
        assert_eq!(props.len(), 3);
    }

    #[test]
    fn test_nested_ports_block() {
        let src = "equipment E101:\n  type: heat_exchanger\n  ports:\n    in: west\n    out: east\n";
        let (doc, diags) = parse_src(src);
        assert!(!diags.has_errors(), "{:?}", diags.diagnostics);
        let props = doc.decls[0].form.props();
        assert_eq!(props.len(), 2);
        // ports should be a Block
        assert!(matches!(props[1].value, PropVal::Block(_)));
    }

    #[test]
    fn test_comment_and_blank() {
        let src = "# comment\n\nequipment P101:\n  type: pump\n";
        let (doc, diags) = parse_src(src);
        assert!(!diags.has_errors());
        assert_eq!(doc.decls.len(), 1);
    }

    #[test]
    fn test_tuple_value() {
        let src = "equipment P101:\n  at: (10,8)\n  type: pump\n";
        let (doc, diags) = parse_src(src);
        assert!(!diags.has_errors(), "{:?}", diags.diagnostics);
        let props = doc.decls[0].form.props();
        assert!(matches!(&props[0].value, PropVal::Value(Value::Tuple(v)) if *v == vec![10, 8]));
    }

    #[test]
    fn test_list_value() {
        let src = "equipment P101:\n  type: pump\n  ports: in, out\n";
        let (doc, diags) = parse_src(src);
        assert!(!diags.has_errors(), "{:?}", diags.diagnostics);
        let props = doc.decls[0].form.props();
        assert!(matches!(&props[1].value, PropVal::Value(Value::List(v)) if v.len() == 2));
    }

    #[test]
    fn test_tab_error() {
        let src = "\tequipment P101:\n";
        let (_doc, diags) = parse_src(src);
        assert!(diags.has_errors());
    }
}
