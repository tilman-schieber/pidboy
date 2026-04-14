# pidboy

A command-line compiler that reads a semantic P&ID DSL and emits SVG.

The source of truth is the DSL text file. Output is deterministic.

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
pidc compile input.pid -o output.svg
```

Options:

```
-o, --output <FILE>    Output SVG path (required)
    --width <INT>      Canvas width override
    --height <INT>     Canvas height override
    --grid <INT>       Layout grid scale in pixels (default: 80)
    --no-route         Skip routing, draw direct connections
    --pretty           Pretty-print SVG output
    --strict           Treat warnings as errors
-q, --quiet
-v, --verbose
```

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

## DSL

The DSL describes what is in the diagram and how it is connected. Placement, routing, and geometry are determined by the compiler.

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

A declaration is either inline or block, not both.

### Declaration kinds

| Kind | Description |
|------|-------------|
| `equipment` | Major process equipment (pump, vessel, exchanger, ...) |
| `valve` | Valve (gate, ball, control_valve, ...) |
| `line` | Process or utility piping connection |
| `instrument` | Indicator, transmitter, controller, alarm |
| `signal` | Signal connection (electrical, pneumatic, ...) |
| `junction` | Explicit tee or branch point |
| `note` | Free annotation text |
| `group` | Logical grouping of objects |
| `area` | Placement or zoning hint |

### Value types

| Syntax | Meaning |
|--------|---------|
| `pump` | Bare string (enum value, scalar) |
| `"P-101"` | Quoted string |
| `(10,8)` | Tuple (used for coordinates) |
| `in,out` | List |
| `P101.out` | Reference (object + port) |
| `TI101` | Reference (object only) |

### Properties by kind

**equipment**

```
equipment P101:
  type: pump            # required; pump, heat_exchanger, tank, vessel, ...
  at: (10,8)            # grid position
  ports: in, out        # simple port list
  label: "P-101"
  orient: east
```

Structured ports with sides:

```
equipment E101:
  type: heat_exchanger
  at: (24,8)
  ports:
    in: west
    out: east
    top: north
  label: "E-101"
```

**valve**

```
valve CV101:
  type: control_valve   # required; gate, ball, butterfly, relief_valve, ...
  actuator: pneumatic   # manual, pneumatic, electric
  fail: closed          # open, closed, last
  at: (16,8)
  ports: in, out
  label: "CV-101"
```

**line**

```
line L100:
  class: process        # required; process, utility, drain, vent
  from: P101.out        # required
  to: CV101.in          # required
  label: "3in-P-1023"
```

**instrument**

```
instrument TI101:
  type: temperature_indicator   # required
  attach: E101.top              # attach to equipment port
  location: field               # field, panel, control_room
  label: "TI-101"

instrument TIC101:
  type: temperature_controller
  at: (24,3)
  location: panel
  label: "TIC-101"
```

**signal**

```
signal S100:
  type: electrical      # required; electrical, pneumatic, hydraulic, digital
  from: TI101           # required
  to: TIC101            # required
```

**junction**

```
junction J1 at=(18,8)
```

**note**

```
note N1:
  at: (5,2)
  text: "Normally closed during startup"
```

**group**

```
group cooling_loop:
  members: P101, CV101, E101
  label: "Cooling loop"
```

### Indentation rules

- 2 spaces per level only
- Tabs are an error
- Maximum nesting depth: one property block inside a declaration

### Comments

```
# This is a comment
equipment P101:
  type: pump  # inline comment
```

---

## Example

`tests/cases/sample.pid`:

```
# Simple control loop example

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

Compile it:

```
pidc compile tests/cases/sample.pid -o sample.svg
```

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
  -> renderer      (src/render/svg.rs, src/render/tikz.rs, …)
```

The parser does not know about SVG. The renderer does not know about source syntax. The router consumes typed semantic objects.

Additional output backends (TikZ, etc.) can be added under `src/render/` without touching the pipeline above the renderer.

### Symbol geometry layer

`src/symbols/mod.rs` defines backend-agnostic symbol geometry as `SymbolDef` / `SymbolElement` structs. Each renderer consumes these and translates them into its own output format (SVG primitives, TikZ `\draw` commands, etc.).

**Known coupling:** `SymbolElement::Path` stores curves as SVG path strings (`d` attribute syntax). A non-SVG renderer must parse or wrap these strings. Do not add SVG-structural constructs (raw markup, `<use>` references, etc.) to `src/symbols/` — those belong in the renderer.

---

## Layout

Objects with an `at` coordinate are placed at that grid position. Grid coordinates are multiplied by the grid scale (default 80 px/unit) to produce SVG coordinates.

Objects without `at` are assigned fallback positions deterministically by declaration order. Instruments with `attach` but no `at` are placed adjacent to their attach target.

---

## Routing

Lines and signals are routed as orthogonal (Manhattan) polylines. The router tries horizontal-then-vertical and vertical-then-horizontal, checks for symbol bounding box collisions, and picks the better option.

---

## Tests

```
cargo test
```
