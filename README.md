# pidboy

A command-line compiler that reads a semantic P&ID DSL and emits SVG.

The source of truth is the DSL text file: it describes **what** is in the
diagram and **how it is connected**. Placement, routing, and drawing
geometry are computed by the compiler — objects without coordinates are
laid out automatically along their piping connectivity. Output is
deterministic.

Rendered symbols and line styles follow **ISO 10628** (flow diagram
symbols/piping) and **ISA-5.1** (instrumentation bubbles and signal
lines); see [Drawing conventions](#drawing-conventions).

---

## Installation

```
cargo build --release
```

The binary is `target/release/pidc`.

---

## Usage

### Compile to SVG

```
pidc compile input.pid -o output.svg --pretty --legend
```

Options:

```
-o, --output <FILE>    Output SVG path (default: input path with .svg)
    --width <INT>      Canvas width override
    --height <INT>     Canvas height override
    --grid <INT>       Layout grid scale in pixels (default: 80)
    --no-route         Skip routing, draw direct connections
    --pretty           Pretty-print SVG output
    --legend           Append a legend explaining every symbol used
    --strict           Treat warnings as errors
-q, --quiet
-v, --verbose
```

`--legend` collects the symbol styles actually present in the diagram —
equipment/valve/instrument symbols, junctions, line classes, signal
types — and renders a boxed legend below the drawing. Unused symbols
never appear; the canvas grows downward so the legend cannot clash with
the diagram.

### Validate without rendering

```
pidc check input.pid
```

Exits nonzero if there are errors. Prints diagnostics to stderr.

### Debug: dump normalized AST

```
pidc dump-ast input.pid
```

---

## Examples

Complete worked examples live in `examples/` (each `.pid` alongside its
compiled `.svg`, generated with `--pretty --legend`):

| File | Shows |
|------|-------|
| `boiler_feed_water.pid` | Explicit grid coordinates, flow + level control loops, off-page connectors |
| `reactor_cooling.pid` | Fully automatic layout, once-through utility runs via connectors, three control loops |
| `three_phase_separator.pid` | Faithful replica of a classic separator P&ID: multi-nozzle drum with internals, bypass stations, drains, relief to flare |

Regenerate them after renderer changes:

```
for f in examples/*.pid; do pidc compile "$f" --pretty --legend -o "${f%.pid}.svg"; done
```

---

## DSL

The full language reference is in [`dsl.md`](dsl.md). A condensed
generation-oriented reference lives in `.claude/skills/pid-dsl/SKILL.md`.

### Declaration forms

**Inline:**

```
line L100 class=process from=P101.out to=CV101.in
```

**Block:**

```
line L100:
  class: process
  from: P101.out
  to: CV101.in
```

A declaration is either inline or block, not both. Indentation is 2
spaces; tabs are an error.

### Declaration kinds

| Kind | Description |
|------|-------------|
| `equipment` | Major process equipment (pump, vessel, exchanger, connector, ...) |
| `valve` | Valve (gate, globe, control_valve, relief_valve, ...) |
| `line` | Piping connection (`to:` optional — omit for an open-ended drain/vent stub) |
| `instrument` | Indicator, transmitter, controller, alarm |
| `signal` | Signal connection (electrical, pneumatic, ...) |
| `junction` | Explicit tee or branch point (accepts directional taps like `J1.south`) |
| `note` | Free annotation text |
| `group` | Logical grouping of objects |
| `area` | Placement or zoning hint |

### Feature highlights

- **Ports with sides** — `ports: in: west, out: east`; multiple ports on
  the same side are distributed evenly in declaration order, so a vessel
  can carry several top nozzles.
- **Off-page connectors** — `type: connector` draws a pentagon flag for
  streams entering/leaving the sheet (utility headers, flare, battery
  limits). Prefer once-through utility runs over closed recycle loops.
- **Open-ended stubs** — a `line` without `to:` draws a short open run
  outward from its port (drains, vents, sample points).
- **Bypass loops** — junction taps (`J1.south → BPV.in`, `BPV.out →
  J2.south`) draw the classic control-valve bypass rectangle.
- **Attached instruments** — `attach: SEP.lt_w` hangs the bubble on that
  port's side with a leader line; plain `attach: SEP` places it above.
- **Vertical valves** — a valve whose ports are all north/south renders
  rotated 90° so it sits properly in a vertical run.

---

## Drawing conventions

These are deliberate decisions; keep them stable.

### Standards

- **Equipment and valve symbols** follow **ISO 10628-2** shapes;
  instrument bubbles follow **ISA-5.1** (style keyed to `location`:
  field / panel / control_room / shared display).
- **Signal lines** follow **ISA-5.1**:
  - *pneumatic* — solid line crossed by pairs of slash marks (`—//—`)
  - *electrical* — dashed `6,3`, weight 1.5 (ISA dashed-electric; keeps
    signals visually distinct from solid process piping)
- **Piping line styles** (each family has a distinct cadence so they
  can't be confused at print scale — only *process* is a solid line):
  - *process* — solid, weight 2
  - *utility* — long dash `14,5`, weight 1
  - *drain* — dash-dot `9,3,1.5,3`, weight 1
  - *vent* — round dots `1.5,4`, weight 1
- **Flow arrows** are drawn at the destination of every piping run and
  signal; instrument leader lines carry none.

### SVG viewer compatibility

The output must render correctly in Adobe Illustrator and other strict
SVG 1.1 importers, which constrains the generated markup:

- **No `<marker>` elements.** Illustrator drops polylines carrying
  `marker-end`. Arrowheads and pneumatic slash marks are explicit
  `<path>` elements.
- **No `<symbol>` elements.** Symbols are `<g>` inside `<defs>`,
  referenced with both `href` and `xlink:href`.
- **No CSS-only styling.** Every element carries explicit presentation
  attributes alongside its class; the embedded stylesheet is a
  convenience, not a requirement.

Verify renderer changes in at least Chrome **and** rsvg before shipping.

---

## Architecture

```
source text
  -> lexer         (src/lexer.rs)
  -> parser        (src/parser.rs, src/ast.rs)
  -> normalize     (src/normalize.rs)
  -> validate      (src/validate.rs)
  -> layout        (src/layout.rs)
  -> route         (src/route.rs)
  -> renderer      (src/render/svg.rs)
```

The parser does not know about SVG. The renderer does not know about
source syntax. The router consumes typed semantic objects. Additional
output backends can be added under `src/render/` without touching the
pipeline above the renderer.

### Symbol geometry layer

`src/symbols/mod.rs` defines backend-agnostic symbol geometry as
`SymbolDef` / `SymbolElement` structs (rects, circles, paths, lines,
filled dots/paths, small annotation text). Each renderer consumes these
and translates them into its own output format.

**Known coupling:** `SymbolElement::Path` stores curves as SVG path
strings (`d` attribute syntax). A non-SVG renderer must parse or wrap
these strings. Do not add SVG-structural constructs (raw markup, `<use>`
references, etc.) to `src/symbols/` — those belong in the renderer.

---

## Layout

Objects with an `at` coordinate are placed at that grid position
(multiplied by the grid scale, default 80 px/unit). Everything else is
placed automatically:

1. Placement propagates along declared lines: each line with one placed
   endpoint pulls its other endpoint next to it, anchored on the actual
   port the line connects to, on the side that port faces. Orthogonal
   port pairs place the target around a pipe corner. Unconnected
   components are seeded below existing content.
2. Attached instruments go outward from their attach point (or port
   side), nudging clear of symbols and of the corridors of ports that
   lines use.
3. Controllers are placed above the valve they actuate; if that spot is
   congested they fall back to their transmitter peer.
4. Leftovers land in a row below the diagram; finally everything is
   shifted to respect the canvas margin.

## Routing

Lines and signals are routed as orthogonal (Manhattan) polylines. For
each connection the router generates a candidate set — direct L-bends,
Z-bends through the midpoint, local hops at two step sizes, and escape
routes around the obstacle field — and scores it by symbol collisions,
bends, length, direction reversals, and collinear overlap with
already-routed lines. Routes leave ports through a short stub in the
port's direction; center-anchored endpoints (instrument bubbles) are
trimmed back to the symbol boundary so arrowheads stay visible; signals
into actuated valves terminate on the actuator head.

---

## Tests

```
cargo test
```
