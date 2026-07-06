// pidboy drag-to-reposition editor.
//
// The compiler runs entirely in the browser (pidc-wasm). The .pid source is
// the single source of truth: a drag becomes `at: (x,y)` written into the
// source, then a fresh compile and a full SVG swap. The server is only a
// file bridge (GET /source, POST /save).

import init, { compile, move_node } from './pkg/pidc_wasm.js';

const GRID = 80;
const DRAG_THRESHOLD_PX = 5; // screen px before a press counts as a drag
const MIN_ZOOM = 0.2;
const MAX_ZOOM = 4;

const canvas = document.getElementById('canvas');
const diagPanel = document.getElementById('diagnostics');
const saveState = document.getElementById('save-state');
const hoverInfo = document.getElementById('hover-info');
const toast = document.getElementById('toast');

let source = '';
let filename = '';
let nodesById = new Map();
let svgEl = null;
let naturalSize = null; // {w, h} from the svg width/height attributes
let zoom = 1;
let saving = false;

// Source snapshots for undo/redo; `histPtr` points at the current state.
let history = [];
let histPtr = -1;

await init();

const meta = await (await fetch('/meta')).json();
filename = meta.filename;
document.getElementById('filename').textContent = filename;
document.title = `pidboy editor — ${filename}`;

source = await (await fetch('/source')).text();
history = [source];
histPtr = 0;
render(JSON.parse(compile(source)));

document.getElementById('save-btn').addEventListener('click', () => save());
window.addEventListener('keydown', onGlobalKey);
canvas.addEventListener('wheel', onWheel, { passive: false });
setInterval(pollExternalChanges, 2000);

// ---------------------------------------------------------------- rendering

function render(res) {
  const scrollLeft = canvas.scrollLeft;
  const scrollTop = canvas.scrollTop;

  showDiagnostics(res.diagnostics);
  if (!res.ok) {
    showToast('compile failed — see diagnostics');
    return;
  }

  canvas.innerHTML = res.svg;
  svgEl = canvas.querySelector('svg');
  naturalSize = {
    w: parseFloat(svgEl.getAttribute('width')),
    h: parseFloat(svgEl.getAttribute('height')),
  };
  applyZoom();
  canvas.scrollLeft = scrollLeft;
  canvas.scrollTop = scrollTop;

  nodesById = new Map(res.nodes.map((n) => [n.id, n]));

  // Draggable things live in three layers: symbols (equipment, valves,
  // instruments, junctions), frames (module rects), and notes.
  for (const layer of ['symbols', 'frames', 'notes']) {
    const g = svgEl.querySelector(`[id="${layer}"]`);
    if (!g) continue;
    for (const node of res.nodes) {
      if (!node.draggable) continue;
      const el = g.querySelector(`[id="${cssQuote(node.id)}"]`);
      if (!el) continue;
      el.setAttribute('data-draggable', '');
      // Frame rects are fill:none; make their interior grabbable too.
      // Symbols/pipes render later in the document, so they stay on top.
      if (node.kind === 'group') el.style.pointerEvents = 'all';
    }
  }

  svgEl.addEventListener('pointerdown', onPointerDown);
  svgEl.addEventListener('pointerover', onHoverIn);
  svgEl.addEventListener('pointerout', onHoverOut);
}

