---
name: pid-dsl
description: Generate, edit, or explain P&ID DSL source files for the pidc compiler. Use when the user describes a process, control loop, piping system, or asks to write, modify, or review a .pid file.
argument-hint: [describe the diagram or paste existing DSL]
---

You are writing P&ID DSL for the `pidc` compiler. Your output must be valid `.pid` source that compiles without errors.

## DSL rules

### Two declaration forms — never mix them

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

A declaration must be one or the other. Never `equipment P101 type=pump:` followed by a block body.

### Indentation

- 2 spaces per level only — tabs are a hard error
- Maximum 2 levels deep (property block inside a declaration)

### Comments

```
# full-line comment
equipment P101:
  type: pump  # inline comment
```

### Value types

| Form | Meaning |
|------|---------|
| `pump` | bare scalar / enum value |
| `"P-101"` | quoted string (use for labels) |
| `(10,8)` | tuple — grid coordinates |
| `in,out` | comma-separated list |
| `P101.out` | reference: object + port |
| `TI101` | reference: object only |

### IDs

- Must be unique across the entire document
- Use realistic engineering IDs: `P101`, `CV101`, `E101`, `TI-101` etc.

---

## Declaration reference

### equipment

```
equipment P101:
  type: pump                # required
  at: (10,8)                # grid position (optional)
  size: (8,3)               # symbol size in grid units (optional; use for large multi-nozzle vessels)
  ports: in, out            # simple port list (optional)
  label: "P-101"            # optional
  orient: east              # north/south/east/west (optional)
```

**Large vessels:** give storage tanks/reactors with many nozzles a `size:` so the ports have room (e.g. `size: (8,3)` = 640x240 px). Vessels/tanks draw parametrically at that exact size and, when the label fits, carry it inside the shell. Other types scale uniformly.

Structured ports with explicit sides:
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

**equipment.type values:** `pump`, `pump_centrifugal`, `pump_positive_displacement`, `heat_exchanger`, `heat_exchanger_shell_tube`, `tank`, `vessel`, `separator`, `separator_3phase`, `reactor_cstr`, `reactor_batch`, `reactor_pfr`, `compressor`, `blower`, `mixer`, `distillation_column`, `connector`, `heat_pad`

**Equipment-on-equipment attachment:** equipment can carry `attach:` to mount flush against a host — heat pads, jackets. Declare a dedicated port on the host for it so pipe nozzles stay clear, and target signals at the attached item:

```
equipment HP1:
  type: heat_pad
  attach: V100.pad      # pad: south port on the vessel
  label: "HEAT PAD"
signal S1 type=electrical from=TIC102 to=HP1
```

**Multiple ports per side:** ports sharing a side are spread evenly along it in declaration order (top-to-bottom for east/west sides, left-to-right for north/south). For `separator_3phase` (large drum with weir, demister pad and vortex breakers drawn in), declare ports in this order so nozzles land on the right internals: `inlet: north`, `psv: north`, `vent: north`, `gas: north` (gas over the demister), `water: south`, `oil: south` (water upstream of the weir, oil downstream), plus optional `lt_w: west` / `lt_e: east` for side-mounted level transmitters (`attach: SEP.lt_w`).

**Attach direction:** `attach: X.port` places the bubble outward on that port's side (west port → bubble left of the vessel with a horizontal leader); plain `attach: X` places it above.

**Off-page connectors:** use `type: connector` (a pentagon flag) for streams that enter or leave the sheet — utility headers (CWS/CWR), flare, battery limits. Prefer once-through utility runs via connectors over drawing closed recycle loops; loops render as tangled rectangles. Example:

```
equipment CWS:
  type: connector
  ports:
    out: west
  label: "CWS"
```

---

### valve

```
valve CV101:
  type: control_valve       # required
  actuator: pneumatic       # manual / pneumatic / electric / diaphragm (optional; diaphragm draws a dome actuator)
  fail: closed              # open / closed / last (optional; renders FC/FO/FL tag)
  state: nc                 # nc / no (optional; renders N.C./N.O. tag, e.g. normally-closed block valves)
  at: (16,8)
  ports: in, out
  label: "CV-101"
```

**valve.type values:** `gate`, `globe`, `ball`, `butterfly`, `plug`, `control_valve`, `check_valve`, `relief_valve`, `safety_valve`

