use crate::diag::{DiagEngine, Diagnostic};
use crate::model::*;

// ---- Controlled vocabularies ----

const EQUIPMENT_TYPES: &[&str] = &[
    "pump", "pump_centrifugal", "pump_positive_displacement",
    "heat_exchanger", "heat_exchanger_shell_tube",
    "tank", "vessel", "separator", "separator_3phase",
    "reactor_cstr", "reactor_batch", "reactor_pfr",
    "compressor", "blower", "mixer", "distillation_column",
    "connector", "heat_pad",
];

const VALVE_TYPES: &[&str] = &[
    "gate", "globe", "ball", "butterfly", "plug",
    "control_valve", "check_valve", "relief_valve", "safety_valve",
];

const INSTRUMENT_TYPES: &[&str] = &[
    "temperature_indicator", "pressure_indicator", "flow_indicator", "level_indicator",
    "temperature_transmitter", "pressure_transmitter", "flow_transmitter", "level_transmitter",
    "temperature_controller", "pressure_controller", "flow_controller", "level_controller",
    "alarm", "relay", "transducer", "level_gauge",
];

const LINE_CLASSES: &[&str] = &["process", "utility", "drain", "vent"];

const SIGNAL_TYPES: &[&str] = &["electrical", "pneumatic", "hydraulic", "digital"];

const LOCATIONS: &[&str] = &["field", "panel", "control_room", "shared"];

pub fn validate(diagram: &Diagram, diags: &mut DiagEngine) {
    validate_equipment(diagram, diags);
    validate_valves(diagram, diags);
    validate_lines(diagram, diags);
    validate_instruments(diagram, diags);
    validate_signals(diagram, diags);
}

fn validate_equipment(diagram: &Diagram, diags: &mut DiagEngine) {
    for eq in diagram.equipment.values() {
        if !EQUIPMENT_TYPES.contains(&eq.equip_type.as_str()) {
            diags.emit(Diagnostic::warning(format!(
                "unknown equipment type `{}` for `{}`; known types: {}",
                eq.equip_type,
                eq.id,
                EQUIPMENT_TYPES.join(", ")
            )));
        }
        if let Some(attach) = &eq.attach {
            validate_ref(diagram, attach, &format!("equipment `{}`", eq.id), "attach", diags);
        }
    }
}

fn validate_valves(diagram: &Diagram, diags: &mut DiagEngine) {
    for v in diagram.valves.values() {
        if !VALVE_TYPES.contains(&v.valve_type.as_str()) {
            diags.emit(Diagnostic::warning(format!(
                "unknown valve type `{}` for `{}`; known types: {}",
                v.valve_type,
                v.id,
                VALVE_TYPES.join(", ")
            )));
        }
    }
}

fn validate_lines(diagram: &Diagram, diags: &mut DiagEngine) {
    for line in diagram.lines.values() {
        // Validate class
        if !LINE_CLASSES.contains(&line.class.as_str()) {
            diags.emit(Diagnostic::warning(format!(
                "unknown line class `{}` for `{}`; known classes: {}",
                line.class,
                line.id,
                LINE_CLASSES.join(", ")
            )));
        }

        // Validate from reference
        validate_ref(diagram, &line.from, &format!("line `{}`", line.id), "from", diags);

        // Validate to reference
        if let Some(to) = &line.to {
            validate_ref(diagram, to, &format!("line `{}`", line.id), "to", diags);
        }
    }
}

fn validate_instruments(diagram: &Diagram, diags: &mut DiagEngine) {
    for instr in diagram.instruments.values() {
        if !INSTRUMENT_TYPES.contains(&instr.instr_type.as_str()) {
            diags.emit(Diagnostic::warning(format!(
                "unknown instrument type `{}` for `{}`; known types: {}",
                instr.instr_type,
                instr.id,
                INSTRUMENT_TYPES.join(", ")
            )));
        }

        if let Some(loc) = &instr.location {
            if !LOCATIONS.contains(&loc.as_str()) {
                diags.emit(Diagnostic::warning(format!(
                    "unknown location `{}` for `{}`; known locations: {}",
                    loc,
                    instr.id,
                    LOCATIONS.join(", ")
                )));
            }
        }

        if let Some(attach) = &instr.attach {
            validate_ref(diagram, attach, &format!("instrument `{}`", instr.id), "attach", diags);
        }
    }
}

