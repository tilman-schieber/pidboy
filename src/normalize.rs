use crate::ast::*;
use crate::diag::{DiagEngine, Diagnostic};
use crate::model::*;
use crate::span::Span;

pub fn normalize(doc: &Document, diags: &mut DiagEngine) -> Diagram {
    let mut diagram = Diagram::new();

    for decl in &doc.decls {
        // Check for duplicate IDs
        if diagram.id_exists(&decl.id) {
            diags.emit(
                Diagnostic::error(format!("duplicate declaration ID `{}`", decl.id))
                    .with_span(decl.span)
                    .with_note("each ID must be unique across all declarations"),
            );
            continue;
        }

        match decl.kind {
            DeclKind::Equipment => {
                if let Some(e) = normalize_equipment(decl, diags) {
                    diagram.equipment.insert(e.id.clone(), e);
                    diagram.order.push((DeclKind::Equipment, decl.id.clone()));
                }
            }
            DeclKind::Valve => {
                if let Some(v) = normalize_valve(decl, diags) {
                    diagram.valves.insert(v.id.clone(), v);
                    diagram.order.push((DeclKind::Valve, decl.id.clone()));
                }
            }
            DeclKind::Line => {
                if let Some(l) = normalize_line(decl, diags) {
                    diagram.lines.insert(l.id.clone(), l);
                    diagram.order.push((DeclKind::Line, decl.id.clone()));
                }
            }
            DeclKind::Instrument => {
                if let Some(instr) = normalize_instrument(decl, diags) {
                    diagram.instruments.insert(instr.id.clone(), instr);
                    diagram.order.push((DeclKind::Instrument, decl.id.clone()));
                }
            }
            DeclKind::Signal => {
                if let Some(s) = normalize_signal(decl, diags) {
                    diagram.signals.insert(s.id.clone(), s);
                    diagram.order.push((DeclKind::Signal, decl.id.clone()));
                }
            }
            DeclKind::Junction => {
                let j = normalize_junction(decl, diags);
                diagram.junctions.insert(j.id.clone(), j);
                diagram.order.push((DeclKind::Junction, decl.id.clone()));
            }
            DeclKind::Note => {
                let n = normalize_note(decl, diags);
                diagram.notes.insert(n.id.clone(), n);
                diagram.order.push((DeclKind::Note, decl.id.clone()));
            }
            DeclKind::Group => {
                if let Some(g) = normalize_group(decl, diags) {
                    diagram.groups.insert(g.id.clone(), g);
                    diagram.order.push((DeclKind::Group, decl.id.clone()));
                }
            }
            DeclKind::Area => {
                let a = normalize_area(decl, diags);
                diagram.areas.insert(a.id.clone(), a);
                diagram.order.push((DeclKind::Area, decl.id.clone()));
            }
        }
    }

    diagram
}

// ---- Property helpers ----

/// Known properties for each declaration kind - used for unknown-property warnings.
fn known_props_for_kind(kind: DeclKind) -> &'static [&'static str] {
    match kind {
        DeclKind::Equipment => &["type", "at", "size", "attach", "ports", "label", "orient", "zone"],
        DeclKind::Valve => &["type", "at", "actuator", "fail", "state", "setpoint", "ports", "label"],
        DeclKind::Line => &["class", "from", "to", "label", "size", "spec", "route", "dir", "flexible", "insulated"],
        DeclKind::Instrument => &["type", "at", "attach", "location", "label", "loop"],
        DeclKind::Signal => &["type", "from", "to", "label"],
        DeclKind::Group => &["members", "label"],
        DeclKind::Area => &["label", "bounds"],
        DeclKind::Note => &["at", "text"],
        DeclKind::Junction => &["at"],
    }
}

fn warn_unknown_props(decl: &Decl, diags: &mut DiagEngine) {
    let known = known_props_for_kind(decl.kind);
    for prop in decl.form.props() {
        if !known.contains(&prop.key.as_str()) {
            diags.emit(
                Diagnostic::warning(format!(
                    "unknown property `{}` on `{}` declaration",
                    prop.key, decl.kind
                ))
                .with_span(prop.span),
            );
        }
    }
}