function cssQuote(id) {
  return id.replace(/"/g, '\\"');
}

function showDiagnostics(diagnostics) {
  diagPanel.innerHTML = '';
  for (const d of diagnostics) {
    const div = document.createElement('div');
    div.className = `diag ${d.severity}`;
    const loc = d.line != null ? `${filename}:${d.line}:${d.col}: ` : '';
    div.textContent = `${loc}${d.severity}: ${d.message}${d.help ? ` (help: ${d.help})` : ''}`;
    diagPanel.appendChild(div);
  }
}

/// Adopt a new source: recompile, swap the drawing, autosave.
function setSource(newSource, { pushHistory = true, autosave = true } = {}) {
  const res = JSON.parse(compile(newSource));
  if (!res.ok) {
    showDiagnostics(res.diagnostics);
    showToast('compile failed — change not applied');
    return false;
  }
  source = newSource;
  if (pushHistory) {
    history.length = histPtr + 1;
    history.push(source);
    histPtr = history.length - 1;
  }
  render(res);
  if (autosave) save();
  return true;
}

// ---------------------------------------------------------- hover highlight

function onHoverIn(e) {
  if (drag) return;
  const pl = e.target.closest('polyline[id]');
  if (!pl) return;
  if (!pl.dataset.origWidth) {
    const w = parseFloat(getComputedStyle(pl).strokeWidth) || 1;
    pl.dataset.origWidth = w;
    pl.style.strokeWidth = w * 2.5;
  }
  hoverInfo.textContent = pl.getAttribute('id');
}

function onHoverOut(e) {
  const pl = e.target.closest('polyline[id]');
  if (!pl) return;
  if (pl.dataset.origWidth) {
    pl.style.strokeWidth = pl.dataset.origWidth;
    delete pl.dataset.origWidth;
  }
  hoverInfo.textContent = '';
}

// ----------------------------------------------------------------- dragging

let drag = null;

function svgPoint(e) {
  return new DOMPoint(e.clientX, e.clientY).matrixTransform(svgEl.getScreenCTM().inverse());
}

function onPointerDown(e) {
  if (e.button !== 0) return;
  const el = e.target.closest('[data-draggable]');
  if (!el) return;
  const node = nodesById.get(el.getAttribute('id'));
  if (!node) return;

  drag = {
    el,
    node,
    startScreen: { x: e.clientX, y: e.clientY },
    startSvg: svgPoint(e),
    origTransform: el.getAttribute('transform') || '',
    active: false, // becomes true past the movement threshold
    gx: Math.round(node.gridX),
    gy: Math.round(node.gridY),
    ghost: null,
  };
  window.addEventListener('pointermove', onPointerMove);
  window.addEventListener('pointerup', onPointerUp);
  e.preventDefault();
}

function onPointerMove(e) {
  if (!drag) return;
  if (!drag.active) {
    const moved = Math.hypot(e.clientX - drag.startScreen.x, e.clientY - drag.startScreen.y);
    if (moved < DRAG_THRESHOLD_PX) return;
    drag.active = true;
    canvas.classList.add('dragging');
  }

  const pt = svgPoint(e);
  const gx = Math.round(drag.node.gridX + (pt.x - drag.startSvg.x) / GRID);
  const gy = Math.round(drag.node.gridY + (pt.y - drag.startSvg.y) / GRID);
  drag.gx = gx;
  drag.gy = gy;

  // Snapped offset from the node's current SVG position. Prepended so the
  // move happens in canvas coordinates even when the symbol is rotated.
  const dx = (gx - drag.node.gridX) * GRID;
  const dy = (gy - drag.node.gridY) * GRID;
  drag.el.setAttribute('transform', `translate(${dx},${dy}) ${drag.origTransform}`);
  updateGhost(dx, dy);
}

function updateGhost(dx, dy) {
  const { node } = drag;
  if (!drag.ghost) {
    const g = document.createElementNS('http://www.w3.org/2000/svg', 'g');
    g.setAttribute('fill', 'none');
    g.setAttribute('stroke', '#0a66c2');
    g.setAttribute('stroke-width', '1');
    g.setAttribute('stroke-dasharray', '4,3');
    g.setAttribute('pointer-events', 'none');
    const rect = document.createElementNS('http://www.w3.org/2000/svg', 'rect');
    rect.setAttribute('width', node.w);
    rect.setAttribute('height', node.h);
    g.appendChild(rect);
    svgEl.appendChild(g);
    drag.ghost = { g, rect };
  }
  drag.ghost.rect.setAttribute('x', node.x + dx - node.w / 2);
  drag.ghost.rect.setAttribute('y', node.y + dy - node.h / 2);
}

function endDrag(revert) {
  if (!drag) return;
  if (revert) drag.el.setAttribute('transform', drag.origTransform);
  if (drag.ghost) drag.ghost.g.remove();
  canvas.classList.remove('dragging');
  window.removeEventListener('pointermove', onPointerMove);
  window.removeEventListener('pointerup', onPointerUp);
  drag = null;
}

function onPointerUp() {
  if (!drag) return;
  if (!drag.active) {
    endDrag(true);
    return;
  }

  const { node, gx, gy } = drag;
  // No-op drags (dropped on the node's own grid cell) leave the source
  // untouched, so a slight wiggle never pins an auto-placed node.
  if (node.pinned && gx === Math.round(node.gridX) && gy === Math.round(node.gridY)) {
    endDrag(true);
    return;
  }

  const mv = JSON.parse(move_node(source, node.id, gx, gy));
  if (!mv.ok) {
    endDrag(true);
    showToast(mv.error);
    return;
  }

  // Where the dragged node should land, in the old coordinate frame.
  const expected = { x: node.x + (gx - node.gridX) * GRID, y: node.y + (gy - node.gridY) * GRID };

  endDrag(false);
  if (!setSource(mv.source)) return;

  // The re-layout can shift the whole drawing (origin re-normalisation);
  // scroll so the dropped node stays put on screen.
  const moved = nodesById.get(node.id);
  if (moved) {
    canvas.scrollLeft += (moved.x - expected.x) * zoom;
    canvas.scrollTop += (moved.y - expected.y) * zoom;
  }
}

// --------------------------------------------------------------- undo / redo

function onGlobalKey(e) {
  if (e.key === 'Escape') {
    endDrag(true);
    return;
  }
  const mod = e.metaKey || e.ctrlKey;
  if (!mod || e.key.toLowerCase() !== 'z') return;
  e.preventDefault();
  if (drag) return;
  if (e.shiftKey) {
    if (histPtr < history.length - 1) {
      histPtr += 1;
      setSource(history[histPtr], { pushHistory: false });
    }
  } else if (histPtr > 0) {
    histPtr -= 1;
    setSource(history[histPtr], { pushHistory: false });
  }
}

// --------------------------------------------------------------------- zoom

function onWheel(e) {
  if (!e.metaKey && !e.ctrlKey) return;
  e.preventDefault();
  if (!svgEl) return;
  const factor = Math.exp(-e.deltaY * 0.0015);
  const newZoom = Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, zoom * factor));
  if (newZoom === zoom) return;

  // Keep the point under the cursor stationary.
  const rect = canvas.getBoundingClientRect();
  const cx = e.clientX - rect.left;
  const cy = e.clientY - rect.top;
  const contentX = (canvas.scrollLeft + cx) / zoom;
  const contentY = (canvas.scrollTop + cy) / zoom;

  zoom = newZoom;
  applyZoom();
  canvas.scrollLeft = contentX * zoom - cx;
  canvas.scrollTop = contentY * zoom - cy;
}

