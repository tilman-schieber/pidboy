mod cli;
mod serve;

use clap::Parser;
use pidc::{compile, render};
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
            legend,
            table,
            title,
            footer,
        } => cmd_compile(&input, output.as_deref(), width, height, no_route, pretty, legend, table, title, footer, cli.strict, cli.quiet, cli.verbose),
        cli::Commands::Check { input } => cmd_check(&input, cli.strict, cli.quiet),
        cli::Commands::DumpAst { input } => cmd_dump_ast(&input, cli.strict),
        cli::Commands::Serve { input, port } => serve::serve(&input, port),
    };

    process::exit(exit_code);
}

fn read_source(path: &str) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("cannot read `{}`: {}", path, e))
}

fn cmd_compile(
    input: &str,
    output: Option<&str>,
    width: Option<u32>,
    height: Option<u32>,
    no_route: bool,
    pretty: bool,
    legend: bool,
    table: bool,
    title: Option<String>,
    footers: Vec<String>,
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

    let opts = render::SvgOptions {
        width,
        height,
        pretty,
        no_route,
        legend,
        table,
        title,
        footers,
    };

    let result = compile::compile_to_parts(&source, strict, &opts);
    result.diags.print_all(&source, input);

    let svg = match result.svg {
        Some(s) => s,
        None => {
            eprintln!("error: compilation failed with {} error(s)", result.diags.error_count());
            return 1;
        }
    };

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

    let (_doc, _diagram, diags) = compile::analyze(&source, strict);
    diags.print_all(&source, input);

    if !diags.has_errors() {
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

    let (_doc, diagram, diags) = compile::analyze(&source, strict);

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
        let to = l
            .to
            .as_ref()
            .map(|t| t.to_string())
            .unwrap_or_else(|| "(open end)".to_string());
        println!("  {} [{}]: {} -> {}", id, l.class, l.from, to);
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