fn find_prop<'a>(props: &'a [Prop], key: &str) -> Option<&'a Prop> {
    props.iter().find(|p| p.key == key)
}

fn prop_as_str<'a>(prop: &'a Prop) -> Option<&'a str> {
    match &prop.value {
        PropVal::Value(Value::Str(s)) => Some(s),
        PropVal::Value(Value::Ref(s, None)) => Some(s),
        _ => None,
    }
}

fn prop_as_ref(prop: &Prop) -> Option<ObjRef> {
    match &prop.value {
        PropVal::Value(Value::Ref(id, port)) => Some(ObjRef::new(id.clone(), port.clone())),
        PropVal::Value(Value::Str(s)) => {
            // Try to parse "id.port" from a bare string
            if let Some((id, port)) = s.split_once('.') {
                Some(ObjRef::new(id, Some(port.to_string())))
            } else {
                Some(ObjRef::new(s.clone(), None))
            }
        }
        _ => None,
    }
}

fn prop_as_gridpos(prop: &Prop) -> Option<GridPos> {
    match &prop.value {
        PropVal::Value(Value::Tuple(nums)) if nums.len() >= 2 => {
            Some(GridPos { x: nums[0], y: nums[1] })
        }
        _ => None,
    }
}

fn prop_as_ports(prop: &Prop, _decl_span: Span, diags: &mut DiagEngine) -> Vec<Port> {
    match &prop.value {
        PropVal::Value(Value::List(items)) => {
            items.iter().map(|s| Port { name: s.clone(), side: None }).collect()
        }
        PropVal::Value(Value::Str(s)) | PropVal::Value(Value::Ref(s, None)) => {
            vec![Port { name: s.clone(), side: None }]
        }
        PropVal::Block(sub_props) => {
            sub_props.iter().map(|p| {
                let side = match &p.value {
                    PropVal::Value(Value::Str(s)) | PropVal::Value(Value::Ref(s, None)) => {
                        Side::from_str(s)
                    }
                    _ => None,
                };
                if side.is_none() {
                    if let PropVal::Value(v) = &p.value {
                        let val_str = match v {
                            Value::Str(s) | Value::Ref(s, None) => s.as_str(),
                            _ => "",
                        };
                        if !val_str.is_empty() {
                            diags.emit(
                                Diagnostic::warning(format!(
                                    "unknown port side `{}` for port `{}`",
                                    val_str, p.key
                                ))
                                .with_span(p.span),
                            );
                        }
                    }
                }
                Port { name: p.key.clone(), side }
            }).collect()
        }
        _ => {
            diags.emit(
                Diagnostic::warning(format!(
                    "could not parse `ports` value: {:?}",
                    prop.value
                ))
                .with_span(prop.span),
            );
            vec![]
        }
    }
}

// ---- Normalizers ----

fn normalize_equipment(decl: &Decl, diags: &mut DiagEngine) -> Option<Equipment> {
    warn_unknown_props(decl, diags);
    let props = decl.form.props();

    let equip_type = match find_prop(props, "type").and_then(|p| prop_as_str(p)) {
        Some(t) => t.to_string(),
        None => {
            diags.emit(
                Diagnostic::error(format!(
                    "equipment `{}` is missing required property `type`",
                    decl.id
                ))
                .with_span(decl.span),
            );
            return None;
        }
    };

    let label = find_prop(props, "label").and_then(|p| prop_as_str(p)).map(String::from);
    let pos = find_prop(props, "at").and_then(|p| prop_as_gridpos(p));
    let size = find_prop(props, "size").and_then(|p| prop_as_gridpos(p));
    let attach = find_prop(props, "attach").and_then(|p| prop_as_ref(p));
    let ports = if let Some(p) = find_prop(props, "ports") {
        prop_as_ports(p, decl.span, diags)
    } else {
        vec![]
    };

    Some(Equipment {
        id: decl.id.clone(),
        equip_type,
        label,
        pos,
        ports,
        size,
        attach,
    })
}

