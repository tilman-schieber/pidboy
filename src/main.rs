mod ast;
mod cli;
mod diag;
mod layout;
mod lexer;
mod model;
mod normalize;
mod parser;
mod render;
mod route;
mod span;
mod symbols;
mod validate;

use clap::Parser;
use std::process;

fn main() {
    let cli = cli::Cli::parse();

    let exit_code = match cli.command {
        cli::Commands::Compile {
            input,
            output,
            width,
            height,
            grid: _grid,
            no_route,
            pretty,
        } => cmd_compile(&input, output.as_deref(), width, height, no_route, pretty, cli.strict, cli.quiet, cli.verbose),
        cli::Commands::Check { input } => cmd_check(&input, cli.strict, cli.quiet),
        cli::Commands::DumpAst { input } => cmd_dump_ast(&input, cli.strict),
    };

    process::exit(exit_code);
}

fn read_source(path: &str) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("cannot read `{}`: {}", path, e))
}

fn run_pipeline(
    source: &str,
    filename: &str,
    strict: bool,
) -> (
    Option<model::Diagram>,
    diag::DiagEngine,
) {
    let mut diags = diag::DiagEngine::new().with_strict(strict);

    let lex_lines = lexer::lex(source, &mut diags);
    let doc = parser::parse(&lex_lines, &mut diags);
    let diagram = normalize::normalize(&doc, &mut diags);
    validate::validate(&diagram, &mut diags);

    // Print diagnostics
    diags.print_all(source, filename);

    if diags.has_errors() {
        (None, diags)
    } else {
        (Some(diagram), diags)
    }
}

fn cmd_compile(
    input: &str,
    output: Option<&str>,
    width: Option<u32>,
    height: Option<u32>,
    no_route: bool,
    pretty: bool,
    strict: bool,
    quiet: bool,
    _verbose: bool,
) -> i32 {
    let source = match read_source(input) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: {}", e);
            return 1;
        }
    };

    let (diagram, diags) = run_pipeline(&source, input, strict);

    let diagram = match diagram {
        Some(d) => d,
        None => {
            eprintln!("error: compilation failed with {} error(s)", diags.error_count());
            return 1;
        }
    };

    let layout = layout::compute_layout(&diagram);
    let routes = route::route(&diagram, &layout);

    let opts = render::SvgOptions {
        width,
        height,
        pretty,
        no_route,
    };

    let svg = render::svg::render(&diagram, &layout, &routes.segments, &opts);

    // Determine output path
    let out_path = match output {
        Some(p) => p.to_string(),
        None => {
            // Default: replace extension with .svg
            let stem = input.trim_end_matches(".pid");
            format!("{}.svg", stem)
        }
    };

    match std::fs::write(&out_path, &svg) {
        Ok(_) => {
            if !quiet {
                eprintln!("wrote {}", out_path);
            }
            0
        }
        Err(e) => {
            eprintln!("error: cannot write `{}`: {}", out_path, e);
            1
        }
    }
}

fn cmd_check(input: &str, strict: bool, quiet: bool) -> i32 {
    let source = match read_source(input) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: {}", e);
            return 1;
        }
    };

    let (result, diags) = run_pipeline(&source, input, strict);

    if result.is_some() {
        if !quiet {
            let w = diags.warning_count();
            if w > 0 {
                eprintln!("{}: ok ({} warning(s))", input, w);
            } else {
                eprintln!("{}: ok", input);
            }
        }
        0
    } else {
        eprintln!("{}: failed ({} error(s))", input, diags.error_count());
        1
    }
}

fn cmd_dump_ast(input: &str, strict: bool) -> i32 {
    let source = match read_source(input) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: {}", e);
            return 1;
        }
    };

    let mut diags = diag::DiagEngine::new().with_strict(strict);
    let lex_lines = lexer::lex(&source, &mut diags);
    let doc = parser::parse(&lex_lines, &mut diags);
    let diagram = normalize::normalize(&doc, &mut diags);
    validate::validate(&diagram, &mut diags);

    diags.print_all(&source, input);

    // Dump diagram in a human-readable format
    println!("# Diagram dump for {}", input);
    println!();

    println!("## Equipment ({} items)", diagram.equipment.len());
    for (id, eq) in &diagram.equipment {
        println!("  {} [{}]", id, eq.equip_type);
        if let Some(pos) = &eq.pos {
            println!("    at: ({}, {})", pos.x, pos.y);
        }
        if let Some(label) = &eq.label {
            println!("    label: {:?}", label);
        }
        if !eq.ports.is_empty() {
            let port_strs: Vec<String> = eq.ports.iter().map(|p| {
                if let Some(side) = p.side {
                    format!("{}:{:?}", p.name, side)
                } else {
                    p.name.clone()
                }
            }).collect();
            println!("    ports: {}", port_strs.join(", "));
        }
    }
    println!();

    println!("## Valves ({} items)", diagram.valves.len());
    for (id, v) in &diagram.valves {
        println!("  {} [{}]", id, v.valve_type);
        if let Some(act) = &v.actuator {
            println!("    actuator: {}", act);
        }
        if let Some(fail) = &v.fail {
            println!("    fail: {}", fail);
        }
    }
    println!();

    println!("## Lines ({} items)", diagram.lines.len());
    for (id, l) in &diagram.lines {
        println!("  {} [{}]: {} -> {}", id, l.class, l.from, l.to);
    }
    println!();

    println!("## Instruments ({} items)", diagram.instruments.len());
    for (id, instr) in &diagram.instruments {
        println!("  {} [{}]", id, instr.instr_type);
        if let Some(attach) = &instr.attach {
            println!("    attach: {}", attach);
        }
    }
    println!();

    println!("## Signals ({} items)", diagram.signals.len());
    for (id, s) in &diagram.signals {
        println!("  {} [{}]: {} -> {}", id, s.sig_type, s.from, s.to);
    }

    if diags.has_errors() { 1 } else { 0 }
}
