use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "pidc", version, about = "P&ID DSL compiler")]
pub struct Cli {
    /// Treat warnings as errors
    #[arg(long, global = true)]
    pub strict: bool,

    /// Suppress output
    #[arg(short, long, global = true)]
    pub quiet: bool,

    /// Verbose output
    #[arg(short, long, global = true)]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Compile a DSL file to SVG
    Compile {
        /// Input .pid file
        input: String,

        /// Output SVG file path
        #[arg(short, long)]
        output: Option<String>,

        /// Canvas width in SVG units
        #[arg(long)]
        width: Option<u32>,

        /// Canvas height in SVG units
        #[arg(long)]
        height: Option<u32>,

        /// Layout grid size (default: 80)
        #[arg(long)]
        grid: Option<u32>,

        /// Skip routing, draw direct connections
        #[arg(long)]
        no_route: bool,

        /// Pretty-print SVG output
        #[arg(long)]
        pretty: bool,

        /// Append a legend explaining every symbol used in the diagram
        #[arg(long)]
        legend: bool,
    },

    /// Validate a DSL file without generating output
    Check {
        /// Input .pid file
        input: String,
    },

    /// Dump the parsed AST for debugging
    DumpAst {
        /// Input .pid file
        input: String,
    },
}