fn normalize_valve(decl: &Decl, diags: &mut DiagEngine) -> Option<Valve> {
    warn_unknown_props(decl, diags);
    let props = decl.form.props();

    let valve_type = match find_prop(props, "type").and_then(|p| prop_as_str(p)) {
        Some(t) => t.to_string(),
        None => {
            diags.emit(
                Diagnostic::error(format!(
                    "valve `{}` is missing required property `type`",
                    decl.id
                ))
                .with_span(decl.span),
            );
            return None;
        }
    };

    let label = find_prop(props, "label").and_then(|p| prop_as_str(p)).map(String::from);
    let pos = find_prop(props, "at").and_then(|p| prop_as_gridpos(p));
    let actuator = find_prop(props, "actuator").and_then(|p| prop_as_str(p)).map(String::from);
    let fail = find_prop(props, "fail").and_then(|p| prop_as_str(p)).map(String::from);
    let state = find_prop(props, "state").and_then(|p| prop_as_str(p)).map(String::from);
    let setpoint = find_prop(props, "setpoint").and_then(|p| prop_as_str(p)).map(String::from);
    let ports = if let Some(p) = find_prop(props, "ports") {
        prop_as_ports(p, decl.span, diags)
    } else {
        vec![]
    };

    Some(Valve {
        id: decl.id.clone(),
        valve_type,
        label,
        pos,
        ports,
        actuator,
        fail,
        state,
        setpoint,
    })
}

fn normalize_line(decl: &Decl, diags: &mut DiagEngine) -> Option<Line> {
    warn_unknown_props(decl, diags);
    let props = decl.form.props();

    let class = match find_prop(props, "class").and_then(|p| prop_as_str(p)) {
        Some(c) => c.to_string(),
        None => {
            diags.emit(
                Diagnostic::error(format!(
                    "line `{}` is missing required property `class`",
                    decl.id
                ))
                .with_span(decl.span),
            );
            return None;
        }
    };

    let from = match find_prop(props, "from").and_then(|p| prop_as_ref(p)) {
        Some(r) => r,
        None => {
            diags.emit(
                Diagnostic::error(format!(
                    "line `{}` is missing required property `from`",
                    decl.id
                ))
                .with_span(decl.span),
            );
            return None;
        }
    };

    // `to` is optional: a line without one is an open-ended stub (drain,
    // vent, sample point) drawn outward from `from`.
    let to = find_prop(props, "to").and_then(|p| prop_as_ref(p));

    let label = find_prop(props, "label").and_then(|p| prop_as_str(p)).map(String::from);
    let flag = |key: &str| {
        find_prop(props, key)
            .and_then(|p| prop_as_str(p))
            .map(|v| v == "true" || v == "yes")
            .unwrap_or(false)
    };
    let flexible = flag("flexible");
    let insulated = flag("insulated");

    Some(Line {
        id: decl.id.clone(),
        class,
        from,
        to,
        label,
        flexible,
        insulated,
    })
}

fn normalize_instrument(decl: &Decl, diags: &mut DiagEngine) -> Option<Instrument> {
    warn_unknown_props(decl, diags);
    let props = decl.form.props();

    let instr_type = match find_prop(props, "type").and_then(|p| prop_as_str(p)) {
        Some(t) => t.to_string(),
        None => {
            diags.emit(
                Diagnostic::error(format!(
                    "instrument `{}` is missing required property `type`",
                    decl.id
                ))
                .with_span(decl.span),
            );
            return None;
        }
    };

    let label = find_prop(props, "label").and_then(|p| prop_as_str(p)).map(String::from);
    let pos = find_prop(props, "at").and_then(|p| prop_as_gridpos(p));
    let attach = find_prop(props, "attach").and_then(|p| prop_as_ref(p));
    let location = find_prop(props, "location").and_then(|p| prop_as_str(p)).map(String::from);

    Some(Instrument {
        id: decl.id.clone(),
        instr_type,
        label,
        pos,
        attach,
        location,
    })
}

