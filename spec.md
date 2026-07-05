# spec.md — Rust Compiler for P&ID DSL to SVG (v0.1)

> **Status: historical design document.** This is the original v0.1
> implementation spec, kept for context. The implementation has moved
> past it — see the [v0.2 addendum](#v02-addendum--implemented-beyond-this-spec)
> at the end, `README.md` for current behavior, and `dsl.md` for the
> current language reference.

## Objective

Build a command-line compiler in Rust that reads the P&ID DSL defined in `dsl.md`, validates it, normalizes it into a typed intermediate representation, performs basic layout and orthogonal routing, and emits **SVG only** in the first implementation phase.

This project is a **compiler**, not an editor. The source of truth is the DSL text file. The output of the tool is deterministic.

The first milestone is **SVG generation only**. Do not implement TikZ/TeX generation yet, but design the architecture so that additional renderers can be added later without major refactoring.

---

## Non-goals for v0.1

Do **not** implement the following in the first phase:

- TikZ or TeX output
- GUI editor
- web frontend
- arbitrary deep nesting in the DSL
- free-form manual path drawing
- automatic import from existing CAD or P&ID tools
- full ISA/ISO standard coverage
- optimization-heavy global auto-layout
- interactive SVG editing
- symbol library loaded from external files unless needed later

---

## Deliverables

The compiler must provide:

1. A Rust CLI executable
2. Parsing for the DSL in `dsl.md`
3. Validation with actionable diagnostics
4. A normalized typed AST / IR
5. Basic placement support using explicit `at` positions
6. Basic orthogonal routing for lines and signals
7. SVG emission
8. Snapshot-friendly output for testing
9. Clear module boundaries so later output backends can be added

---

## Required CLI behavior

Create a CLI binary named `pidc`.

### Commands

#### 1. `compile`
Compile a DSL file into SVG.

```bash
pidc compile input.pid -o output.svg
```

Required behavior:
- read source file
- parse
- validate
- normalize
- route
- render SVG
- write to output path

#### 2. `check`
Validate a DSL file without generating SVG.

```bash
pidc check input.pid
```

Required behavior:
- parse
- validate
- print diagnostics
- exit nonzero on error

#### 3. `fmt` (optional in v0.1, desirable if cheap)
Normalize formatting of the DSL source.

```bash
pidc fmt input.pid
```

May be deferred if it slows down core compiler work.

#### 4. `dump-ast`
Emit the normalized internal representation in a debug-friendly format.

```bash
pidc dump-ast input.pid
```

Use a stable textual format suitable for debugging and tests.

---

## Required CLI flags

### For `compile`
- `-o, --output <FILE>`: output SVG path
- `--width <INT>`: optional canvas width
- `--height <INT>`: optional canvas height
- `--grid <INT>`: layout grid size, default sensible value
- `--no-route`: optional debug mode, render direct connections without routing if possible
- `--pretty`: pretty-print SVG output

### Common flags
- `--strict`: treat warnings as errors
- `--color <auto|always|never>`: diagnostic color output
- `-q, --quiet`
- `-v, --verbose`

---

## High-level architecture

The implementation must be split into these conceptual stages:

```txt
source text
  -> lexer/parser
  -> concrete syntax handling
  -> normalized AST / semantic IR
  -> validation
  -> layout preparation
  -> orthogonal routing
  -> SVG renderer
```

### Design rule
The parser must not know about SVG.
The SVG renderer must not know about source syntax details.
The router must consume semantic objects, not text fragments.

---

## Rust project structure

Suggested crate layout:

```txt
src/
  main.rs
  cli.rs
  diag.rs
  span.rs
  lexer.rs
  parser.rs
  cst.rs
  ast.rs
  normalize.rs
  validate.rs
  model.rs
  layout.rs
  route.rs
  render/
    mod.rs
    svg.rs
  symbols/
    mod.rs
  tests/
```

This can be adjusted, but the responsibilities must remain separated.

### Module responsibilities

#### `cli.rs`
- argument parsing with `clap`
- command dispatch
- output handling
- exit codes

#### `span.rs`
- source spans
- line/column mapping
- utilities for diagnostics

#### `diag.rs`
- parse and validation diagnostics
- warnings/errors
- formatted terminal output

#### `lexer.rs`
- tokenization for the DSL
- indentation handling
- comments
- punctuation
- identifiers
- quoted strings

#### `parser.rs`
- parse declarations
- parse inline vs block forms
- preserve spans
- produce CST or directly produce a permissive AST if preferred

#### `ast.rs`
- syntax-level data structures
- declaration nodes
- property values
- blocks and inline properties

#### `normalize.rs`
- convert parsed forms into normalized semantic objects
- unify inline and block declarations
- normalize tuples/lists/references
- normalize coordinates and ports

#### `validate.rs`
- semantic validation
- required properties by kind
- uniqueness
- reference resolution
- enum checks
- property legality by declaration kind

#### `model.rs`
- typed semantic IR used by layout/routing/rendering
- this is the main compiler-internal representation

#### `layout.rs`
- derive initial positions
- calculate bounds
- apply grid scaling
- prepare symbol anchor points

#### `route.rs`
- orthogonal routing
- simple obstacle avoidance
- attach line endpoints to ports
- produce routed polylines

#### `render/svg.rs`
- render model + routed geometry to SVG
- deterministic output order
- optional pretty formatting

#### `symbols/mod.rs`
- built-in symbol geometry for equipment/valves/instruments
- no external symbol-loading required in v0.1

---

## Crate/library recommendations

Use these where useful:

- `clap` for CLI
- `thiserror` for error types
- `miette` or `ariadne` for diagnostics if desired
- `indexmap` for deterministic insertion-order maps where useful
- `serde` optionally for debug dumps only
- `roxmltree` is **not needed**
- avoid heavy GUI/web dependencies

Do not over-engineer. Prefer a small, robust dependency set.

---

## Source language support

The compiler must implement the DSL defined in `dsl.md`.

### Supported declaration kinds in v0.1

- `equipment`
- `valve`
- `line`
- `instrument`
- `signal`
- `group`
- `area`
- `note`
- `junction`

### Supported syntax forms

#### Inline declaration
```txt
line L100 class=process from=P101.out to=CV101.in
```

#### Block declaration
```txt
line L100:
  class: process
  from: P101.out
  to: CV101.in
```

### Indentation rules
- 2-space indentation only
- tabs in indentation are an error
- nesting deeper than one property block should be rejected or warned in v0.1
- comments starting with `#` must be ignored
- blank lines allowed

### Mixed form rule
A declaration is either inline or block, not both.

---

## Semantic model requirements

Create a typed model that distinguishes the following concepts.

### Diagram
Top-level container for all objects and metadata.

### Equipment
Fields should include at least:
- `id`
- `kind` or `equipment_type`
- `label`
- `position_hint`
- `ports`

### Valve
Fields should include at least:
- `id`
- `valve_type`
- `label`
- `position_hint`
- `actuator`
- `fail_mode`
- `ports`

### Instrument
Fields should include at least:
- `id`
- `instrument_type`
- `label`
- `position_hint`
- `attach`
- `location`

### Line
Fields should include at least:
- `id`
- `class`
- `from`
- `to`
- `label`
- routing result placeholder

### Signal
Fields should include at least:
- `id`
- `signal_type`
- `from`
- `to`
- `label`
- routing result placeholder

### Junction
Fields should include at least:
- `id`
- `position_hint`

### Note
Fields should include at least:
- `id`
- `text`
- `position_hint`

### Group / Area
May exist semantically but do not need sophisticated rendering in v0.1.

---

## Value normalization rules

The compiler must normalize these consistently:

### Coordinates
Input:
```txt
at=(10,8)
```
or
```txt
at: (10,8)
```

Normalize to:
- integer grid coordinate pair

### Lists
Input:
```txt
ports: in, out
```

Normalize to:
- vector/list of strings or structured ports

### References
Input:
```txt
P101.out
```

Normalize to:
- `{ object_id: "P101", port: Some("out") }`

Input:
```txt
TI101
```

Normalize to:
- `{ object_id: "TI101", port: None }`

### Ports
Support both:
```txt
ports: in, out
```

and:
```txt
ports:
  in: west
  out: east
```

Normalize to a structured port model with:
- `name`
- optional `side`

---

## Validation requirements

Validation must happen after parsing and normalization.

### Required checks

#### Global
- IDs are unique
- declaration kinds are known
- properties are valid for the declaration kind
- required properties exist

#### Connectivity
- `from` and `to` references resolve
- referenced ports exist if ports were explicitly declared
- `attach` references resolve

#### Enum validation
Validate controlled vocabularies for v0.1.

### Suggested controlled vocabularies

#### `equipment.type`
Support at least:
- `pump`
- `pump_centrifugal`
- `heat_exchanger`
- `tank`
- `vessel`
- `separator`
- `reactor_cstr`
- `reactor_batch`
- `reactor_pfr`
- `compressor`
- `blower`
- `mixer`
- `distillation_column`

#### `valve.type`
Support at least:
- `gate`
- `globe`
- `ball`
- `butterfly`
- `plug`
- `control_valve`
- `check_valve`
- `relief_valve`
- `safety_valve`

#### `instrument.type`
Support at least:
- `temperature_indicator`
- `pressure_indicator`
- `flow_indicator`
- `level_indicator`
- `temperature_transmitter`
- `pressure_transmitter`
- `flow_transmitter`
- `level_transmitter`
- `temperature_controller`
- `pressure_controller`
- `flow_controller`
- `level_controller`
- `alarm`

#### `line.class`
Support at least:
- `process`
- `utility`
- `drain`
- `vent`

#### `signal.type`
Support at least:
- `electrical`
- `pneumatic`
- `hydraulic`
- `digital`

#### `location`
Support at least:
- `field`
- `panel`
- `control_room`

#### `orient`
Support at least:
- `north`
- `south`
- `east`
- `west`

### Diagnostics
Diagnostics must include:
- message
- line and column
- source snippet if practical
- note/help message where useful

Diagnostics should be phrased for humans, not parser developers.

Example:
- “unknown property `foo` on `line` declaration”
- “reference `P101.out` points to missing port `out`”
- “tabs are not allowed for indentation”
- “declaration `equipment P101` mixes inline and block syntax”

---

## Layout requirements for v0.1

The first version does **not** need full automatic graph layout.

### Required behavior
- if an object has `at`, use it as the placement anchor
- if an object lacks `at`, assign a fallback position deterministically
- use a fixed grid scale to convert logical coordinates to SVG units
- compute diagram bounds automatically unless explicitly overridden

### Suggested approach
- logical grid spacing default: 80 units
- object anchor point centered on the symbol
- fallback placement order deterministic by declaration order

### Non-goals for layout v0.1
- global optimization
- sophisticated packing
- constraint solving
- auto-zoning

---

## Routing requirements for v0.1

Routing should be algorithmic and orthogonal.

### Required behavior
- route `line` and `signal` connections as Manhattan polylines
- connect to ports where available
- if no explicit port exists, infer a reasonable anchor
- avoid running through symbol interiors where possible
- prefer fewer bends
- produce deterministic output

### Acceptable v0.1 strategy
A simple router is sufficient, for example:
1. determine endpoint anchors
2. attempt horizontal-then-vertical route
3. attempt vertical-then-horizontal route
4. if blocked, try a simple detour on the routing grid
5. choose the route with fewer collisions and bends

### Obstacles
Treat symbol bounding boxes as routing obstacles.

### Signals vs lines
Render both through the same routing pipeline, but style them differently in SVG.

### Non-goals for routing v0.1
- full A* over a dense grid if not needed
- crossing minimization beyond simple heuristics
- bundled bus routing
- advanced obstacle negotiation

---

## SVG renderer requirements

The renderer must output standalone SVG.

### Required SVG structure
- root `<svg>` with width, height, and viewBox
- groups for:
  - defs
  - background optional
  - lines
  - signals
  - symbols
  - labels
  - notes

### Determinism
Output ordering must be stable:
- declaration order where sensible
- deterministic attribute ordering if practical
- identical input should produce identical SVG bytes when options match

### Rendering responsibilities

#### Equipment
Render built-in symbol shapes for known equipment types.

v0.1 may group many equipment types into simple canonical visual families, for example:
- pump-like
- exchanger-like
- vessel/tank-like
- column-like
- generic box fallback

#### Valve
Render a recognizable inline valve symbol.
Control valves may have a distinct marker from manual valves.

#### Instrument
Render instrument bubbles or simple standard markers.
Differentiate at least:
- indicators
- transmitters
- controllers
- alarms

#### Junction
Render a small filled node or tee marker.

#### Notes
Render text at the requested position.

#### Lines and signals
Render routed polylines.
Use different stroke/dash conventions for:
- process lines
- utility lines
- signal types

### Labels
Render labels for:
- equipment
- valves
- instruments
- notes
- line labels if present and feasible

### Styling
Use simple built-in styling in v0.1.
Do not depend on external CSS.
Allow future theming later.

---

## Built-in symbol system

Implement symbols as Rust code in v0.1.

### Design requirement
Separate semantic type from geometry.

Example:
- `equipment.type = pump_centrifugal`
- symbol resolver maps that to a built-in SVG shape recipe

### Symbol API
Design a symbol abstraction that can later support other backends.

Suggested direction:
- semantic symbol request -> abstract geometry description or backend-specific draw calls

But keep v0.1 pragmatic. Direct SVG construction is acceptable if code structure remains clean.

---

## Output behavior

### Success
`compile` writes an SVG file and exits 0.

### Failure
On parse or validation failure:
- write diagnostics to stderr
- do not write partial SVG unless explicitly in a future debug mode
- exit nonzero

---

## Testing requirements

The project must include automated tests.

### Required test categories

#### Parser tests
- inline declarations
- block declarations
- comments and blanks
- indentation errors
- references
- tuples and lists

#### Validation tests
- duplicate IDs
- missing required properties
- unknown properties
- bad enum values
- unresolved references
- bad ports

#### Routing tests
- simple straight route
- one-bend route
- simple obstacle detour
- deterministic route result

#### Rendering tests
- known input generates SVG
- stable output snapshots for representative diagrams
- minimal SVG sanity assertions

### Golden tests
Add a `tests/cases/` directory with:
- input DSL files
- expected outcome
- expected SVG snapshots for selected cases

---

## Error handling requirements

Do not crash on malformed input.
All user-facing failures must be surfaced as diagnostics.

Use proper error types and `Result` propagation.
Panics are acceptable only for internal invariants that truly indicate programmer error.

---

## Performance expectations

This is a CLI compiler, not a real-time system.

v0.1 performance targets:
- small and medium diagrams should compile quickly
- correctness, determinism, and maintainability matter more than micro-optimization

Do not prematurely optimize.

---

## Suggested implementation order

Implement in this sequence:

1. CLI skeleton
2. spans and diagnostics
3. lexer
4. parser
5. normalization
6. semantic model
7. validation
8. minimal SVG renderer with symbols placed at fixed positions
9. basic routing
10. labels and polishing
11. tests and snapshot stabilization

This order is preferred because it produces visible results early while preserving architecture quality.

---

## Acceptance criteria for v0.1

The first milestone is complete when all of the following are true:

1. `pidc check sample.pid` validates valid files and rejects invalid files with useful diagnostics
2. `pidc compile sample.pid -o sample.svg` produces a valid standalone SVG
3. Inline and block syntax both work
4. Core declaration kinds are supported
5. Explicit positions via `at` are honored
6. Lines and signals are orthogonally routed in simple cases
7. Output is deterministic
8. Test coverage exists for parsing, validation, routing, and rendering
9. The codebase is clearly structured for future addition of a TikZ renderer

---

## Example input that must compile

```txt
equipment P101:
  type: pump
  at: (10,8)
  ports: in, out
  label: "P-101"

valve CV101:
  type: control_valve
  actuator: pneumatic
  fail: closed
  at: (16,8)
  ports: in, out
  label: "CV-101"

equipment E101:
  type: heat_exchanger
  at: (24,8)
  ports:
    in: west
    out: east
    top: north
  label: "E-101"

line L100:
  class: process
  from: P101.out
  to: CV101.in

line L101:
  class: process
  from: CV101.out
  to: E101.in

instrument TI101:
  type: temperature_indicator
  attach: E101.top
  location: field
  label: "TI-101"

instrument TIC101:
  type: temperature_controller
  at: (24,3)
  location: panel
  label: "TIC-101"

signal S100:
  type: electrical
  from: TI101
  to: TIC101

signal S101:
  type: pneumatic
  from: TIC101
  to: CV101
```

The compiler must successfully render this to SVG in v0.1.

---

## Final design principles

- Keep the source language semantic
- Keep routing algorithmic
- Keep renderer modular
- Keep output deterministic
- Prefer clarity over cleverness
- Build a real compiler pipeline, not a string transformation script

---

## v0.2 addendum — implemented beyond this spec

The shipped compiler extends v0.1 in the following ways (authoritative
descriptions in `README.md` and `dsl.md`):

**Language**
- `line.to` is optional: omitting it draws an open-ended stub (drains,
  vents, sample points).
- Junctions accept directional taps (`J1.south`) although they declare
  no ports; this is the bypass-loop idiom.
- Ports sharing a side are distributed evenly in declaration order.
- New equipment types: `separator_3phase` (drum with drawn internals),
  `connector` (off-page flag). New instrument location: `shared`.
  New actuator value: `diaphragm`.
- `attach: X.port` places the instrument on that port's side.

**Layout & routing**
- Auto-placement is connectivity-driven (port-anchored, corner-aware),
  not a fallback grid. Controllers place above the valve they actuate.
- The router scores a candidate set (L/Z/local-hop/escape) by
  collisions, bends, length, reversals, and overlap with earlier
  routes; endpoints trim to symbol boundaries; signals into actuated
  valves land on the actuator head.

**Rendering**
- Symbols/line styles follow ISO 10628 and ISA-5.1: pneumatic signals
  are solid with slash-mark pairs; drain/vent/utility use distinct dash
  cadences; flow arrowheads on all piping.
- No SVG `<marker>`/`<symbol>`/CSS-only styling — output must survive
  Adobe Illustrator's importer (see `CLAUDE.md` for the hard rules).
- `pidc compile --legend` appends an auto-generated legend of exactly
  the symbols used.
- Vertical valves (all-N/S ports) render rotated 90°.
