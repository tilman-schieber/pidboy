// pidboy drag-to-reposition editor.
//
// The compiler runs entirely in the browser (pidc-wasm). The .pid source is
// the single source of truth: a drag becomes `at: (x,y)` written into the
// source, then a fresh compile and a full SVG swap. The server is only a
// file bridge (GET /source, POST /save).

import init, { compile, move_node } from './pkg/pidc_wasm.js';

const GRID = 80;
const DRAG_THRESHOLD_PX = 5; // screen px before a press counts as a drag

const canvas = document.getElementById('canvas');
const diagPanel = document.getElementById('diagnostics');
const saveState = document.getElementById('save-state');
const toast = document.getElementById('toast');

let source = '';
let filename = '';
let nodesById = new Map();
let svgEl = null;

await init();

const meta = await (await fetch('/meta')).json();
filename = meta.filename;
document.getElementById('filename').textContent = filename;
document.title = `pidboy editor — ${filename}`;

source = await (await fetch('/source')).text();
render(JSON.parse(compile(source)));

document.getElementById('save-btn').addEventListener('click', () => save());

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
  canvas.scrollLeft = scrollLeft;
  canvas.scrollTop = scrollTop;

  svgEl = canvas.querySelector('svg');
  nodesById = new Map(res.nodes.map((n) => [n.id, n]));

  const symbols = svgEl.querySelector('#symbols');
  if (symbols) {
    for (const node of res.nodes) {
      if (!node.draggable) continue;
      const el = symbols.querySelector(`[id="${cssQuote(node.id)}"]`);
      if (el) el.setAttribute('data-draggable', '');
    }
  }

  svgEl.addEventListener('pointerdown', onPointerDown);
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
  window.addEventListener('keydown', onDragKey);
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
  window.removeEventListener('keydown', onDragKey);
  drag = null;
}

function onDragKey(e) {
  if (e.key === 'Escape') endDrag(true);
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

  const res = JSON.parse(compile(mv.source));
  if (!res.ok) {
    endDrag(true);
    showDiagnostics(res.diagnostics);
    showToast('move rejected — recompile failed, reverted');
    return;
  }

  endDrag(false);
  source = mv.source;
  render(res);
  save();
}

// ------------------------------------------------------------------- saving

async function save() {
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
  }
}

let toastTimer = null;

function showToast(msg) {
  toast.textContent = msg;
  toast.classList.add('show');
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => toast.classList.remove('show'), 3200);
}
