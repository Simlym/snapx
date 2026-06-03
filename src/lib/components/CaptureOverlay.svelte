<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';
  import Magnifier from './Magnifier.svelte';
  import SizeIndicator from './SizeIndicator.svelte';
  import Toolbar from './Toolbar.svelte';
  import type { ToolType } from './Toolbar.svelte';

  // ── Annotation types ───────────────────────────────────────────────
  // Shared line-style traits live on the relevant shapes so each annotation
  // remembers how it was drawn (dashed vs solid, rounded corners, arrowheads).
  type LineStyle = 'solid' | 'dashed';
  type ArrowStyle = 'end' | 'both' | 'none';
  interface AnnBase       { id: string; color: string; sw: number; }
  interface BoxAnn        extends AnnBase { type: 'rect' | 'ellipse'; x: number; y: number; w: number; h: number; fill: boolean; dash: LineStyle; round: boolean; }
  interface ArrowAnn      extends AnnBase { type: 'arrow'; x1: number; y1: number; x2: number; y2: number; dash: LineStyle; head: ArrowStyle; }
  interface LineAnn       extends AnnBase { type: 'line';  x1: number; y1: number; x2: number; y2: number; dash: LineStyle; }
  interface PenAnn        extends AnnBase { type: 'pen'; pts: [number, number][]; }
  interface TextAnn       extends AnnBase { type: 'text'; x: number; y: number; text: string; bg: boolean; }
  interface MosaicAnn     extends AnnBase { type: 'mosaic'; x: number; y: number; w: number; h: number; }
  interface HighlightAnn  extends AnnBase { type: 'highlight'; x: number; y: number; w: number; h: number; }
  interface CounterAnn    extends AnnBase { type: 'counter'; x: number; y: number; n: number; }
  type Annotation = BoxAnn | ArrowAnn | LineAnn | PenAnn | TextAnn | MosaicAnn | HighlightAnn | CounterAnn;
  type HandleId = 'tl' | 'tc' | 'tr' | 'ml' | 'mr' | 'bl' | 'bc' | 'br' | 'p1' | 'p2';

  // ── Props ──────────────────────────────────────────────────────────
  interface Props {
    /** Null during the selecting phase — the overlay is transparent over the
     *  live desktop and only grabs the screenshot once a region is committed. */
    screenshotData: string | null;
    screenshotWidth: number;
    screenshotHeight: number;
    oncopy?: (imageData: string) => void;
    onsave?: (imageData: string) => void;
    onquicksave?: (imageData: string) => void;
    onpin?: (imageData: string, pxW: number, pxH: number) => void;
    oncancel?: () => void;
    /** Full-screen screenshot used by the magnifier cursor during selection. */
    magnifierSrc?: string;
    magnifierW?: number;
    magnifierH?: number;
    /** Long-screenshot mode: emits the chosen region in physical pixels. */
    scrollMode?: boolean;
    onscroll?: (region: { x: number; y: number; w: number; h: number }) => void;
    /** Grab ONLY the given region (physical px) on demand, called when the user
     *  commits. Resolves with the cropped screenshot, or null on failure. */
    captureForSelection?: (
      region: { x: number; y: number; w: number; h: number }
    ) => Promise<{ data: string; width: number; height: number } | null>;
  }

  let {
    screenshotData = $bindable(null),
    screenshotWidth = $bindable(0),
    screenshotHeight = $bindable(0),
    oncopy, onsave, onquicksave, onpin, oncancel,
    magnifierSrc = '', magnifierW = 0, magnifierH = 0,
    scrollMode = false, onscroll, captureForSelection,
  }: Props = $props();
  let bgSrc = $derived(screenshotData ? `data:image/png;base64,${screenshotData}` : '');

  // ── Phase ──────────────────────────────────────────────────────────
  let phase: 'selecting' | 'annotating' = $state('selecting');

  // ── Region selection ───────────────────────────────────────────────
  let ox = $state(0), oy = $state(0);
  let cx = $state(0), cy = $state(0);
  let selecting = $state(false);
  let sel = $derived.by(() => ({
    x: Math.min(ox, cx), y: Math.min(oy, cy),
    w: Math.abs(cx - ox), h: Math.abs(cy - oy),
  }));
  let hasSel = $derived(sel.w > 4 && sel.h > 4);

  // ── Mouse tracking (magnifier) ────────────────────────────────────
  let mx = $state(0), my = $state(0);

  // Whether the cursor is over (or just outside) the committed selection. The
  // resize grips key off this so a finished region reads as a clean box — not a
  // "selected" object covered in handles — until you actually move to adjust it.
  const GRIP_REVEAL = 14;
  let nearSel = $derived(
    hasSel &&
    mx >= sel.x - GRIP_REVEAL && mx <= sel.x + sel.w + GRIP_REVEAL &&
    my >= sel.y - GRIP_REVEAL && my <= sel.y + sel.h + GRIP_REVEAL
  );

  // ── Eyedropper / magnifier colour sampling ────────────────────────
  // Samples pixel colour from the full-screen magnifier screenshot (available
  // from capture-open, before any region is committed) so the colour picker
  // works the moment capture mode opens — Snipaste-style.
  let pickedColor = $state('');
  let colorCopied = $state(false);
  let colorFormat: 'hex' | 'rgb' | 'hsl' = $state('rgb');
  let magnifierCanvas: HTMLCanvasElement | null = null;
  let magnifierCtx: CanvasRenderingContext2D | null = null;
  let magnifierImgEl: HTMLImageElement | null = null;

  // Reset the offscreen canvas whenever the source image changes.
  $effect(() => {
    magnifierSrc; // track
    magnifierCanvas = null;
    magnifierCtx    = null;
    magnifierImgEl  = null;
  });

  function ensureMagnifierCanvas() {
    if (!magnifierSrc) return;
    if (magnifierCtx || magnifierImgEl) return;
    magnifierImgEl = new Image();
    magnifierImgEl.onload = () => {
      const w = magnifierW || magnifierImgEl!.naturalWidth;
      const h = magnifierH || magnifierImgEl!.naturalHeight;
      const c = document.createElement('canvas');
      c.width = w; c.height = h;
      const ctx = c.getContext('2d', { willReadFrequently: true });
      if (ctx) {
        ctx.drawImage(magnifierImgEl!, 0, 0, w, h);
        magnifierCanvas = c;
        magnifierCtx    = ctx;
      }
    };
    magnifierImgEl.src = magnifierSrc;
  }

  function sampleColorAt(clientX: number, clientY: number) {
    ensureMagnifierCanvas();
    if (!magnifierCtx || !magnifierW || !magnifierH) return;
    const scaleX = magnifierW / window.innerWidth;
    const scaleY = magnifierH / window.innerHeight;
    const px = Math.max(0, Math.min(magnifierW - 1, Math.round(clientX * scaleX)));
    const py = Math.max(0, Math.min(magnifierH - 1, Math.round(clientY * scaleY)));
    try {
      const d = magnifierCtx.getImageData(px, py, 1, 1).data;
      pickedColor = '#' + [d[0], d[1], d[2]].map((v) => v.toString(16).padStart(2, '0')).join('');
    } catch (_) { /* not ready */ }
  }

  function formattedColor(hex: string, fmt: 'hex' | 'rgb' | 'hsl'): string {
    const m = /^#?([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})$/i.exec(hex);
    if (!m) return hex;
    const r = parseInt(m[1], 16), g = parseInt(m[2], 16), b = parseInt(m[3], 16);
    if (fmt === 'rgb') return `rgb(${r}, ${g}, ${b})`;
    if (fmt === 'hsl') {
      const rn = r / 255, gn = g / 255, bn = b / 255;
      const max = Math.max(rn, gn, bn), min = Math.min(rn, gn, bn);
      const l = (max + min) / 2;
      let h = 0, s = 0;
      if (max !== min) {
        const d = max - min;
        s = l > 0.5 ? d / (2 - max - min) : d / (max + min);
        if (max === rn) h = ((gn - bn) / d + (gn < bn ? 6 : 0)) / 6;
        else if (max === gn) h = ((bn - rn) / d + 2) / 6;
        else h = ((rn - gn) / d + 4) / 6;
      }
      return `hsl(${Math.round(h * 360)}, ${Math.round(s * 100)}%, ${Math.round(l * 100)}%)`;
    }
    return hex.toUpperCase();
  }

  async function copyPickedColor() {
    if (!pickedColor) return;
    try {
      await navigator.clipboard.writeText(formattedColor(pickedColor, colorFormat));
      colorCopied = true;
      setTimeout(() => (colorCopied = false), 900);
    } catch (_) { /* clipboard unavailable */ }
  }

  // ── Window edge detection (snap-to-window) ────────────────────────
  // Window bounds come from Rust in monitor-local *physical* px; we convert to
  // viewport (CSS) px to overlay them on the displayed screenshot. Before any
  // drag, hovering highlights the smallest window under the cursor; clicking it
  // (without dragging) snaps the selection to that window — Snipaste-style.
  interface WinRect { x: number; y: number; w: number; h: number; }
  let windowRects: WinRect[] = [];
  let hoveredWindow = $state<WinRect | null>(null);

  onMount(async () => {
    try {
      const wins = await invoke<
        { x: number; y: number; width: number; height: number }[]
      >('list_windows', { monitorIndex: 0 });
      // Window bounds come back in monitor-local *physical* px. We have no
      // screenshot yet (capture is deferred), so convert to CSS px via the
      // device pixel ratio — which equals this monitor's scale factor.
      const dpr = window.devicePixelRatio || 1;
      windowRects = wins.map((w) => ({
        x: w.x / dpr, y: w.y / dpr, w: w.width / dpr, h: w.height / dpr,
      }));
    } catch (e) {
      // Window enumeration is best-effort; selection still works without it.
      console.warn('list_windows unavailable:', e);
    }
  });

  // Smallest window whose bounds contain the point — the most specific target.
  function windowAt(px: number, py: number): WinRect | null {
    let best: WinRect | null = null;
    for (const r of windowRects) {
      if (px >= r.x && px <= r.x + r.w && py >= r.y && py <= r.y + r.h) {
        if (!best || r.w * r.h < best.w * best.h) best = r;
      }
    }
    return best;
  }

  // ── Annotation state ───────────────────────────────────────────────
  let activeTool: ToolType = $state('rect');
  let activeColor = $state('#ef4444');
  let activeStroke = $state(3);
  let fillMode = $state(false);
  let textBg = $state(false);
  // Line-style traits, applied to newly drawn shapes (see annDown).
  let lineStyle: LineStyle = $state('solid');
  let roundCorners = $state(false);
  let arrowStyle: ArrowStyle = $state('end');
  // Tools whose stroke width can be tuned with the mouse wheel.
  const HAS_STROKE = new Set<ToolType>(['rect','ellipse','arrow','line','text','pen','counter']);
  let counterNum = $state(1);
  let annotations: Annotation[] = $state([]);
  let undoStack: Annotation[][] = $state([]);
  let redoStack: Annotation[][] = $state([]);
  let currentAnn: Annotation | null = $state(null);
  let annDragging = $state(false);
  let annOx = 0, annOy = 0;

  // ── Text input ─────────────────────────────────────────────────────
  let textVisible = $state(false);
  let textPx = $state(0), textPy = $state(0);
  let textVal = $state('');
  // eslint-disable-next-line prefer-const
  let textInputEl: HTMLInputElement = $state(null as unknown as HTMLInputElement);

  // ── Select tool state ──────────────────────────────────────────────
  let selectedId: string | null = $state(null);
  let selMoving = $state(false);
  let selResizing: HandleId | null = $state(null);
  let selDragStartMx = 0, selDragStartMy = 0;
  let selDragOrigAnn: Annotation | null = null;
  let hoveredId: string | null = $state(null);
  let hoveredHandle: HandleId | null = $state(null);
  let selectedAnn = $derived(annotations.find(a => a.id === selectedId) ?? null);

  // ── Geometry helpers ───────────────────────────────────────────────
  function distToSeg(px: number, py: number, ax: number, ay: number, bx: number, by: number): number {
    const dx = bx - ax, dy = by - ay;
    const lenSq = dx * dx + dy * dy;
    if (lenSq === 0) return Math.hypot(px - ax, py - ay);
    const t = Math.max(0, Math.min(1, ((px - ax) * dx + (py - ay) * dy) / lenSq));
    return Math.hypot(px - ax - t * dx, py - ay - t * dy);
  }

  function hitTest(ann: Annotation, x: number, y: number): boolean {
    const M = 6;
    if (ann.type === 'rect' || ann.type === 'ellipse' || ann.type === 'mosaic' || ann.type === 'highlight') {
      return x >= ann.x - M && x <= ann.x + ann.w + M && y >= ann.y - M && y <= ann.y + ann.h + M;
    }
    if (ann.type === 'arrow' || ann.type === 'line') {
      return distToSeg(x, y, ann.x1, ann.y1, ann.x2, ann.y2) < M + ann.sw;
    }
    if (ann.type === 'pen') {
      for (let i = 0; i < ann.pts.length - 1; i++) {
        if (distToSeg(x, y, ann.pts[i][0], ann.pts[i][1], ann.pts[i + 1][0], ann.pts[i + 1][1]) < M + ann.sw)
          return true;
      }
      return false;
    }
    if (ann.type === 'text') {
      const fs = 14 + ann.sw * 2;
      return x >= ann.x - M && x <= ann.x + ann.text.length * fs * 0.6 + M &&
             y >= ann.y - fs - M && y <= ann.y + M;
    }
    if (ann.type === 'counter') {
      return Math.hypot(x - ann.x, y - ann.y) < 12 + ann.sw * 2 + M;
    }
    return false;
  }

  function getHandles(ann: Annotation): { id: HandleId; x: number; y: number }[] {
    if (ann.type === 'rect' || ann.type === 'ellipse' || ann.type === 'mosaic' || ann.type === 'highlight') {
      const { x, y, w, h } = ann;
      return [
        { id: 'tl', x,         y         }, { id: 'tc', x: x + w / 2, y         }, { id: 'tr', x: x + w, y         },
        { id: 'ml', x,         y: y+h/2  },                                          { id: 'mr', x: x + w, y: y + h/2 },
        { id: 'bl', x,         y: y + h  }, { id: 'bc', x: x + w / 2, y: y + h  }, { id: 'br', x: x + w, y: y + h   },
      ];
    }
    if (ann.type === 'arrow' || ann.type === 'line') {
      return [{ id: 'p1', x: ann.x1, y: ann.y1 }, { id: 'p2', x: ann.x2, y: ann.y2 }];
    }
    return [];
  }

  function applyMove(ann: Annotation, dx: number, dy: number): Annotation {
    if (ann.type === 'rect' || ann.type === 'ellipse' || ann.type === 'mosaic' || ann.type === 'highlight') {
      return { ...ann, x: ann.x + dx, y: ann.y + dy };
    }
    if (ann.type === 'arrow' || ann.type === 'line') {
      return { ...ann, x1: ann.x1 + dx, y1: ann.y1 + dy, x2: ann.x2 + dx, y2: ann.y2 + dy };
    }
    if (ann.type === 'pen') {
      return { ...ann, pts: ann.pts.map(([px, py]) => [px + dx, py + dy] as [number, number]) };
    }
    if (ann.type === 'text' || ann.type === 'counter') {
      return { ...ann, x: ann.x + dx, y: ann.y + dy };
    }
    return ann;
  }

  function applyResize(origAnn: Annotation, hid: HandleId, curMx: number, curMy: number): Annotation {
    if (origAnn.type === 'rect' || origAnn.type === 'ellipse' || origAnn.type === 'mosaic' || origAnn.type === 'highlight') {
      let { x, y, w, h } = origAnn;
      let right = x + w, bottom = y + h;
      if (hid === 'tl' || hid === 'ml' || hid === 'bl') x = curMx;
      if (hid === 'tl' || hid === 'tc' || hid === 'tr') y = curMy;
      if (hid === 'tr' || hid === 'mr' || hid === 'br') right = curMx;
      if (hid === 'bl' || hid === 'bc' || hid === 'br') bottom = curMy;
      const nw = right - x, nh = bottom - y;
      if (nw < 5 || nh < 5) return origAnn;
      return { ...origAnn, x, y, w: nw, h: nh };
    }
    if (origAnn.type === 'arrow' || origAnn.type === 'line') {
      if (hid === 'p1') return { ...origAnn, x1: curMx, y1: curMy };
      if (hid === 'p2') return { ...origAnn, x2: curMx, y2: curMy };
    }
    return origAnn;
  }

  // ── Apply a style change to the selected annotation (live, WYSIWYG) ──
  // When the select tool has something selected, style edits (stroke, colour,
  // dash, round, fill, arrow head, text bg) are written straight onto that
  // annotation so the user sees the result immediately. Each call is one undo
  // step. Returns true if a selected annotation was patched.
  function patchSelected(patch: Partial<Annotation>): boolean {
    if (!selectedId) return false;
    const cur = annotations.find(a => a.id === selectedId);
    if (!cur) return false;
    pushUndo();
    annotations = annotations.map(a =>
      a.id === selectedId ? ({ ...a, ...patch } as Annotation) : a
    );
    return true;
  }

  // Switch a selected annotation between sibling shapes that share geometry:
  // rect ⇄ ellipse (box-based) and arrow ⇄ line (segment-based). Carries over
  // position/size, colour, stroke and any compatible traits.
  function switchSelectedShape(to: 'rect' | 'ellipse' | 'arrow' | 'line') {
    if (!selectedId) return;
    const cur = annotations.find(a => a.id === selectedId);
    if (!cur) return;
    let next: Annotation | null = null;
    if ((cur.type === 'rect' || cur.type === 'ellipse') && (to === 'rect' || to === 'ellipse')) {
      next = { ...cur, type: to };
    } else if ((cur.type === 'arrow' || cur.type === 'line') && (to === 'arrow' || to === 'line')) {
      if (to === 'arrow') {
        // line → arrow: add a head (default 'end').
        next = { ...(cur as LineAnn), type: 'arrow', head: arrowStyle } as ArrowAnn;
      } else {
        // arrow → line: drop the head field.
        const { head: _drop, ...rest } = cur as ArrowAnn;
        next = { ...rest, type: 'line' } as LineAnn;
      }
    }
    if (!next) return;
    pushUndo();
    annotations = annotations.map(a => (a.id === selectedId ? next! : a));
    // The panel reflects the SELECTED annotation's type (styleType), so it
    // updates automatically — no need to touch activeTool (which would also
    // risk clearing the selection via onToolChange downstream).
  }

  // ── Mouse wheel: adjust brush/stroke size while annotating ─────────
  // Scroll up = thicker, down = thinner. Targeting (in priority order):
  //   1. the annotation directly under the cursor (hover) — WYSIWYG resize,
  //   2. the currently selected annotation,
  //   3. otherwise the active tool's size for the *next* shape.
  // Live-resizing an existing annotation mutates its `sw` directly and folds
  // a continuous scroll gesture into a single undo step.
  let wheelUndoTimer: ReturnType<typeof setTimeout> | null = null;
  function resizeAnnSw(id: string, delta: number) {
    const cur = annotations.find(a => a.id === id);
    if (!cur) return;
    const nw = Math.max(1, Math.min(20, cur.sw + delta));
    if (nw === cur.sw) return;
    // Push one undo entry at the start of a scroll burst, then coalesce the
    // rest until the user pauses (300ms) — so a flick of the wheel is one undo.
    if (!wheelUndoTimer) pushUndo();
    if (wheelUndoTimer) clearTimeout(wheelUndoTimer);
    wheelUndoTimer = setTimeout(() => { wheelUndoTimer = null; }, 300);
    annotations = annotations.map(a =>
      a.id === id ? ({ ...a, sw: nw } as Annotation) : a
    );
    activeStroke = nw;
  }
  function onWheel(e: WheelEvent) {
    if (phase !== 'annotating') return;
    const delta = e.deltaY < 0 ? 1 : -1;

    // 1) Annotation under the cursor wins (no need to pick the select tool).
    const hovered = [...annotations].reverse().find(a => hitTest(a, e.clientX, e.clientY));
    if (hovered && hovered.type !== 'mosaic' && hovered.type !== 'highlight') {
      e.preventDefault();
      resizeAnnSw(hovered.id, delta);
      return;
    }
    // 2) Fall back to the selected annotation.
    if (selectedId) {
      const sel = annotations.find(a => a.id === selectedId);
      if (sel && sel.type !== 'mosaic' && sel.type !== 'highlight') {
        e.preventDefault();
        resizeAnnSw(selectedId, delta);
        return;
      }
    }
    // 3) Otherwise set the size for the next shape.
    if (!HAS_STROKE.has(activeTool)) return;
    e.preventDefault();
    activeStroke = Math.max(1, Math.min(20, activeStroke + delta));
  }

  // ── Keyboard ───────────────────────────────────────────────────────
  function onKeyDown(e: KeyboardEvent) {
    if (textVisible && e.key !== 'Escape') return;

    if (e.key === 'Escape') {
      if (ocrEditVisible) { ocrEditVisible = false; return; }
      if (ocrActive) { ocrActive = false; return; }
      if (textVisible) { textVisible = false; return; }
      if (phase === 'annotating') { selectedId = null; backToSelecting(); return; }
      oncancel?.();
      return;
    }

    if (e.key === 'Enter') {
      if (scrollMode && phase === 'selecting' && hasSel) { confirmScrollRegion(); return; }
      if (phase === 'selecting' && hasSel) { enterAnnotating(); return; }
      if (phase === 'annotating') { doExport('copy'); return; }
    }

    // Eyedropper: cycle colour format (Shift) or copy colour value (C).
    if (phase === 'selecting' && e.key === 'Shift' && !e.ctrlKey && !e.metaKey) {
      const fmts: ('hex' | 'rgb' | 'hsl')[] = ['hex', 'rgb', 'hsl'];
      colorFormat = fmts[(fmts.indexOf(colorFormat) + 1) % fmts.length];
      return;
    }
    if (phase === 'selecting' && e.key.toLowerCase() === 'c' && !e.ctrlKey && !e.metaKey) {
      copyPickedColor();
      return;
    }

    if (phase === 'annotating') {
      if ((e.ctrlKey || e.metaKey) && !e.shiftKey && e.key.toLowerCase() === 'z') { e.preventDefault(); undo(); return; }
      if ((e.ctrlKey || e.metaKey) && (e.key.toLowerCase() === 'y' || (e.shiftKey && e.key.toLowerCase() === 'z'))) {
        e.preventDefault(); redo(); return;
      }
      if ((e.key === 'Delete' || e.key === 'Backspace') && selectedId) {
        pushUndo();
        annotations = annotations.filter(a => a.id !== selectedId);
        selectedId = null;
        return;
      }
      const shortcuts: Record<string, ToolType> = {
        s: 'select', r: 'rect', e: 'ellipse', a: 'arrow', l: 'line',
        t: 'text', p: 'pen', m: 'mosaic', h: 'highlight', n: 'counter', x: 'eraser',
      };
      if (!e.ctrlKey && !e.metaKey && shortcuts[e.key.toLowerCase()]) {
        activeTool = shortcuts[e.key.toLowerCase()];
        if (activeTool !== 'select') selectedId = null;
      }
    }
  }

  // Grab the full-monitor screenshot on demand and wait until it's available.
  // No-op if we already have it. This is the deferred capture: it runs the
  // first time the user does anything that needs real pixels (enter annotating,
  // copy, save, pin, OCR) — never at trigger time. Returns false on failure.
  let capturing = $state(false);
  async function ensureScreenshot(): Promise<boolean> {
    if (screenshotData) return true;
    if (!captureForSelection || !hasSel) return false;
    capturing = true;
    try {
      // Convert the (final) selection from CSS px to monitor-local physical px
      // so Rust crops exactly the region the user framed.
      const dpr = window.devicePixelRatio || 1;
      const region = {
        x: Math.round(sel.x * dpr), y: Math.round(sel.y * dpr),
        w: Math.round(sel.w * dpr), h: Math.round(sel.h * dpr),
      };
      const res = await captureForSelection(region);
      if (!res) return false;
      // Parent owns the state; mirror it locally so this component re-derives
      // bgSrc and the scale factors immediately.
      screenshotData   = res.data;
      screenshotWidth  = res.width;
      screenshotHeight = res.height;
      return true;
    } finally {
      capturing = false;
    }
  }

  async function enterAnnotating() {
    if (!(await ensureScreenshot())) return;
    phase = 'annotating';
  }

  function backToSelecting() {
    phase = 'selecting';
    annotations = [];
    undoStack = [];
    redoStack = [];
    currentAnn = null;
    selectedId = null;
    counterNum = 1;
    ocrActive = false;
    ocrData = null;
    ocrError = null;
    ocrEditVisible = false;
  }

  function pushUndo() {
    undoStack = [...undoStack, [...annotations]];
    redoStack = [];
  }

  function undo() {
    if (undoStack.length === 0) return;
    redoStack = [...redoStack, [...annotations]];
    annotations = undoStack[undoStack.length - 1];
    undoStack = undoStack.slice(0, -1);
  }

  function redo() {
    if (redoStack.length === 0) return;
    undoStack = [...undoStack, [...annotations]];
    annotations = redoStack[redoStack.length - 1];
    redoStack = redoStack.slice(0, -1);
  }

  // ── SELECTING phase handlers ───────────────────────────────────────
  let selDownX = 0, selDownY = 0, selDragged = false;
  // How an in-progress drag mutates the selection. 'new' = rubber-band a fresh
  // rect; 'move' = translate the whole selection; a HandleId = resize from that
  // grip. Lets the user tweak a committed selection (Snipaste-style) before
  // capturing — capture is still deferred until an action button is hit.
  type SelMode = 'new' | 'move' | HandleId;
  let selMode: SelMode = 'new';
  // Snapshot of the selection rect + pointer at drag start, for move/resize.
  let dragStart = { x: 0, y: 0, w: 0, h: 0, mx: 0, my: 0 };
  // Which grip (if any) the cursor is hovering over an existing selection.
  let hoveredSelHandle = $state<HandleId | null>(null);

  const HANDLE_HIT = 9; // px radius around a grip that counts as a hit

  // Grip positions for the current selection, in viewport px.
  function selHandles(): { id: HandleId; x: number; y: number }[] {
    const { x, y, w, h } = sel;
    return [
      { id: 'tl', x,         y },         { id: 'tc', x: x + w / 2, y },         { id: 'tr', x: x + w, y },
      { id: 'ml', x,         y: y + h/2 },                                       { id: 'mr', x: x + w, y: y + h/2 },
      { id: 'bl', x,         y: y + h },  { id: 'bc', x: x + w / 2, y: y + h },  { id: 'br', x: x + w, y: y + h },
    ];
  }
  function selHandleAt(px: number, py: number): HandleId | null {
    if (!hasSel) return null;
    for (const h of selHandles()) {
      if (Math.hypot(px - h.x, py - h.y) <= HANDLE_HIT) return h.id;
    }
    return null;
  }
  function insideSel(px: number, py: number): boolean {
    return hasSel && px >= sel.x && px <= sel.x + sel.w && py >= sel.y && py <= sel.y + sel.h;
  }

  function selDown(e: MouseEvent) {
    if (e.button !== 0) return;
    selecting = true;
    selDragged = false;
    selDownX = e.clientX; selDownY = e.clientY;

    // Decide what this drag does based on where it started, relative to any
    // existing selection: on a grip → resize; inside → move; else → new rect.
    const grip = selHandleAt(e.clientX, e.clientY);
    if (grip) {
      selMode = grip;
    } else if (insideSel(e.clientX, e.clientY)) {
      selMode = 'move';
    } else {
      selMode = 'new';
    }

    if (selMode === 'new') {
      ox = e.clientX; oy = e.clientY;
      cx = e.clientX; cy = e.clientY;
    } else {
      // Normalize so (ox,oy)=top-left, (cx,cy)=bottom-right, then snapshot.
      ox = sel.x; oy = sel.y; cx = sel.x + sel.w; cy = sel.y + sel.h;
      dragStart = { x: sel.x, y: sel.y, w: sel.w, h: sel.h, mx: e.clientX, my: e.clientY };
    }
  }

  function selMove(e: MouseEvent) {
    mx = e.clientX; my = e.clientY;
    sampleColorAt(e.clientX, e.clientY);

    if (!selecting) {
      // Idle hover: show window edge-detection only when there's no committed
      // selection; once a selection exists, surface its resize grips instead.
      if (hasSel) {
        hoveredSelHandle = selHandleAt(e.clientX, e.clientY);
        hoveredWindow = null;
      } else {
        hoveredWindow = windowAt(e.clientX, e.clientY);
      }
      return;
    }

    if (Math.hypot(e.clientX - selDownX, e.clientY - selDownY) > 3) selDragged = true;

    if (selMode === 'new') {
      cx = e.clientX; cy = e.clientY;
    } else if (selMode === 'move') {
      const dx = e.clientX - dragStart.mx;
      const dy = e.clientY - dragStart.my;
      ox = dragStart.x + dx; oy = dragStart.y + dy;
      cx = ox + dragStart.w;  cy = oy + dragStart.h;
    } else {
      // Resize from a grip. ox/oy = top-left, cx/cy = bottom-right.
      const id = selMode;
      if (id === 'tl' || id === 'ml' || id === 'bl') ox = e.clientX;
      if (id === 'tr' || id === 'mr' || id === 'br') cx = e.clientX;
      if (id === 'tl' || id === 'tc' || id === 'tr') oy = e.clientY;
      if (id === 'bl' || id === 'bc' || id === 'br') cy = e.clientY;
    }
  }

  function selUp() {
    // Click without dragging on empty space → snap to the hovered window.
    if (selecting && !selDragged && selMode === 'new') {
      const win = windowAt(selDownX, selDownY);
      if (win) {
        ox = win.x; oy = win.y;
        cx = win.x + win.w; cy = win.y + win.h;
      }
    }
    selecting = false;
    selMode = 'new';
    hoveredWindow = null;
  }

  function selDblClick() {
    ox = 0; oy = 0;
    cx = window.innerWidth; cy = window.innerHeight;
    enterAnnotating();
  }

  // Cursor for the selecting phase: resize arrows over grips, move over the
  // selection body, crosshair elsewhere.
  function selCursorFor(id: HandleId | null, inside: boolean): string {
    if (id === 'tl' || id === 'br') return 'nwse-resize';
    if (id === 'tr' || id === 'bl') return 'nesw-resize';
    if (id === 'tc' || id === 'bc') return 'ns-resize';
    if (id === 'ml' || id === 'mr') return 'ew-resize';
    if (inside) return 'move';
    return 'crosshair';
  }

  // ── ANNOTATING phase handlers ─────────────────────────────────────
  function annDown(e: MouseEvent) {
    if (e.button !== 0) return;

    if (activeTool === 'select') {
      if (selectedAnn) {
        const handles = getHandles(selectedAnn);
        const clicked = handles.find(h => Math.hypot(e.clientX - h.x, e.clientY - h.y) < 8);
        if (clicked) {
          selResizing = clicked.id;
          selDragStartMx = e.clientX; selDragStartMy = e.clientY;
          selDragOrigAnn = { ...selectedAnn } as Annotation;
          return;
        }
      }
      const hit = [...annotations].reverse().find(a => hitTest(a, e.clientX, e.clientY));
      if (hit) {
        selectedId = hit.id;
        selMoving = true;
        selDragStartMx = e.clientX; selDragStartMy = e.clientY;
        selDragOrigAnn = { ...hit } as Annotation;
      } else {
        selectedId = null;
      }
      return;
    }

    if (activeTool === 'eraser') {
      const hit = [...annotations].reverse().find(a => hitTest(a, e.clientX, e.clientY));
      if (hit) { pushUndo(); annotations = annotations.filter(a => a.id !== hit.id); }
      annDragging = true;
      return;
    }

    if (activeTool === 'text') {
      textPx = e.clientX; textPy = e.clientY - 4;
      textVal = '';
      textVisible = true;
      setTimeout(() => textInputEl?.focus(), 10);
      return;
    }

    if (activeTool === 'counter') {
      pushUndo();
      const newId = crypto.randomUUID();
      annotations = [...annotations, {
        id: newId, type: 'counter',
        color: activeColor, sw: activeStroke,
        x: e.clientX, y: e.clientY, n: counterNum,
      }];
      selectedId = newId;
      counterNum++;
      return;
    }

    annDragging = true;
    annOx = e.clientX; annOy = e.clientY;
    selectedId = null;
    const base = { id: crypto.randomUUID(), color: activeColor, sw: activeStroke };

    if (activeTool === 'rect' || activeTool === 'ellipse') {
      currentAnn = { ...base, type: activeTool, x: e.clientX, y: e.clientY, w: 0, h: 0, fill: fillMode, dash: lineStyle, round: roundCorners };
    } else if (activeTool === 'arrow') {
      currentAnn = { ...base, type: 'arrow', x1: e.clientX, y1: e.clientY, x2: e.clientX, y2: e.clientY, dash: lineStyle, head: arrowStyle };
    } else if (activeTool === 'line') {
      currentAnn = { ...base, type: 'line', x1: e.clientX, y1: e.clientY, x2: e.clientX, y2: e.clientY, dash: lineStyle };
    } else if (activeTool === 'pen') {
      currentAnn = { ...base, type: 'pen', pts: [[e.clientX, e.clientY]] };
    } else if (activeTool === 'mosaic') {
      currentAnn = { ...base, type: 'mosaic', x: e.clientX, y: e.clientY, w: 0, h: 0 };
    } else if (activeTool === 'highlight') {
      currentAnn = { ...base, type: 'highlight', x: e.clientX, y: e.clientY, w: 0, h: 0 };
    }
  }

  function annMove(e: MouseEvent) {
    if (activeTool === 'select') {
      if (!selMoving && !selResizing) {
        if (selectedAnn) {
          const handles = getHandles(selectedAnn);
          const near = handles.find(h => Math.hypot(e.clientX - h.x, e.clientY - h.y) < 8);
          hoveredHandle = near?.id ?? null;
        } else {
          hoveredHandle = null;
        }
        if (!hoveredHandle) {
          hoveredId = [...annotations].reverse().find(a => hitTest(a, e.clientX, e.clientY))?.id ?? null;
        }
      }
      if (selResizing && selDragOrigAnn) {
        annotations = annotations.map(a =>
          a.id === selectedId ? applyResize(selDragOrigAnn!, selResizing!, e.clientX, e.clientY) : a
        );
        return;
      }
      if (selMoving && selDragOrigAnn && selectedId) {
        annotations = annotations.map(a =>
          a.id === selectedId ? applyMove(selDragOrigAnn!, e.clientX - selDragStartMx, e.clientY - selDragStartMy) : a
        );
        return;
      }
      return;
    }

    if (activeTool === 'eraser' && annDragging) {
      const hit = [...annotations].reverse().find(a => hitTest(a, e.clientX, e.clientY));
      if (hit) { pushUndo(); annotations = annotations.filter(a => a.id !== hit.id); }
      return;
    }

    if (!annDragging || !currentAnn) return;

    if (currentAnn.type === 'rect' || currentAnn.type === 'ellipse' ||
        currentAnn.type === 'mosaic' || currentAnn.type === 'highlight') {
      currentAnn = {
        ...currentAnn,
        x: Math.min(annOx, e.clientX), y: Math.min(annOy, e.clientY),
        w: Math.abs(e.clientX - annOx), h: Math.abs(e.clientY - annOy),
      };
    } else if (currentAnn.type === 'arrow' || currentAnn.type === 'line') {
      currentAnn = { ...currentAnn, x2: e.clientX, y2: e.clientY };
    } else if (currentAnn.type === 'pen') {
      currentAnn = { ...currentAnn, pts: [...currentAnn.pts, [e.clientX, e.clientY]] };
    }
  }

  function annUp() {
    if (activeTool === 'select') {
      if ((selMoving || selResizing) && selDragOrigAnn && selectedId) {
        const finalAnn = annotations.find(a => a.id === selectedId);
        if (finalAnn && JSON.stringify(selDragOrigAnn) !== JSON.stringify(finalAnn)) {
          const preState = annotations.map(a => a.id === selectedId ? selDragOrigAnn! : a);
          undoStack = [...undoStack, preState];
          redoStack = [];
        }
      }
      selMoving = false;
      selResizing = null;
      selDragOrigAnn = null;
      return;
    }

    if (activeTool === 'eraser') { annDragging = false; return; }
    if (!annDragging || !currentAnn) return;
    annDragging = false;

    let valid = false;
    if (currentAnn.type === 'rect' || currentAnn.type === 'ellipse' ||
        currentAnn.type === 'mosaic' || currentAnn.type === 'highlight') {
      valid = currentAnn.w > 3 && currentAnn.h > 3;
    } else if (currentAnn.type === 'arrow' || currentAnn.type === 'line') {
      const dx = currentAnn.x2 - currentAnn.x1, dy = currentAnn.y2 - currentAnn.y1;
      valid = dx * dx + dy * dy > 25;
    } else if (currentAnn.type === 'pen') {
      valid = currentAnn.pts.length > 2;
    }

    if (valid) { pushUndo(); annotations = [...annotations, currentAnn]; selectedId = currentAnn.id; }
    currentAnn = null;
  }

  function commitText() {
    if (textVal.trim()) {
      pushUndo();
      const newId = crypto.randomUUID();
      const fontSize = 14 + activeStroke * 2;
      annotations = [...annotations, {
        id: newId, type: 'text',
        color: activeColor, sw: activeStroke,
        x: textPx, y: textPy + fontSize, text: textVal.trim(), bg: textBg,
      }];
      selectedId = newId;
    }
    textVisible = false;
    textVal = '';
  }

  // ── Arrow path helper ──────────────────────────────────────────────
  // `head` controls which ends get an arrowhead: 'end' (at x2), 'both', or
  // 'none' (a plain shaft, i.e. degenerates to a line).
  function arrowD(x1: number, y1: number, x2: number, y2: number, sw: number, head: ArrowStyle = 'end'): string {
    const dx = x2 - x1, dy = y2 - y1;
    if (Math.hypot(dx, dy) < 2) return '';
    const ang = Math.atan2(dy, dx);
    const hl = Math.max(12, sw * 4);
    const parts = [`M${x1},${y1}L${x2},${y2}`];
    const tip = (tx: number, ty: number, a: number) => {
      const a1 = a - Math.PI / 6, a2 = a + Math.PI / 6;
      parts.push(`M${tx},${ty}L${tx - hl * Math.cos(a1)},${ty - hl * Math.sin(a1)}`);
      parts.push(`M${tx},${ty}L${tx - hl * Math.cos(a2)},${ty - hl * Math.sin(a2)}`);
    };
    if (head === 'end' || head === 'both') tip(x2, y2, ang);
    if (head === 'both') tip(x1, y1, ang + Math.PI);
    return parts.join(' ');
  }

  // ── Canvas mosaic helper ───────────────────────────────────────────
  function applyMosaic(ctx: CanvasRenderingContext2D, x: number, y: number, w: number, h: number, block: number) {
    for (let bx = 0; bx < w; bx += block) {
      for (let by = 0; by < h; by += block) {
        const bw = Math.min(block, w - bx), bh = Math.min(block, h - by);
        if (bw <= 0 || bh <= 0) continue;
        const d = ctx.getImageData(x + bx, y + by, bw, bh).data;
        const n = d.length / 4;
        let r = 0, g = 0, b = 0;
        for (let i = 0; i < n; i++) { r += d[i * 4]; g += d[i * 4 + 1]; b += d[i * 4 + 2]; }
        ctx.fillStyle = `rgb(${Math.round(r / n)},${Math.round(g / n)},${Math.round(b / n)})`;
        ctx.fillRect(x + bx, y + by, bw, bh);
      }
    }
  }

  // ── Export ─────────────────────────────────────────────────────────
  async function doExport(action: 'copy' | 'save' | 'quicksave' | 'pin') {
    if (!(await ensureScreenshot())) return;
    const { data, width, height } = await compositeImage();
    if (action === 'copy')           oncopy?.(data);
    else if (action === 'save')      onsave?.(data);
    else if (action === 'quicksave') onquicksave?.(data);
    else if (action === 'pin')       onpin?.(data, width, height);
  }

  // ── OCR (text recognition + selectable text layer) ──────────────────
  interface OcrLine { text: string; x: number; y: number; w: number; h: number }
  interface OcrData { lines: OcrLine[]; img_w: number; img_h: number }

  let ocrActive = $state(false);              // text-selection mode on/off
  let ocrData = $state<OcrData | null>(null); // cached recognition for this capture
  let ocrBusy = $state(false);
  let ocrError = $state<string | null>(null);
  let ocrCopied = $state(false);
  let ocrSelection = $state('');
  let ocrSelectionRect = $state<{ top: number; left: number; width: number } | null>(null);
  let ocrEditVisible = $state(false);
  let ocrEditText = $state('');
  let ocrEditX = $state(0);
  let ocrEditY = $state(0);
  let ocrEditDragging = false;
  let ocrEditDragDX = 0, ocrEditDragDY = 0;

  $effect(() => {
    if (!ocrActive) { ocrSelection = ''; ocrSelectionRect = null; return; }
    const handler = () => {
      const s = window.getSelection();
      const text = s?.toString() ?? '';
      ocrSelection = text;
      if (text && s && s.rangeCount > 0) {
        const r = s.getRangeAt(0).getBoundingClientRect();
        ocrSelectionRect = { top: r.top, left: r.left + r.width / 2, width: r.width };
      } else {
        ocrSelectionRect = null;
      }
    };
    document.addEventListener('selectionchange', handler);
    return () => document.removeEventListener('selectionchange', handler);
  });

  // The screenshot is drawn 1:1 over the selection rect, so OCR boxes (in
  // recognised-image px) map to viewport CSS px by sel-size / screenshot-size.
  let ocrScaleX = $derived(screenshotWidth ? sel.w / screenshotWidth : 1);
  let ocrScaleY = $derived(screenshotHeight ? sel.h / screenshotHeight : 1);

  async function runOcr() {
    if (ocrActive) { ocrActive = false; return; }   // toggle off
    if (!(await ensureScreenshot())) return;
    // Freeze the capture (stable image + dim backdrop), as entering annotate does.
    phase = 'annotating';
    ocrActive = true;
    if (ocrData) return;                             // cached → instant
    ocrBusy = true;
    ocrError = null;
    try {
      // OCR the *raw* screenshot (no annotations) — best accuracy.
      const res = await invoke<OcrData>('ocr_image', { imageData: screenshotData });
      ocrData = res;
      if (!res.lines.length) ocrError = '未识别到文字';
    } catch (e) {
      ocrError = String(e);
    } finally {
      ocrBusy = false;
    }
  }

  function openOcrEdit(text: string) {
    ocrEditText = text;
    ocrEditX = Math.round((window.innerWidth - 420) / 2);
    ocrEditY = Math.round((window.innerHeight - 320) / 2);
    ocrEditVisible = true;
  }

  function ocrEditDragStart(e: MouseEvent) {
    e.preventDefault(); e.stopPropagation();
    ocrEditDragging = true;
    ocrEditDragDX = e.clientX - ocrEditX;
    ocrEditDragDY = e.clientY - ocrEditY;
    window.addEventListener('mousemove', ocrEditDragMove, true);
    window.addEventListener('mouseup', ocrEditDragEnd, true);
  }
  function ocrEditDragMove(e: MouseEvent) {
    if (!ocrEditDragging) return;
    ocrEditX = Math.max(0, Math.min(window.innerWidth - 420, e.clientX - ocrEditDragDX));
    ocrEditY = Math.max(0, Math.min(window.innerHeight - 80, e.clientY - ocrEditDragDY));
  }
  function ocrEditDragEnd() {
    ocrEditDragging = false;
    window.removeEventListener('mousemove', ocrEditDragMove, true);
    window.removeEventListener('mouseup', ocrEditDragEnd, true);
  }

  async function confirmOcrEdit() {
    if (!ocrEditText.trim()) return;
    try {
      await navigator.clipboard.writeText(ocrEditText);
      ocrCopied = true;
      setTimeout(() => { ocrCopied = false; ocrEditVisible = false; }, 800);
    } catch (_) { /* unavailable */ }
  }

  function closeOcr() { ocrActive = false; ocrEditVisible = false; }

  // Stretch a text-layer line to exactly fill its box width (pdf.js technique)
  // so the transparent selectable text lines up with the glyphs in the image.
  function fitLine(node: HTMLElement, targetW: number) {
    const apply = (w: number) => {
      node.style.transform = 'none';
      const natural = node.scrollWidth;
      if (natural > 0 && w > 0) node.style.transform = `scaleX(${w / natural})`;
    };
    requestAnimationFrame(() => apply(targetW));
    return { update(w: number) { requestAnimationFrame(() => apply(w)); } };
  }

  // ── Long screenshot: hand the chosen region (physical px) back to App ──
  function confirmScrollRegion() {
    // Scroll mode never captures a backdrop, so map CSS px → physical px via
    // the device pixel ratio (this monitor's scale factor) for capture_region.
    const dpr = window.devicePixelRatio || 1;
    onscroll?.({
      x: Math.round(sel.x * dpr),
      y: Math.round(sel.y * dpr),
      w: Math.round(sel.w * dpr),
      h: Math.round(sel.h * dpr),
    });
  }

  function compositeImage(): Promise<{ data: string; width: number; height: number }> {
    return new Promise((resolve, reject) => {
      const img = new Image();
      img.onload = () => {
        const { x, y, w, h } = sel;
        // screenshotData is already the cropped region (physical px). The crop's
        // pixel size relative to the on-screen selection (CSS px) IS the scale
        // factor — derive it from the image so it stays exact even at fractional
        // DPI. The backdrop is drawn whole (no second crop).
        const pw = screenshotWidth;
        const ph = screenshotHeight;
        const scaleX = pw / w;
        const scaleY = ph / h;
        const canvas = document.createElement('canvas');
        canvas.width = pw; canvas.height = ph;
        const ctx = canvas.getContext('2d')!;
        ctx.imageSmoothingEnabled = false;
        ctx.drawImage(img, 0, 0, pw, ph);
        ctx.imageSmoothingEnabled = true;

        const offX = x, offY = y;
        const tx = (v: number) => (v - offX) * scaleX;
        const ty = (v: number) => (v - offY) * scaleY;
        const mosaicBlock = Math.round(14 * Math.max(scaleX, scaleY));

        for (const ann of annotations) {
          if (ann.type !== 'mosaic') continue;
          const cx2 = Math.max(0, Math.round(tx(ann.x)));
          const cy2 = Math.max(0, Math.round(ty(ann.y)));
          const cw = Math.min(Math.round(ann.w * scaleX), pw - cx2);
          const ch = Math.min(Math.round(ann.h * scaleY), ph - cy2);
          if (cw > 0 && ch > 0) applyMosaic(ctx, cx2, cy2, cw, ch, mosaicBlock);
        }

        for (const ann of annotations) {
          if (ann.type === 'mosaic') continue;
          ctx.save();
          ctx.strokeStyle = ann.color;
          ctx.fillStyle = ann.color;
          ctx.lineWidth = ann.sw * scaleX;
          ctx.lineCap = 'round';
          ctx.lineJoin = 'round';
          // Dashed line style (shared by rect/ellipse/arrow/line).
          const dashed = (ann.type === 'rect' || ann.type === 'ellipse' ||
                          ann.type === 'arrow' || ann.type === 'line') && ann.dash === 'dashed';
          ctx.setLineDash(dashed ? [ann.sw * scaleX * 3, ann.sw * scaleX * 2.2] : []);

          if (ann.type === 'rect') {
            const rx = tx(ann.x), ry = ty(ann.y), rw = ann.w * scaleX, rh = ann.h * scaleY;
            const rad = ann.round ? Math.min(rw, rh) * 0.18 : 0;
            ctx.beginPath();
            if (rad > 0 && typeof ctx.roundRect === 'function') ctx.roundRect(rx, ry, rw, rh, rad);
            else ctx.rect(rx, ry, rw, rh);
            if (ann.fill) ctx.fill(); else ctx.stroke();
          } else if (ann.type === 'ellipse') {
            ctx.beginPath();
            ctx.ellipse(
              tx(ann.x) + (ann.w * scaleX) / 2, ty(ann.y) + (ann.h * scaleY) / 2,
              Math.max(0, (ann.w * scaleX) / 2), Math.max(0, (ann.h * scaleY) / 2),
              0, 0, Math.PI * 2
            );
            if (ann.fill) ctx.fill(); else ctx.stroke();
          } else if (ann.type === 'arrow') {
            const ax1 = tx(ann.x1), ay1 = ty(ann.y1), ax2 = tx(ann.x2), ay2 = ty(ann.y2);
            const ang = Math.atan2(ay2 - ay1, ax2 - ax1);
            const hl = Math.max(12, ann.sw * 4) * scaleX;
            ctx.beginPath(); ctx.moveTo(ax1, ay1); ctx.lineTo(ax2, ay2); ctx.stroke();
            ctx.setLineDash([]);
            const drawTip = (tipX: number, tipY: number, a: number) => {
              ctx.beginPath();
              ctx.moveTo(tipX, tipY);
              ctx.lineTo(tipX - hl * Math.cos(a - Math.PI / 6), tipY - hl * Math.sin(a - Math.PI / 6));
              ctx.lineTo(tipX - hl * Math.cos(a + Math.PI / 6), tipY - hl * Math.sin(a + Math.PI / 6));
              ctx.closePath(); ctx.fill();
            };
            if (ann.head === 'end' || ann.head === 'both') drawTip(ax2, ay2, ang);
            if (ann.head === 'both') drawTip(ax1, ay1, ang + Math.PI);
          } else if (ann.type === 'line') {
            ctx.beginPath(); ctx.moveTo(tx(ann.x1), ty(ann.y1)); ctx.lineTo(tx(ann.x2), ty(ann.y2)); ctx.stroke();
          } else if (ann.type === 'pen') {
            if (ann.pts.length > 1) {
              ctx.beginPath();
              ctx.moveTo(tx(ann.pts[0][0]), ty(ann.pts[0][1]));
              for (let i = 1; i < ann.pts.length; i++)
                ctx.lineTo(tx(ann.pts[i][0]), ty(ann.pts[i][1]));
              ctx.stroke();
            }
          } else if (ann.type === 'text') {
            const fs = (14 + ann.sw * 2) * scaleX;
            ctx.font = `bold ${fs}px sans-serif`;
            if (ann.bg) {
              const met = ctx.measureText(ann.text);
              const pad = fs * 0.25;
              ctx.fillStyle = 'rgba(0,0,0,0.65)';
              ctx.fillRect(tx(ann.x) - pad, ty(ann.y) - fs - pad, met.width + pad * 2, fs + pad * 2);
              ctx.fillStyle = ann.color;
            }
            ctx.fillText(ann.text, tx(ann.x), ty(ann.y));
          } else if (ann.type === 'highlight') {
            ctx.globalAlpha = 0.4;
            ctx.fillRect(tx(ann.x), ty(ann.y), ann.w * scaleX, ann.h * scaleY);
            ctx.globalAlpha = 1;
          } else if (ann.type === 'counter') {
            const r = (12 + ann.sw * 2) * scaleX;
            ctx.beginPath(); ctx.arc(tx(ann.x), ty(ann.y), r, 0, Math.PI * 2);
            ctx.fillStyle = ann.color; ctx.fill();
            ctx.font = `bold ${r * 1.1}px sans-serif`;
            ctx.fillStyle = 'white';
            ctx.textAlign = 'center'; ctx.textBaseline = 'middle';
            ctx.fillText(String(ann.n), tx(ann.x), ty(ann.y));
            ctx.textAlign = 'left'; ctx.textBaseline = 'alphabetic';
          }
          ctx.restore();
        }

        const out = canvas.toDataURL('image/png').replace('data:image/png;base64,', '');
        resolve({ data: out, width: pw, height: ph });
      };
      img.onerror = reject;
      img.src = bgSrc;
    });
  }

  // ── Cursor ─────────────────────────────────────────────────────────
  let cursorStyle = $derived.by(() => {
    if (phase === 'selecting') {
      // While actively dragging, reflect the live drag mode; otherwise reflect
      // what's under the cursor (grip / inside / empty).
      if (selecting) {
        if (selMode === 'move') return 'move';
        if (selMode !== 'new') return selCursorFor(selMode, false);
        return 'crosshair';
      }
      return selCursorFor(hoveredSelHandle, insideSel(mx, my));
    }
    if (activeTool === 'text') return 'text';
    if (activeTool === 'eraser') return 'cell';
    if (activeTool === 'select') {
      if (hoveredHandle === 'tl' || hoveredHandle === 'br') return 'nwse-resize';
      if (hoveredHandle === 'tr' || hoveredHandle === 'bl') return 'nesw-resize';
      if (hoveredHandle === 'tc' || hoveredHandle === 'bc') return 'ns-resize';
      if (hoveredHandle === 'ml' || hoveredHandle === 'mr') return 'ew-resize';
      if (hoveredHandle === 'p1' || hoveredHandle === 'p2') return 'crosshair';
      if (hoveredId || selMoving || selResizing) return 'move';
      return 'default';
    }
    return 'crosshair';
  });