`globe` renders as a bowtie with a filled plug dot (use it for bypass and throttling valves); `control_valve` with `actuator: diaphragm` renders a dome actuator.

**Bypass loops:** junctions accept directional taps (`J1.west`, `J1.south`, …) even though they declare no ports. Standard control-valve station with bypass:

```
junction J1
junction J2
line: ... upstream valve → J1.east
line: J1.west → CV.in       # main run through the control valve
line: CV.out → J2.east
line: J2.west → ... downstream valve
line: J1.south → BPV.in     # globe-valve bypass below
line: BPV.out → J2.south
```

---

### line

```
line L100:
  class: process            # required: process / utility / drain / vent
  from: P101.out            # required — object or object.port
  to: CV101.in              # optional — omit for an open-ended stub
  label: "3in-P-1023"       # optional
```

**line.class values:** `process`, `utility`, `drain`, `vent`

**Open-ended stubs:** a line with no `to` draws a short open run outward from `from` — use for drains, vents and sample points. Direction follows the `from` port side (else vents point up, drains down). Typical drain off a pipe run:

```
junction JD1                # tee on the main run
valve DV1:
  type: gate
  ports:
    in: north
    out: south              # all-N/S ports draw the valve rotated vertical
line: JD1.south → DV1.in    (class: drain)
line: from: DV1.out, label: "OD"   (class: drain, no to:)
```

---

### instrument

```
instrument TI101:
  type: temperature_indicator   # required
  attach: E101.top              # attach to equipment port (optional)
  location: field               # field / panel / control_room / shared (optional; shared = circle in square, DCS shared display)
  label: "TI-101"

instrument TIC101:
  type: temperature_controller
  at: (24,3)                    # explicit position if not attached
  location: panel
  label: "TIC-101"
```

**instrument.type values:** `temperature_indicator`, `pressure_indicator`, `flow_indicator`, `level_indicator`, `temperature_transmitter`, `pressure_transmitter`, `flow_transmitter`, `level_transmitter`, `temperature_controller`, `pressure_controller`, `flow_controller`, `level_controller`, `alarm`, `relay`, `transducer`, `level_gauge`

`relay` is for PY/TY-style computing relays in split-range schemes: controller → relay (electrical) → valve (pneumatic), one relay per valve.

---

### signal

```
signal S100:
  type: electrical          # required: electrical / pneumatic / hydraulic / digital
  from: TI101               # required
  to: TIC101                # required
  label: "4-20mA"           # optional
```

---

### junction

```
junction J1 at=(18,8)
```

Use junctions for explicit tee/branch points where lines split.

---

### note

```
note N1:
  at: (5,2)
  text: "Normally closed during startup"
```

---

### group

```
group cooling_loop:
  members: P101, CV101, E101, TI101, TIC101
  label: "Cooling loop"
```

---

### area

```
area process_side:
  label: "Process Area"
  bounds: (0,0), (40,20)
```

---

## Grid layout hints

- Coordinates are integer grid units; the compiler scales them to pixels (default 80 px/unit)
- Place objects left-to-right or top-to-bottom as the process flows
- Leave 4–6 grid units between adjacent equipment
- Instruments can sit above (lower y) or below the process line
- Objects without `at` are placed automatically — omit `at` if layout doesn't matter

**Typical layout pattern:**

```
# Process flows left to right at y=8
# Instruments at y=3 (above) or y=13 (below)

equipment FEED:   at: (4,8)
valve V1:         at: (10,8)
equipment R1:     at: (18,8)
instrument TI1:   at: (18,3)
```

---

## Connectivity rules

- `from` and `to` on `line` connect piping between equipment/valves
- `from` and `to` on `signal` connect instruments and controllers
- `attach` on `instrument` places it physically near equipment (use a named port)
- If you declare ports explicitly, `from`/`to` references must use a declared port name

---

## Output instructions

$ARGUMENTS

Generate a complete, valid `.pid` file. Follow these rules:
1. Start with a `# comment` describing the diagram
2. Declare equipment and valves before lines that reference them
3. Declare instruments after the equipment they attach to
4. Declare signals after the instruments they connect
5. Use realistic engineering tag numbers (P-101 style labels, P101 style IDs)
6. Every `from`/`to`/`attach` target must be declared somewhere in the file
7. If ports are listed on equipment, only reference declared port names
8. Output only the `.pid` source — no explanation unless asked
