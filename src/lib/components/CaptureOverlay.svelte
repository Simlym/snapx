<script lang="ts">
  import Magnifier from './Magnifier.svelte';
  import SizeIndicator from './SizeIndicator.svelte';
  import Toolbar from './Toolbar.svelte';
  import type { ToolType } from './Toolbar.svelte';

  // ── Annotation types ───────────────────────────────────────────────
  interface AnnBase       { id: string; color: string; sw: number; }
  interface BoxAnn        extends AnnBase { type: 'rect' | 'ellipse'; x: number; y: number; w: number; h: number; fill: boolean; }
  interface ArrowAnn      extends AnnBase { type: 'arrow'; x1: number; y1: number; x2: number; y2: number; }
  interface LineAnn       extends AnnBase { type: 'line';  x1: number; y1: number; x2: number; y2: number; }
  interface PenAnn        extends AnnBase { type: 'pen'; pts: [number, number][]; }
  interface TextAnn       extends AnnBase { type: 'text'; x: number; y: number; text: string; bg: boolean; }
  interface MosaicAnn     extends AnnBase { type: 'mosaic'; x: number; y: number; w: number; h: number; }
  interface HighlightAnn  extends AnnBase { type: 'highlight'; x: number; y: number; w: number; h: number; }
  interface CounterAnn    extends AnnBase { type: 'counter'; x: number; y: number; n: number; }
  type Annotation = BoxAnn | ArrowAnn | LineAnn | PenAnn | TextAnn | MosaicAnn | HighlightAnn | CounterAnn;
  type HandleId = 'tl' | 'tc' | 'tr' | 'ml' | 'mr' | 'bl' | 'bc' | 'br' | 'p1' | 'p2';

  // ── Props ──────────────────────────────────────────────────────────
  interface Props {
    screenshotData: string;
    screenshotWidth: number;
    screenshotHeight: number;
    oncopy?: (imageData: string) => void;
    onsave?: (imageData: string) => void;
    onquicksave?: (imageData: string) => void;
    onpin?: (imageData: string, pxW: number, pxH: number) => void;
    oncancel?: () => void;
  }

  let { screenshotData, screenshotWidth, screenshotHeight, oncopy, onsave, onquicksave, onpin, oncancel }: Props = $props();
  let bgSrc = $derived(`data:image/png;base64,${screenshotData}`);

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

  // ── Eyedropper (colour under cursor) ──────────────────────────────
  // An offscreen canvas holds the full screenshot so we can read the exact
  // pixel colour under the cursor while choosing a region (Snipaste-style).
  // Built lazily on first sample so the capture path stays fast.
  let pickedColor = $state('');
  let colorCopied = $state(false);
  let sampleCanvas: HTMLCanvasElement | null = null;
  let sampleCtx: CanvasRenderingContext2D | null = null;
  let sampleImg: HTMLImageElement | null = null;

  function ensureSampleCanvas() {
    if (sampleCtx || sampleImg) return;
    sampleImg = new Image();
    sampleImg.onload = () => {
      const c = document.createElement('canvas');
      c.width = screenshotWidth;
      c.height = screenshotHeight;
      const ctx = c.getContext('2d', { willReadFrequently: true });
      if (ctx) {
        ctx.drawImage(sampleImg!, 0, 0, screenshotWidth, screenshotHeight);
        sampleCanvas = c;
        sampleCtx = ctx;
      }
    };
    sampleImg.src = bgSrc;
  }

  function sampleColorAt(clientX: number, clientY: number) {
    ensureSampleCanvas();
    if (!sampleCtx) return;
    const scaleX = screenshotWidth / window.innerWidth;
    const scaleY = screenshotHeight / window.innerHeight;
    const px = Math.max(0, Math.min(screenshotWidth - 1, Math.round(clientX * scaleX)));
    const py = Math.max(0, Math.min(screenshotHeight - 1, Math.round(clientY * scaleY)));
    try {
      const d = sampleCtx.getImageData(px, py, 1, 1).data;
      pickedColor = '#' + [d[0], d[1], d[2]].map((v) => v.toString(16).padStart(2, '0')).join('');
    } catch (_) { /* not ready */ }
  }

  async function copyPickedColor() {
    if (!pickedColor) return;
    try {
      await navigator.clipboard.writeText(pickedColor);
      colorCopied = true;
      setTimeout(() => (colorCopied = false), 900);
    } catch (_) { /* clipboard unavailable */ }
  }

  // ── Annotation state ───────────────────────────────────────────────
  let activeTool: ToolType = $state('rect');
  let activeColor = $state('#ef4444');
  let activeStroke = $state(3);
  let fillMode = $state(false);
  let textBg = $state(false);
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

  // ── Keyboard ───────────────────────────────────────────────────────
  function onKeyDown(e: KeyboardEvent) {
    if (textVisible && e.key !== 'Escape') return;

    if (e.key === 'Escape') {
      if (textVisible) { textVisible = false; return; }
      if (phase === 'annotating') { selectedId = null; backToSelecting(); return; }
      oncancel?.();
      return;
    }

    if (e.key === 'Enter') {
      if (phase === 'selecting' && hasSel) { enterAnnotating(); return; }
      if (phase === 'annotating') { doExport('copy'); return; }
    }

    // Eyedropper: copy the colour under the cursor while still choosing a region.
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

  function enterAnnotating() { phase = 'annotating'; }

  function backToSelecting() {
    phase = 'selecting';
    annotations = [];
    undoStack = [];
    redoStack = [];
    currentAnn = null;
    selectedId = null;
    counterNum = 1;
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
  function selDown(e: MouseEvent) {
    if (e.button !== 0) return;
    selecting = true;
    ox = e.clientX; oy = e.clientY;
    cx = e.clientX; cy = e.clientY;
  }
  function selMove(e: MouseEvent) {
    mx = e.clientX; my = e.clientY;
    sampleColorAt(e.clientX, e.clientY);
    if (selecting) { cx = e.clientX; cy = e.clientY; }
  }
  function selUp() { selecting = false; }
  function selDblClick() {
    ox = 0; oy = 0;
    cx = window.innerWidth; cy = window.innerHeight;
    enterAnnotating();
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
      annotations = [...annotations, {
        id: crypto.randomUUID(), type: 'counter',
        color: activeColor, sw: activeStroke,
        x: e.clientX, y: e.clientY, n: counterNum,
      }];
      counterNum++;
      return;
    }

    annDragging = true;
    annOx = e.clientX; annOy = e.clientY;
    const base = { id: crypto.randomUUID(), color: activeColor, sw: activeStroke };

    if (activeTool === 'rect' || activeTool === 'ellipse') {
      currentAnn = { ...base, type: activeTool, x: e.clientX, y: e.clientY, w: 0, h: 0, fill: fillMode };
    } else if (activeTool === 'arrow') {
      currentAnn = { ...base, type: 'arrow', x1: e.clientX, y1: e.clientY, x2: e.clientX, y2: e.clientY };
    } else if (activeTool === 'line') {
      currentAnn = { ...base, type: 'line', x1: e.clientX, y1: e.clientY, x2: e.clientX, y2: e.clientY };
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

    if (valid) { pushUndo(); annotations = [...annotations, currentAnn]; }
    currentAnn = null;
  }

  function commitText() {
    if (textVal.trim()) {
      pushUndo();
      const fontSize = 14 + activeStroke * 2;
      annotations = [...annotations, {
        id: crypto.randomUUID(), type: 'text',
        color: activeColor, sw: activeStroke,
        x: textPx, y: textPy + fontSize, text: textVal.trim(), bg: textBg,
      }];
    }
    textVisible = false;
    textVal = '';
  }

  // ── Arrow path helper ──────────────────────────────────────────────
  function arrowD(x1: number, y1: number, x2: number, y2: number, sw: number): string {
    const dx = x2 - x1, dy = y2 - y1;
    if (Math.hypot(dx, dy) < 2) return '';
    const ang = Math.atan2(dy, dx);
    const hl = Math.max(12, sw * 4);
    const a1 = ang - Math.PI / 6, a2 = ang + Math.PI / 6;
    return [
      `M${x1},${y1}L${x2},${y2}`,
      `M${x2},${y2}L${x2 - hl * Math.cos(a1)},${y2 - hl * Math.sin(a1)}`,
      `M${x2},${y2}L${x2 - hl * Math.cos(a2)},${y2 - hl * Math.sin(a2)}`,
    ].join(' ');
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
    const { data, width, height } = await compositeImage();
    if (action === 'copy')           oncopy?.(data);
    else if (action === 'save')      onsave?.(data);
    else if (action === 'quicksave') onquicksave?.(data);
    else if (action === 'pin')       onpin?.(data, width, height);
  }

  function compositeImage(): Promise<{ data: string; width: number; height: number }> {
    return new Promise((resolve, reject) => {
      const img = new Image();
      img.onload = () => {
        const { x, y, w, h } = sel;
        const scaleX = screenshotWidth / window.innerWidth;
        const scaleY = screenshotHeight / window.innerHeight;
        // Snap source crop to integer physical px. A fractional src origin
        // (e.g. x*1.25 at 125% scaling) makes drawImage bilinearly resample the
        // whole crop → soft image. Integer src rect + equal dst = 1:1 blit.
        const sx = Math.round(x * scaleX);
        const sy = Math.round(y * scaleY);
        const pw = Math.round(w * scaleX);
        const ph = Math.round(h * scaleY);
        const canvas = document.createElement('canvas');
        canvas.width = pw; canvas.height = ph;
        const ctx = canvas.getContext('2d')!;
        ctx.imageSmoothingEnabled = false;
        ctx.drawImage(img, sx, sy, pw, ph, 0, 0, pw, ph);
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

          if (ann.type === 'rect') {
            if (ann.fill) ctx.fillRect(tx(ann.x), ty(ann.y), ann.w * scaleX, ann.h * scaleY);
            else ctx.strokeRect(tx(ann.x), ty(ann.y), ann.w * scaleX, ann.h * scaleY);
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
            ctx.beginPath();
            ctx.moveTo(ax2, ay2);
            ctx.lineTo(ax2 - hl * Math.cos(ang - Math.PI / 6), ay2 - hl * Math.sin(ang - Math.PI / 6));
            ctx.lineTo(ax2 - hl * Math.cos(ang + Math.PI / 6), ay2 - hl * Math.sin(ang + Math.PI / 6));
            ctx.closePath(); ctx.fill();
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
    if (phase === 'selecting') return 'crosshair';
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
  ondblclick={phase === 'selecting' ? selDblClick : undefined}
  oncontextmenu={(e) => {
    e.preventDefault();
    if (phase === 'annotating') { selectedId = null; backToSelecting(); }
    else oncancel?.();
  }}
>
  <img
    src={bgSrc} alt=""
    class="absolute inset-0 w-full h-full object-fill pointer-events-none"
    draggable={false}
  />

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

    <rect x="0" y="0" width="100%" height="100%"
      fill={phase === 'annotating' ? 'rgba(0,0,0,0.25)' : 'rgba(0,0,0,0.52)'}
      mask="url(#dim-mask)" />

    {#if hasSel}
      <rect x={sel.x} y={sel.y} width={sel.w} height={sel.h}
        fill="none"
        stroke={phase === 'annotating' ? '#3b82f6' : 'rgba(59,130,246,0.9)'}
        stroke-width="2" />
      {#if phase === 'selecting'}
        {#each [[sel.x, sel.y], [sel.x + sel.w, sel.y], [sel.x, sel.y + sel.h], [sel.x + sel.w, sel.y + sel.h]] as [hx, hy]}
          <rect x={hx! - 4} y={hy! - 4} width="8" height="8"
            fill="white" stroke="rgba(59,130,246,0.9)" stroke-width="1.5" />
        {/each}
      {/if}
    {/if}

    <g clip-path="url(#sel-clip)">
      {#snippet annShape(ann: Annotation, preview: boolean)}
        {#if ann.type === 'rect'}
          {#if ann.fill}
            <rect x={ann.x} y={ann.y} width={ann.w} height={ann.h}
              fill={ann.color} opacity={preview ? 0.55 : 1} />
          {:else}
            <rect x={ann.x} y={ann.y} width={ann.w} height={ann.h}
              fill="none" stroke={ann.color} stroke-width={ann.sw}
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
              fill="none" stroke={ann.color} stroke-width={ann.sw} opacity={preview ? 0.65 : 1} />
          {/if}
        {:else if ann.type === 'arrow'}
          <path d={arrowD(ann.x1, ann.y1, ann.x2, ann.y2, ann.sw)}
            stroke={ann.color} fill={ann.color} stroke-width={ann.sw}
            stroke-linecap="round" stroke-linejoin="round" opacity={preview ? 0.65 : 1} />
        {:else if ann.type === 'line'}
          <line x1={ann.x1} y1={ann.y1} x2={ann.x2} y2={ann.y2}
            stroke={ann.color} stroke-width={ann.sw}
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

  {#if phase === 'selecting' && hasSel}
    <SizeIndicator x={sel.x} y={sel.y} width={sel.w} height={sel.h} />
  {/if}

  {#if phase === 'selecting'}
    <Magnifier screenshotSrc={bgSrc} mouseX={mx} mouseY={my} color={pickedColor} copied={colorCopied} />
  {/if}

  {#if hasSel && !selecting}
    <Toolbar
      selX={sel.x} selY={sel.y} selW={sel.w} selH={sel.h}
      {phase} {activeTool} color={activeColor} strokeWidth={activeStroke}
      {fillMode} {textBg}
      canUndo={undoStack.length > 0} canRedo={redoStack.length > 0}
      onAnnotate={enterAnnotating}
      onToolChange={(t) => {
        activeTool = t;
        if (phase === 'selecting') enterAnnotating();
        if (t !== 'select') selectedId = null;
      }}
      onColorChange={(c) => activeColor = c}
      onStrokeWidthChange={(w) => activeStroke = w}
      onFillToggle={() => fillMode = !fillMode}
      onTextBgToggle={() => textBg = !textBg}
      onUndo={undo}
      onRedo={redo}
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
        <div>拖拽选择截图区域 · 双击全屏</div>
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
</div>
