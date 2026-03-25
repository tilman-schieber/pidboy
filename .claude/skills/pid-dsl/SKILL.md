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
  ports: in, out            # simple port list (optional)
  label: "P-101"            # optional
  orient: east              # north/south/east/west (optional)
```

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

**equipment.type values:** `pump`, `pump_centrifugal`, `pump_positive_displacement`, `heat_exchanger`, `heat_exchanger_shell_tube`, `tank`, `vessel`, `separator`, `reactor_cstr`, `reactor_batch`, `reactor_pfr`, `compressor`, `blower`, `mixer`, `distillation_column`

---

### valve

```
valve CV101:
  type: control_valve       # required
  actuator: pneumatic       # manual / pneumatic / electric (optional)
  fail: closed              # open / closed / last (optional)
  at: (16,8)
  ports: in, out
  label: "CV-101"
```

**valve.type values:** `gate`, `globe`, `ball`, `butterfly`, `plug`, `control_valve`, `check_valve`, `relief_valve`, `safety_valve`

---

### line

```
line L100:
  class: process            # required: process / utility / drain / vent
  from: P101.out            # required — object or object.port
  to: CV101.in              # required
  label: "3in-P-1023"       # optional
```

**line.class values:** `process`, `utility`, `drain`, `vent`

---

### instrument

```
instrument TI101:
  type: temperature_indicator   # required
  attach: E101.top              # attach to equipment port (optional)
  location: field               # field / panel / control_room (optional)
  label: "TI-101"

instrument TIC101:
  type: temperature_controller
  at: (24,3)                    # explicit position if not attached
  location: panel
  label: "TIC-101"
```

**instrument.type values:** `temperature_indicator`, `pressure_indicator`, `flow_indicator`, `level_indicator`, `temperature_transmitter`, `pressure_transmitter`, `flow_transmitter`, `level_transmitter`, `temperature_controller`, `pressure_controller`, `flow_controller`, `level_controller`, `alarm`

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