</script>

<svelte:window onkeydown={onKeyDown} />

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="fixed inset-0"
  style="cursor:{cursorStyle}"
  onmousedown={phase === 'selecting' ? selDown : annDown}
  onmousemove={phase === 'selecting' ? selMove : annMove}
  onmouseup={phase === 'selecting' ? selUp : annUp}
  onwheel={onWheel}
  ondblclick={phase === 'selecting' ? selDblClick : undefined}
  oncontextmenu={(e) => {
    e.preventDefault();
    if (phase === 'annotating') { selectedId = null; backToSelecting(); }
    else oncancel?.();
  }}
>
  <!-- Backdrop only exists once captured (annotating phase). It's the cropped
       region, so position it exactly over the selection rect (1:1). During live
       selection the overlay is transparent so the real desktop shows through. -->
  {#if screenshotData}
    <img
      src={bgSrc} alt=""
      class="absolute object-fill pointer-events-none"
      style="left:{sel.x}px; top:{sel.y}px; width:{sel.w}px; height:{sel.h}px; animation: snapx-fade-in 0.14s ease both;"
      draggable={false}
    />
  {/if}

  <!-- OCR selectable text layer: transparent, per-line positioned text laid
       over the screenshot so the user can drag-select / double-click words /
       copy, just like selecting text in a PDF. -->
  {#if ocrActive && ocrData}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="absolute z-[55] snapx-ocr-layer"
      style="left:{sel.x}px; top:{sel.y}px; width:{sel.w}px; height:{sel.h}px;"
      onmousedown={(e) => e.stopPropagation()}
      onmousemove={(e) => e.stopPropagation()}
      onmouseup={(e) => e.stopPropagation()}
      ondblclick={(e) => e.stopPropagation()}
    >
      {#each ocrData.lines as line}
        <div
          class="snapx-ocr-line"
          style="left:{line.x * ocrScaleX}px; top:{line.y * ocrScaleY}px; height:{line.h * ocrScaleY}px; line-height:{line.h * ocrScaleY}px; font-size:{line.h * ocrScaleY * 0.78}px;"
          use:fitLine={line.w * ocrScaleX}
        >{line.text}</div>
      {/each}
    </div>
  {/if}

  <svg class="absolute inset-0 w-full h-full pointer-events-none" overflow="visible">
    <defs>
      <mask id="dim-mask">
        <rect x="0" y="0" width="100%" height="100%" fill="white" />
        {#if hasSel}<rect x={sel.x} y={sel.y} width={sel.w} height={sel.h} fill="black" />{/if}
      </mask>
      <clipPath id="sel-clip">
        <rect x={sel.x} y={sel.y} width={sel.w} height={sel.h} />
      </clipPath>
      <pattern id="mosaic-pat" x="0" y="0" width="12" height="12" patternUnits="userSpaceOnUse">
        <rect x="0" y="0" width="6" height="6" fill="rgba(40,40,40,0.88)"/>
        <rect x="6" y="0" width="6" height="6" fill="rgba(85,85,85,0.88)"/>
        <rect x="0" y="6" width="6" height="6" fill="rgba(85,85,85,0.88)"/>
        <rect x="6" y="6" width="6" height="6" fill="rgba(40,40,40,0.88)"/>
      </pattern>
    </defs>

    <!-- Dim only during annotating (subtle, makes the crop pop). The selecting
         phase has NO dim: the overlay is fully transparent over the live desktop,
         so the deferred capture can grab clean pixels without hiding the window. -->
    {#if phase === 'annotating'}
      <rect x="0" y="0" width="100%" height="100%"
        fill="rgba(0,0,0,0.25)" mask="url(#dim-mask)"
        style="animation: snapx-fade-in 0.14s ease both;" />
    {/if}

    <!-- Hovered window outline (snap-to-window edge detection) -->
    {#if phase === 'selecting' && !selecting && !hasSel && hoveredWindow}
      <rect x={hoveredWindow.x} y={hoveredWindow.y}
        width={hoveredWindow.w} height={hoveredWindow.h}
        fill="rgba(59,130,246,0.12)" stroke="rgba(59,130,246,0.9)"
        stroke-width="1.5" stroke-dasharray="6 3" />
    {/if}

    {#if hasSel}
      <!-- Border sits 1px OUTSIDE the selection rect so its stroke never lands
           inside the capture region — the grab stays clean and the border can
           stay painted right through the capture (no flicker on commit). -->
      <rect x={sel.x - 1.5} y={sel.y - 1.5} width={sel.w + 3} height={sel.h + 3}
        fill="none"
        stroke={phase === 'annotating' ? '#3b82f6' : 'rgba(59,130,246,0.9)'}
        stroke-width="2" />
      {#if phase === 'selecting' && !capturing && nearSel}
        <!-- 8 resize grips (corners + edge midpoints). Drag any to resize, or
             drag inside the box to move it — handled in selDown/selMove.
             Shown only on hover (nearSel) so a committed region looks clean,
             and hidden during the grab so they don't bake into the crop. -->
        {#each selHandles() as h}
          <rect x={h.x - 4} y={h.y - 4} width="8" height="8"
            fill={hoveredSelHandle === h.id ? 'rgba(59,130,246,0.95)' : 'white'}
            stroke="rgba(59,130,246,0.9)" stroke-width="1.5" />
        {/each}
      {/if}
    {/if}

    <g clip-path="url(#sel-clip)">
      {#snippet annShape(ann: Annotation, preview: boolean)}
        {#if ann.type === 'rect'}
          {#if ann.fill}
            <rect x={ann.x} y={ann.y} width={ann.w} height={ann.h}
              rx={ann.round ? Math.min(ann.w, ann.h) * 0.18 : 0}
              fill={ann.color} opacity={preview ? 0.55 : 1} />
          {:else}
            <rect x={ann.x} y={ann.y} width={ann.w} height={ann.h}
              rx={ann.round ? Math.min(ann.w, ann.h) * 0.18 : 0}
              fill="none" stroke={ann.color} stroke-width={ann.sw}
              stroke-dasharray={ann.dash === 'dashed' ? `${ann.sw * 3} ${ann.sw * 2.2}` : 'none'}
              stroke-linecap="round" stroke-linejoin="round" opacity={preview ? 0.65 : 1} />
          {/if}
        {:else if ann.type === 'ellipse'}
          {#if ann.fill}
            <ellipse cx={ann.x + ann.w / 2} cy={ann.y + ann.h / 2}
              rx={Math.max(0, ann.w / 2)} ry={Math.max(0, ann.h / 2)}
              fill={ann.color} opacity={preview ? 0.55 : 1} />
          {:else}
            <ellipse cx={ann.x + ann.w / 2} cy={ann.y + ann.h / 2}
              rx={Math.max(0, ann.w / 2)} ry={Math.max(0, ann.h / 2)}
              fill="none" stroke={ann.color} stroke-width={ann.sw}
              stroke-dasharray={ann.dash === 'dashed' ? `${ann.sw * 3} ${ann.sw * 2.2}` : 'none'}
              opacity={preview ? 0.65 : 1} />
          {/if}
        {:else if ann.type === 'arrow'}
          <path d={arrowD(ann.x1, ann.y1, ann.x2, ann.y2, ann.sw, ann.head)}
            stroke={ann.color} fill={ann.color} stroke-width={ann.sw}
            stroke-dasharray={ann.dash === 'dashed' ? `${ann.sw * 3} ${ann.sw * 2.2}` : 'none'}
            stroke-linecap="round" stroke-linejoin="round" opacity={preview ? 0.65 : 1} />
        {:else if ann.type === 'line'}
          <line x1={ann.x1} y1={ann.y1} x2={ann.x2} y2={ann.y2}
            stroke={ann.color} stroke-width={ann.sw}
            stroke-dasharray={ann.dash === 'dashed' ? `${ann.sw * 3} ${ann.sw * 2.2}` : 'none'}
            stroke-linecap="round" opacity={preview ? 0.65 : 1} />
        {:else if ann.type === 'pen'}
          <polyline points={ann.pts.map(([px, py]) => `${px},${py}`).join(' ')}
            fill="none" stroke={ann.color} stroke-width={ann.sw}
            stroke-linecap="round" stroke-linejoin="round" opacity={preview ? 0.65 : 1} />
        {:else if ann.type === 'text'}
          {#if ann.bg}
            <rect
              x={ann.x - 3} y={ann.y - (14 + ann.sw * 2) - 2}
              width={ann.text.length * (14 + ann.sw * 2) * 0.62 + 6}
              height={(14 + ann.sw * 2) + 4}
              fill="rgba(0,0,0,0.65)" rx="2" opacity={preview ? 0.65 : 1} />
          {/if}
          <text x={ann.x} y={ann.y} fill={ann.color}
            font-size={14 + ann.sw * 2} font-weight="bold" font-family="sans-serif"
            opacity={preview ? 0.65 : 1}>{ann.text}</text>
        {:else if ann.type === 'mosaic'}
          <rect x={ann.x} y={ann.y} width={ann.w} height={ann.h}
            fill="url(#mosaic-pat)" opacity={preview ? 0.55 : 0.82} />
          <rect x={ann.x} y={ann.y} width={ann.w} height={ann.h}
            fill="none" stroke="rgba(255,255,255,0.25)" stroke-width="1"
            stroke-dasharray="4 2" opacity={preview ? 0.55 : 0.82} />
        {:else if ann.type === 'highlight'}
          <rect x={ann.x} y={ann.y} width={ann.w} height={ann.h}
            fill={ann.color} opacity={preview ? 0.22 : 0.4} />
        {:else if ann.type === 'counter'}
          <circle cx={ann.x} cy={ann.y} r={12 + ann.sw * 2}
            fill={ann.color} opacity={preview ? 0.65 : 1} />
          <text x={ann.x} y={ann.y} fill="white"
            font-size={12 + ann.sw * 2} font-weight="bold" font-family="sans-serif"
            text-anchor="middle" dominant-baseline="central"
            opacity={preview ? 0.65 : 1}>{ann.n}</text>
        {/if}
      {/snippet}

      {#each annotations as ann (ann.id)}
        {@render annShape(ann, false)}
      {/each}
      {#if currentAnn}
        {@render annShape(currentAnn, true)}
      {/if}

      <!-- Selection overlay -->
      {#if activeTool === 'select' && selectedAnn}
        {#if selectedAnn.type === 'rect' || selectedAnn.type === 'ellipse' ||
             selectedAnn.type === 'mosaic' || selectedAnn.type === 'highlight'}
          <rect x={selectedAnn.x - 3} y={selectedAnn.y - 3}
            width={selectedAnn.w + 6} height={selectedAnn.h + 6}
            fill="none" stroke="#3b82f6" stroke-width="1.5" stroke-dasharray="5 3" />
        {:else if selectedAnn.type === 'arrow' || selectedAnn.type === 'line'}
          <line x1={selectedAnn.x1} y1={selectedAnn.y1} x2={selectedAnn.x2} y2={selectedAnn.y2}
            stroke="#3b82f6" stroke-width={selectedAnn.sw + 8} stroke-linecap="round" opacity="0.25" />
        {:else if selectedAnn.type === 'pen'}
          <polyline points={selectedAnn.pts.map(([px, py]) => `${px},${py}`).join(' ')}
            fill="none" stroke="#3b82f6" stroke-width={selectedAnn.sw + 8}
            stroke-linecap="round" stroke-linejoin="round" opacity="0.25" />
        {:else if selectedAnn.type === 'text'}
          <rect x={selectedAnn.x - 5} y={selectedAnn.y - (14 + selectedAnn.sw * 2) - 4}
            width={selectedAnn.text.length * (14 + selectedAnn.sw * 2) * 0.62 + 10}
            height={(14 + selectedAnn.sw * 2) + 8}
            fill="none" stroke="#3b82f6" stroke-width="1.5" stroke-dasharray="5 3" rx="2" />
        {:else if selectedAnn.type === 'counter'}
          <circle cx={selectedAnn.x} cy={selectedAnn.y} r={12 + selectedAnn.sw * 2 + 5}
            fill="none" stroke="#3b82f6" stroke-width="1.5" stroke-dasharray="5 3" />
        {/if}
        {#each getHandles(selectedAnn) as handle}
          <rect x={handle.x - 5} y={handle.y - 5} width="10" height="10"
            fill="white" stroke="#3b82f6" stroke-width="1.5" rx="2" />
        {/each}
      {/if}
    </g>
  </svg>

  {#if phase === 'selecting' && hasSel && !capturing}
    <SizeIndicator x={sel.x} y={sel.y} width={sel.w} height={sel.h} />
  {/if}

  {#if phase === 'selecting' && magnifierSrc}
    <Magnifier screenshotSrc={magnifierSrc} mouseX={mx} mouseY={my} color={pickedColor} copied={colorCopied} {colorFormat} />
  {/if}

  {#if scrollMode && hasSel && !selecting}
    <!-- Long-screenshot: confirm the region, then scroll-capture begins -->
    <div class="fixed left-1/2 -translate-x-1/2 z-50" style="top:{sel.y + sel.h + 10 > window.innerHeight - 50 ? sel.y - 46 : sel.y + sel.h + 10}px;">
      <button
        class="flex items-center gap-1.5 px-3 py-1.5 text-sm text-white rounded-lg font-medium shadow-2xl"
        style="background: var(--accent); animation: snapx-pop-in 0.16s var(--ease) both;"
        onmousedown={(e) => e.stopPropagation()}
        onclick={confirmScrollRegion}
      >📜 开始长截图 (Enter)</button>
    </div>
  {:else if hasSel && !selecting}
    <Toolbar
      selX={sel.x} selY={sel.y} selW={sel.w} selH={sel.h}
      {phase} {activeTool} color={activeColor} strokeWidth={activeStroke}
      {fillMode} {textBg} {lineStyle} {roundCorners} {arrowStyle}
      styleType={selectedAnn?.type ?? activeTool}
      hasSelection={selectedAnn !== null}
      canUndo={undoStack.length > 0} canRedo={redoStack.length > 0}
      onAnnotate={enterAnnotating}
      onToolChange={(t) => {
        activeTool = t;
        if (phase === 'selecting') enterAnnotating();
        if (t !== 'select') selectedId = null;
      }}
      onColorChange={(c) => { activeColor = c; patchSelected({ color: c }); }}
      onStrokeWidthChange={(w) => { activeStroke = w; patchSelected({ sw: w }); }}
      onFillToggle={() => {
        fillMode = !fillMode;
        if (selectedAnn && (selectedAnn.type === 'rect' || selectedAnn.type === 'ellipse'))
          patchSelected({ fill: fillMode });
      }}
      onTextBgToggle={() => {
        textBg = !textBg;
        if (selectedAnn?.type === 'text') patchSelected({ bg: textBg });
      }}
      onLineStyleChange={(s) => { lineStyle = s; patchSelected({ dash: s }); }}
      onRoundToggle={() => {
        roundCorners = !roundCorners;
        if (selectedAnn?.type === 'rect') patchSelected({ round: roundCorners });
      }}
      onArrowStyleChange={(s) => {
        arrowStyle = s;
        if (selectedAnn?.type === 'arrow') patchSelected({ head: s });
      }}
      onUndo={undo}
      onRedo={redo}
      onOcr={runOcr}
      {ocrActive}
      onCopy={() => doExport('copy')}
      onSave={() => doExport('save')}
      onQuickSave={() => doExport('quicksave')}
      onPin={() => doExport('pin')}
      onCancel={() => {
        if (phase === 'annotating') { selectedId = null; backToSelecting(); }
        else oncancel?.();
      }}
    />
  {/if}

  {#if textVisible}
    <!-- svelte-ignore a11y_autofocus -->
    <input
      bind:this={textInputEl}
      bind:value={textVal}
      autofocus
      class="fixed z-50 bg-transparent outline-none font-bold min-w-32 border-b-2"
      style="left:{textPx}px;top:{textPy}px;font-size:{14 + activeStroke * 2}px;color:{activeColor};border-color:{activeColor};caret-color:{activeColor};"
      onkeydown={(e) => {
        if (e.key === 'Enter') { e.preventDefault(); commitText(); }
        if (e.key === 'Escape') { textVisible = false; }
        e.stopPropagation();
      }}
      onblur={commitText}
    />
  {/if}

  {#if !hasSel && !selecting}
    <div class="absolute inset-0 flex items-center justify-center pointer-events-none">
      <div class="bg-black/70 text-white px-6 py-3 rounded-xl text-sm backdrop-blur-sm space-y-1 text-center">
        <div>拖拽选择 · 单击窗口自动识别 · 双击全屏</div>
        <div class="text-white/50 text-xs">右键 / ESC 取消</div>
      </div>
    </div>
  {/if}

  {#if phase === 'annotating' && hasSel}
    <div class="fixed top-3 left-1/2 -translate-x-1/2 pointer-events-none z-40">
      <div class="bg-black/60 text-white/70 px-4 py-1.5 rounded-full text-xs backdrop-blur-sm">
        S选择 R矩形 E椭圆 A箭头 L直线 T文字 P画笔 M马赛克 H高亮 N序号 X橡皮
        · Ctrl+Z/Y 撤销/重做 · Del 删除选中 · Enter 复制 · ESC 返回
      </div>
    </div>
  {/if}

  <!-- OCR text-selection mode: floating status / controls -->
  {#if ocrActive}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="fixed top-3 left-1/2 -translate-x-1/2 z-[60]"
      style="animation: snapx-fade-in 0.12s ease both;"
      onmousedown={(e) => e.stopPropagation()}
    >
      {#if ocrBusy}
        <div class="bg-black/70 text-white/80 px-4 py-1.5 rounded-full text-xs backdrop-blur-sm">识别中…</div>
      {:else if ocrError}
        <div class="bg-black/70 px-4 py-1.5 rounded-full text-xs backdrop-blur-sm flex items-center gap-3">
          <span class="text-red-300/90">{ocrError}</span>
          <button class="text-white/70 hover:text-white" onclick={closeOcr}>退出 (ESC)</button>
        </div>
      {:else if ocrData}
        <div class="bg-black/70 text-white px-3 py-1.5 rounded-full text-xs backdrop-blur-sm flex items-center gap-2 shadow-2xl">
          <span class="text-white/55">拖拽选择文字 · 双击选词</span>
          <button
            class="px-2.5 py-1 rounded-md text-white font-medium"
            style="background: var(--accent);"
            onclick={() => openOcrEdit(ocrData!.lines.map(l => l.text).join('\n'))}
          >复制全部</button>
          <button class="px-2 py-1 rounded-md text-white/70 hover:text-white hover:bg-white/10" onclick={closeOcr}>退出 (ESC)</button>
        </div>
      {/if}
    </div>
  {/if}

  <!-- Floating copy button: appears above the drag-selected text in the OCR layer -->
  {#if ocrActive && ocrSelection && ocrSelectionRect}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="fixed z-[65] pointer-events-none"
      style="left:{ocrSelectionRect.left}px; top:{Math.max(36, ocrSelectionRect.top - 8)}px; transform:translate(-50%,-100%);"
    >
      <button
        class="pointer-events-auto px-3 py-1 rounded-lg text-white text-xs font-semibold shadow-xl"
        style="background:var(--accent); animation:snapx-pop-in 0.1s var(--ease) both;"
        onmousedown={(e) => { e.preventDefault(); e.stopPropagation(); }}
        onclick={() => openOcrEdit(ocrSelection)}
      >复制</button>
    </div>
  {/if}

  <!-- OCR edit dialog: draggable floating panel, no backdrop so user can compare -->
  {#if ocrEditVisible}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="fixed z-[70] bg-[#1e1e1e] rounded-2xl shadow-2xl border border-white/10 flex flex-col"
      style="left:{ocrEditX}px; top:{ocrEditY}px; width:420px; max-width:calc(100vw - 16px); animation:snapx-pop-in 0.12s var(--ease) both;"
      onmousedown={(e) => e.stopPropagation()}
      onmousemove={(e) => e.stopPropagation()}
      onmouseup={(e) => e.stopPropagation()}
    >
      <!-- Title bar = drag handle -->
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div
        class="flex items-center justify-between px-4 pt-3.5 pb-2 cursor-grab active:cursor-grabbing select-none"
        onmousedown={ocrEditDragStart}
      >
        <div class="text-white/80 text-sm font-semibold">
          识别结果
          <span class="text-white/35 font-normal text-xs ml-1.5">可修改后复制</span>
        </div>
        <button
          class="text-white/35 hover:text-white/80 text-xl leading-none transition-colors px-1"
          onmousedown={(e) => e.stopPropagation()}
          onclick={() => { ocrEditVisible = false; }}
        >×</button>
      </div>
      <div class="px-4 pb-4 flex flex-col gap-3">
        <textarea
          class="bg-black/40 text-white text-sm rounded-xl p-3 border border-white/10 resize-none outline-none leading-relaxed"
          style="height:180px; font-family:inherit;"
          bind:value={ocrEditText}
          spellcheck={false}
          onkeydown={(e) => { if (e.key === 'Escape') { e.stopPropagation(); ocrEditVisible = false; } }}
        ></textarea>
        <div class="flex gap-2 justify-end">
          <button
            class="px-3 py-1.5 rounded-lg text-sm text-white/55 hover:text-white hover:bg-white/10 transition-colors"
            onclick={() => { ocrEditVisible = false; }}
          >取消</button>
          <button
            class="px-5 py-1.5 rounded-lg text-sm text-white font-medium transition-colors"
            style="background:var(--accent);"
            onclick={confirmOcrEdit}
          >{ocrCopied ? '已复制 ✓' : '复制'}</button>
        </div>
      </div>
    </div>
  {/if}
</div>
