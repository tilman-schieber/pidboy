# pidboy — project conventions

`pidc` compiles a semantic P&ID DSL to SVG. Language reference: `dsl.md`.
Generation-oriented DSL reference: `.claude/skills/pid-dsl/SKILL.md`
(keep it in sync when the language changes). `spec.md` is the historical
v0.1 design document — do not treat it as current behavior.

## Standards decisions (do not regress)

Symbols and line styles are deliberately aligned with **ISO 10628**
(flow-diagram symbols/piping) and **ISA-5.1** (instrument bubbles and
signal lines):

- Pneumatic signals: **solid line with pairs of slash marks** (`—//—`),
  drawn as explicit paths (`render_pneumatic_marks` in
  `src/render/svg.rs`). Never a dash pattern — that was tried and is
  indistinguishable from drain/vent dashes.
- Electrical signals: dashed `6,3` per ISA. Process piping is the ONLY
  solid line style — a solid thin electric was tried and users could
  not tell signal wires from pipes.
- Line cadences must stay mutually distinct: process solid/2, utility
  long dash `14,5`, electric dash `6,3`/1.5, drain dash-dot
  `9,3,1.5,3`, vent dots `1.5,4`, pneumatic solid+slashes.
- Instrument bubble style is keyed to `location` (field / panel /
  control_room / shared), not instrument type, per ISA-5.1.
- Relief valves render as the angle pattern (inlet below, outlet to the
  side, spring on top); declare `ports: in: south, out: east`.
- Flow arrowheads on all piping and signals, none on instrument leaders.

## SVG viewer compatibility (hard constraints)

The user edits/views output in **Adobe Illustrator**. Its SVG importer
drops or mangles several constructs, so the renderer must not emit:

- `<marker>` / `marker-end` — Illustrator drops the entire polyline.
  Arrowheads and slash marks are explicit `<path>` elements.
- `<symbol>` — use `<g>` inside `<defs>`, referenced with both `href`
  and `xlink:href`.
- CSS-only styling — every element carries explicit presentation
  attributes alongside its class.

Verify renderer changes in Chrome **and** rsvg (`rsvg-convert`) before
shipping; they catch different problems.

## Workflow

- `cargo test` must pass; unit tests live next to the code, integration
  tests in `tests/`.
- After any renderer/layout/symbol change, regenerate all examples and
  eyeball the renders:

  ```
  for f in examples/*.pid; do
    ./target/debug/pidc compile "$f" --pretty --legend -o "${f%.pid}.svg"
  done
  ```

  Examples are committed with `--pretty --legend`;
  `batch_polymerisation` additionally uses `--table --title ...
  --footer ...` (see its header comment for the exact command).
- The browser editor (`pidc serve <file.pid>`) needs its WASM bundle
  built once via `scripts/build-editor.sh` (requires `wasm-pack` and the
  `wasm32-unknown-unknown` target), then `cargo build` to embed it.
  Without the bundle, `cargo build`/`cargo test` still work (build.rs
  embeds empty placeholders) and `serve` explains what to run.
  `cargo build --no-default-features` builds the library only (no bin).
- The three examples are regression fixtures as much as documentation:
  `boiler_feed_water` (explicit coordinates), `reactor_cooling` (fully
  automatic layout), `three_phase_separator` (replica of a reference
  drawing; exercises multi-nozzle ports, bypasses, stubs, vertical
  valves, corner placement).

## Architecture notes

- Pipeline: lexer → parser → normalize → validate → layout → route →
  render. Keep the renderer ignorant of source syntax and the parser
  ignorant of SVG. Library entry points live in `src/compile.rs`
  (`compile_to_parts`, `analyze`); the pipeline core must stay pure (no
  I/O, printing, threads, or time) so it keeps compiling to WASM.
- Interactive editor: `wasm/` (crate `pidc-wasm`) exposes
  `compile`/`move_node` as JSON-over-wasm-bindgen; `editor/web/` is a
  dependency-free ES-module frontend; `src/serve.rs` is a dumb file
  bridge (all compilation happens client-side). A drag writes
  `at: (x,y)` into the `.pid` source via spans (`src/edit.rs`) and
  recompiles — the `.pid` file stays the single source of truth.
  Framed-module members are not individually draggable (layout ignores
  their `at:`).
- `src/symbols/mod.rs` is backend-agnostic geometry; no SVG structural
  constructs there (path `d` strings are the accepted exception).
- Layout is connectivity-driven: placement anchors on the specific port
  a line uses (`port_offset` is shared between layout and routing so
  they cannot drift). Instrument placement avoids "used-port corridors"
  — strips outside ports that lines connect to.
- The router picks from a fixed candidate set (L, Z, local hops, escape
  routes) scored by collisions/bends/length/reversals/overlap. It has no
  full pathfinder; if a route looks impossible, add a candidate shape
  rather than special-casing coordinates.