function applyZoom() {
  if (!svgEl || !naturalSize) return;
  svgEl.style.width = `${naturalSize.w * zoom}px`;
  svgEl.style.height = `${naturalSize.h * zoom}px`;
}

// ------------------------------------------------- external change polling

async function pollExternalChanges() {
  if (drag || saving) return;
  try {
    const text = await (await fetch('/source')).text();
    if (drag || saving) return; // re-check: a drag may have started meanwhile
    if (text !== source) {
      showToast('file changed on disk — reloaded');
      setSource(text, { autosave: false });
    }
  } catch {
    // server briefly unreachable; try again next tick
  }
}

// ------------------------------------------------------------------- saving

async function save() {
  saving = true;
  try {
    const resp = await fetch('/save', { method: 'POST', body: source });
    if (!resp.ok) throw new Error(`HTTP ${resp.status}`);
    const t = new Date().toLocaleTimeString();
    saveState.textContent = `saved ${t}`;
    saveState.classList.remove('error');
  } catch (err) {
    saveState.textContent = 'save failed';
    saveState.classList.add('error');
    showToast(`save failed: ${err.message}`);
  } finally {
    saving = false;
  }
}

let toastTimer = null;

function showToast(msg) {
  toast.textContent = msg;
  toast.classList.add('show');
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => toast.classList.remove('show'), 3200);
}