fn validate_signals(diagram: &Diagram, diags: &mut DiagEngine) {
    for sig in diagram.signals.values() {
        if !SIGNAL_TYPES.contains(&sig.sig_type.as_str()) {
            diags.emit(Diagnostic::warning(format!(
                "unknown signal type `{}` for `{}`; known types: {}",
                sig.sig_type,
                sig.id,
                SIGNAL_TYPES.join(", ")
            )));
        }

        validate_ref(diagram, &sig.from, &format!("signal `{}`", sig.id), "from", diags);
        validate_ref(diagram, &sig.to, &format!("signal `{}`", sig.id), "to", diags);
    }
}

fn validate_ref(diagram: &Diagram, obj_ref: &ObjRef, context: &str, prop: &str, diags: &mut DiagEngine) {
    if !diagram.id_exists(&obj_ref.id) {
        diags.emit(Diagnostic::error(format!(
            "{} property `{}` references unknown ID `{}`",
            context, prop, obj_ref.id
        )));
        return;
    }

    // Validate port if specified
    if let Some(port_name) = &obj_ref.port {
        if let Some(ports) = diagram.get_ports(&obj_ref.id) {
            if !ports.is_empty() && !ports.iter().any(|p| &p.name == port_name) {
                diags.emit(Diagnostic::error(format!(
                    "reference `{}.{}` points to missing port `{}`; declared ports: {}",
                    obj_ref.id,
                    port_name,
                    port_name,
                    ports.iter().map(|p| p.name.as_str()).collect::<Vec<_>>().join(", ")
                )));
            }
        }
        // If no ports declared, any port name is acceptable
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diag::DiagEngine;
    use crate::lexer::lex;
    use crate::parser::parse;
    use crate::normalize::normalize;

    fn compile(src: &str) -> (Diagram, DiagEngine) {
        let mut diags = DiagEngine::new();
        let lines = lex(src, &mut diags);
        let doc = parse(&lines, &mut diags);
        let diagram = normalize(&doc, &mut diags);
        validate(&diagram, &mut diags);
        (diagram, diags)
    }

    #[test]
    fn test_duplicate_ids() {
        let src = "equipment P101:\n  type: pump\n\nequipment P101:\n  type: vessel\n";
        let (_d, diags) = compile(src);
        assert!(diags.has_errors());
        assert!(diags.diagnostics.iter().any(|d| d.message.contains("duplicate")));
    }

    #[test]
    fn test_unresolved_ref() {
        let src = "line L1:\n  class: process\n  from: MISSING\n  to: ALSOMISSING\n";
        let (_d, diags) = compile(src);
        assert!(diags.has_errors());
    }

    #[test]
    fn test_bad_port_ref() {
        let src = "equipment P101:\n  type: pump\n  ports: in, out\n\nline L1:\n  class: process\n  from: P101.nosuchport\n  to: P101.out\n";
        let (_d, diags) = compile(src);
        assert!(diags.has_errors());
    }

    #[test]
    fn test_missing_required_prop() {
        let src = "equipment P101:\n  at: (1,1)\n";
        let (_d, diags) = compile(src);
        assert!(diags.has_errors());
    }

    #[test]
    fn test_unknown_enum() {
        let src = "equipment P101:\n  type: magical_unicorn\n";
        let (_d, diags) = compile(src);
        // Should have a warning (not an error) for unknown type
        assert!(diags.diagnostics.iter().any(|d| d.message.contains("unknown equipment type")));
    }
}