fn normalize_signal(decl: &Decl, diags: &mut DiagEngine) -> Option<Signal> {
    warn_unknown_props(decl, diags);
    let props = decl.form.props();

    let sig_type = match find_prop(props, "type").and_then(|p| prop_as_str(p)) {
        Some(t) => t.to_string(),
        None => {
            diags.emit(
                Diagnostic::error(format!(
                    "signal `{}` is missing required property `type`",
                    decl.id
                ))
                .with_span(decl.span),
            );
            return None;
        }
    };

    let from = match find_prop(props, "from").and_then(|p| prop_as_ref(p)) {
        Some(r) => r,
        None => {
            diags.emit(
                Diagnostic::error(format!(
                    "signal `{}` is missing required property `from`",
                    decl.id
                ))
                .with_span(decl.span),
            );
            return None;
        }
    };

    let to = match find_prop(props, "to").and_then(|p| prop_as_ref(p)) {
        Some(r) => r,
        None => {
            diags.emit(
                Diagnostic::error(format!(
                    "signal `{}` is missing required property `to`",
                    decl.id
                ))
                .with_span(decl.span),
            );
            return None;
        }
    };

    let label = find_prop(props, "label").and_then(|p| prop_as_str(p)).map(String::from);

    Some(Signal {
        id: decl.id.clone(),
        sig_type,
        from,
        to,
        label,
    })
}

fn normalize_junction(decl: &Decl, diags: &mut DiagEngine) -> Junction {
    warn_unknown_props(decl, diags);
    let props = decl.form.props();
    let pos = find_prop(props, "at").and_then(|p| prop_as_gridpos(p));
    Junction { id: decl.id.clone(), pos }
}

fn normalize_note(decl: &Decl, diags: &mut DiagEngine) -> Note {
    warn_unknown_props(decl, diags);
    let props = decl.form.props();
    let pos = find_prop(props, "at").and_then(|p| prop_as_gridpos(p));
    let text = find_prop(props, "text").and_then(|p| prop_as_str(p)).map(String::from);
    Note { id: decl.id.clone(), text, pos }
}

fn normalize_group(decl: &Decl, diags: &mut DiagEngine) -> Option<Group> {
    warn_unknown_props(decl, diags);
    let props = decl.form.props();

    let members = match find_prop(props, "members") {
        Some(p) => match &p.value {
            PropVal::Value(Value::List(items)) => items.clone(),
            PropVal::Value(Value::Str(s)) | PropVal::Value(Value::Ref(s, None)) => {
                vec![s.clone()]
            }
            _ => {
                diags.emit(
                    Diagnostic::error(format!(
                        "group `{}` property `members` must be a comma-separated list",
                        decl.id
                    ))
                    .with_span(p.span),
                );
                return None;
            }
        },
        None => {
            diags.emit(
                Diagnostic::error(format!(
                    "group `{}` is missing required property `members`",
                    decl.id
                ))
                .with_span(decl.span),
            );
            return None;
        }
    };

    let label = find_prop(props, "label").and_then(|p| prop_as_str(p)).map(String::from);

    Some(Group {
        id: decl.id.clone(),
        members,
        label,
    })
}

fn normalize_area(decl: &Decl, diags: &mut DiagEngine) -> Area {
    warn_unknown_props(decl, diags);
    let props = decl.form.props();
    let label = find_prop(props, "label").and_then(|p| prop_as_str(p)).map(String::from);

    // bounds: two tuples or a list - complex, just skip for now
    // bounds: two tuples - simplified, not yet parsed
    if let Some(_bp) = find_prop(props, "bounds") {
        // TODO: parse bounds in a future version
    }

    Area {
        id: decl.id.clone(),
        label,
        bounds: None, // area bounds parsing is complex, skip for v0.1
    }
}
