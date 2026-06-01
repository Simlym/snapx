<script lang="ts">
  export type ToolType =
    | 'select' | 'rect' | 'ellipse' | 'arrow' | 'line'
    | 'text' | 'pen' | 'mosaic' | 'highlight' | 'counter' | 'eraser';

  type LineStyle = 'solid' | 'dashed';
  type ArrowStyle = 'end' | 'both' | 'none';

  interface Props {
    selX: number; selY: number; selW: number; selH: number;
    phase: 'selecting' | 'annotating';
    activeTool: ToolType;
    color: string;
    strokeWidth: number;
    fillMode: boolean;
    textBg: boolean;
    lineStyle: LineStyle;
    roundCorners: boolean;
    arrowStyle: ArrowStyle;
    /** Whose traits the style panel reflects: the selected annotation's type if
     *  one is selected, otherwise the active drawing tool. */
    styleType: ToolType;
    /** True when an existing annotation is selected (select tool). Enables the
     *  shape-switch buttons (rect⇄ellipse, arrow⇄line). */
    hasSelection: boolean;
    canUndo: boolean;
    canRedo: boolean;
    onAnnotate?: () => void;
    onToolChange?: (t: ToolType) => void;
    onColorChange?: (c: string) => void;
    onStrokeWidthChange?: (w: number) => void;
    onFillToggle?: () => void;
    onTextBgToggle?: () => void;
    onLineStyleChange?: (s: LineStyle) => void;
    onRoundToggle?: () => void;
    onArrowStyleChange?: (s: ArrowStyle) => void;
    onShapeSwitch?: (t: 'rect' | 'ellipse' | 'arrow' | 'line') => void;
    onUndo?: () => void;
    onRedo?: () => void;
    onOcr?: () => void;
    onCopy?: () => void;
    onSave?: () => void;
    onQuickSave?: () => void;
    onPin?: () => void;
    onCancel?: () => void;
    ocrActive?: boolean;
  }

  let {
    selX, selY, selW, selH,
    phase, activeTool, color, strokeWidth, fillMode, textBg,
    lineStyle, roundCorners, arrowStyle, styleType, hasSelection, canUndo, canRedo,
    onToolChange, onColorChange, onStrokeWidthChange,
    onFillToggle, onTextBgToggle, onLineStyleChange, onRoundToggle, onArrowStyleChange,
    onShapeSwitch,
    onUndo, onRedo, onOcr, onCopy, onSave, onQuickSave, onPin, onCancel,
    ocrActive = false,
  }: Props = $props();

  // A small set of the most-used colours; a free colour-picker sits beside them.
  const PRESET_COLORS = [
    '#ef4444', '#f97316', '#eab308', '#22c55e', '#3b82f6', '#ffffff', '#000000',
  ];

  interface ToolDef { type: ToolType; title: string }

  // Standalone buttons (always visible, no dropdown). Icons render via the
  // {#snippet icon} below, keyed by `type` — see the markup.
  const SELECT_TOOL: ToolDef = { type: 'select', title: '选择/移动 (S)' };
  const ERASER_TOOL: ToolDef = { type: 'eraser', title: '橡皮擦 (X)' };

  // Drawing tools grouped so the bar stays short; the group's current tool
  // shows on the bar, siblings switch from the style panel's tool row.
  const GROUPS: { name: string; tools: ToolDef[] }[] = [
    { name: '形状', tools: [
      { type: 'rect',    title: '矩形 (R)' },
      { type: 'ellipse', title: '椭圆 (E)' },
      { type: 'arrow',   title: '箭头 (A)' },
      { type: 'line',    title: '直线 (L)' },
    ]},
    { name: '标记', tools: [
      { type: 'text',      title: '文字 (T)' },
      { type: 'pen',       title: '画笔 (P)' },
      { type: 'counter',   title: '序号 (N)' },
      { type: 'highlight', title: '高亮 (H)' },
    ]},
    { name: '遮掩', tools: [
      { type: 'mosaic', title: '马赛克 (M)' },
    ]},
  ];

  // Quick stroke-width presets (the slider beside them stays for fine-tuning).
  const STROKE_PRESETS = [
    { w: 2,  dot: 4,  label: '细' },
    { w: 6,  dot: 8,  label: '中' },
    { w: 12, dot: 13, label: '粗' },
  ];

  // Which traits each tool exposes in its style panel.
  const HAS_STYLE  = new Set<ToolType>(['rect','ellipse','arrow','line','text','pen','mosaic','highlight','counter']);
  const HAS_FILL   = new Set<ToolType>(['rect', 'ellipse']);
  const HAS_DASH   = new Set<ToolType>(['rect', 'ellipse', 'arrow', 'line']);
  // Tools whose stroke width is meaningful (mosaic/highlight ignore it).
  const HAS_STROKE = new Set<ToolType>(['rect','ellipse','arrow','line','text','pen','counter']);

  function groupFace(tools: ToolDef[]): ToolDef {
    return tools.find(d => d.type === activeTool) ?? tools[0];
  }
  function groupActive(tools: ToolDef[]): boolean {
    return tools.some(d => d.type === activeTool);
  }
  // The group a tool belongs to (undefined for the standalone select/eraser).
  function groupOf(t: ToolType) {
    return GROUPS.find(g => g.tools.some(d => d.type === t));
  }
  // Two shapes can "morph" in place (carrying geometry/colour) when they share
  // a geometry family: box-based (rect⇄ellipse) or segment-based (arrow⇄line).
  function morphCompatible(a: ToolType, b: ToolType): boolean {
    const box = (x: ToolType) => x === 'rect' || x === 'ellipse';
    const seg = (x: ToolType) => x === 'arrow' || x === 'line';
    return (box(a) && box(b)) || (seg(a) && seg(b));
  }

  // ── Style panel — anchored under the *active* tool's button ────────
  // The style panel "follows the selection": picking a tool opens it right
  // under that tool's button. We track each tool button's element so the panel
  // can centre itself there regardless of which group the tool lives in.
  let styleOpen = $state(true);            // open by default once a tool is chosen
  let toolEls: Partial<Record<ToolType, HTMLElement>> = {};
  let barEl: HTMLElement | undefined = $state();
  let panelEl: HTMLElement | undefined = $state();
  // Horizontal offset (px) of the panel's centre, relative to the bar's left.
  let panelCenter = $state(0);
  // Extra px to nudge the panel so it stays on-screen (added to translateX).
  let panelShift = $state(0);

  function pickTool(t: ToolType) {
    onToolChange?.(t);
    // Selecting a styleable tool auto-opens its style panel. The select tool
    // keeps it open too — it'll show once an annotation is actually selected
    // (governed by hasSelection in showStyle).
    styleOpen = HAS_STYLE.has(t) || t === 'select';
  }

  // Clicking a sibling in the panel's tool row: morph the *selected* annotation
  // in place when the target shares its geometry family, otherwise just switch
  // the active drawing tool (which clears any selection downstream).
  function switchTo(t: ToolType) {
    if (hasSelection && t !== styleType && morphCompatible(styleType, t)) {
      onShapeSwitch?.(t as 'rect' | 'ellipse' | 'arrow' | 'line');
    } else {
      pickTool(t);
    }
  }
  // Sibling tools shown in the panel's switch row (empty for standalone tools).
  const switchTools = $derived(groupOf(styleType)?.tools ?? []);

  // Recompute the panel anchor whenever the active tool (or layout) changes.
  function updateAnchor() {
    // Anchor under the button that represents what the panel is editing: the
    // selected annotation's shape (styleType) if any, else the active tool.
    // Fall back to the select button so a selection still has an anchor.
    const btn = toolEls[styleType] ?? toolEls[activeTool] ?? toolEls['select'];
    if (!btn || !barEl) return;
    const b = btn.getBoundingClientRect();
    const bar = barEl.getBoundingClientRect();
    panelCenter = b.left - bar.left + b.width / 2;
    // Keep the (centred) panel within the viewport: if its left/right edge would
    // spill off-screen, shift it back by that overflow.
    panelShift = 0;
    if (panelEl) {
      const pw = panelEl.offsetWidth;
      const desiredLeft = bar.left + panelCenter - pw / 2;
      const M = 6;
      if (desiredLeft < M) panelShift = M - desiredLeft;
      else if (desiredLeft + pw > window.innerWidth - M)
        panelShift = (window.innerWidth - M) - (desiredLeft + pw);
    }
  }
  // Re-anchor whenever the edited target or layout changes.
  $effect(() => {
    // touch deps so the effect re-runs when they change
    void activeTool; void styleType; void hasSelection; void styleOpen;
    void switchTools; void phase;
    void selX; void selY; void selW; void selH; void detached;
    requestAnimationFrame(updateAnchor);
  });

  // Show the panel while annotating whenever the *effective target* is a
  // styleable thing: a selected annotation, or a styleable active tool. Using
  // styleType (selected annotation's type, else active tool) means the panel
  // appears even when the select tool is active (activeTool === 'select').
  const showStyle = $derived(
    phase === 'annotating' && styleOpen &&
    (hasSelection || HAS_STYLE.has(activeTool)) && HAS_STYLE.has(styleType)
  );

  // ── Stroke width via scroll inside the style panel ─────────────────
  function strokeWheel(e: WheelEvent) {
    e.preventDefault();
    onStrokeWidthChange?.(Math.max(1, Math.min(20, strokeWidth + (e.deltaY < 0 ? 1 : -1))));
  }

  // ── Dragging the toolbar by its grip ───────────────────────────────
  // Until the user drags, the bar tracks the selection (Snipaste-style).
  // The first drag detaches it to a fixed screen position.
  let detached = $state(false);
  let posX = $state(0);
  let posY = $state(0);
  let dragging = false;
  let dragDX = 0, dragDY = 0;

  function dragStart(e: MouseEvent) {
    e.stopPropagation();
    e.preventDefault();
    const rect = (e.currentTarget as HTMLElement).closest('.snapx-toolbar')?.getBoundingClientRect();
    if (rect) { posX = rect.left; posY = rect.top; }
    detached = true;
    dragging = true;
    dragDX = e.clientX - posX;
    dragDY = e.clientY - posY;
    window.addEventListener('mousemove', dragMove, true);
    window.addEventListener('mouseup', dragEnd, true);
  }
  function dragMove(e: MouseEvent) {
    if (!dragging) return;
    const maxX = window.innerWidth - 40, maxY = window.innerHeight - 40;
    posX = Math.max(4, Math.min(maxX, e.clientX - dragDX));
    posY = Math.max(4, Math.min(maxY, e.clientY - dragDY));
  }
  function dragEnd() {
    dragging = false;
    window.removeEventListener('mousemove', dragMove, true);
    window.removeEventListener('mouseup', dragEnd, true);
  }

  // Position: follow the selection until detached, then use the dragged coords.
  let toolbarStyle = $derived.by(() => {
    if (detached) return `left:${posX}px;top:${posY}px`;
    const GAP = 10;
    const TOOLBAR_H = 44;
    const cx = selX + selW / 2;
    const belowY = selY + selH + GAP;
    const aboveY = selY - TOOLBAR_H - GAP;
    const top = belowY + TOOLBAR_H > (typeof window !== 'undefined' ? window.innerHeight : 1080)
      ? aboveY : belowY;
    return `left:${cx}px;top:${top}px;transform:translateX(-50%)`;
  });

  function stopProp(e: MouseEvent) { e.stopPropagation(); }
</script>

<!-- Tool icons — one consistent SVG set keyed by tool type, replacing the
     glyphs that rendered unevenly across platforms/fonts. -->
{#snippet icon(type: ToolType)}
  <svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor"
    stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
    {#if type === 'select'}
      <path d="M5 3l14 6.6-6 1.7L10.6 18z" fill="currentColor" stroke="none"/>
    {:else if type === 'rect'}
      <rect x="4" y="6" width="16" height="12" rx="1.5"/>
    {:else if type === 'ellipse'}
      <ellipse cx="12" cy="12" rx="9" ry="6.5"/>
    {:else if type === 'arrow'}
      <path d="M5 19L17.5 6.5"/><path d="M11 6.5h7v7"/>
    {:else if type === 'line'}
      <path d="M5 19L19 5"/>
    {:else if type === 'text'}
      <path d="M6 6V5h12v1M12 5v14M9.5 19h5"/>
    {:else if type === 'pen'}
      <path d="M4 20l1.3-4L15 6.3l2.7 2.7L8 18.7z"/><path d="M13.5 7.8l2.7 2.7"/>
    {:else if type === 'counter'}
      <circle cx="12" cy="12" r="8.5"/><path d="M10.8 9.6l1.7-1.2V16M10.5 16h4"/>
    {:else if type === 'highlight'}
      <path d="M5 19.5h6"/><path d="M8.5 16L14 6l4 2.3-5.5 10z"/>
    {:else if type === 'mosaic'}
      <rect x="4" y="4" width="16" height="16" rx="1"/>
      <path d="M4 9.3h16M4 14.6h16M9.3 4v16M14.6 4v16"/>
    {:else if type === 'eraser'}
      <path d="M8.5 20H6l-2.2-2.2a1.8 1.8 0 010-2.6l8-8 6.8 6.8-6 6z"/>
      <path d="M9.5 9.5l6.8 6.8"/><path d="M7 20h12"/>
    {/if}
  </svg>
{/snippet}

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="snapx-toolbar fixed z-50 select-none"
  style={toolbarStyle}
  onmousedown={stopProp}
  onmouseup={stopProp}
>
  <div bind:this={barEl} class="relative flex items-center gap-0.5 bg-[#1e1e1e] rounded-xl shadow-2xl px-1.5 py-1.5 border border-white/10">

    <!-- ── Drag handle ── -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="w-5 h-7 flex items-center justify-center rounded-lg text-gray-500 hover:text-gray-300 hover:bg-white/5 transition-colors cursor-grab active:cursor-grabbing"
      title="拖动工具栏"
      onmousedown={dragStart}
    >
      <svg class="w-3 h-3" viewBox="0 0 24 24" fill="currentColor">
        <circle cx="9" cy="6" r="1.6"/><circle cx="15" cy="6" r="1.6"/>
        <circle cx="9" cy="12" r="1.6"/><circle cx="15" cy="12" r="1.6"/>
        <circle cx="9" cy="18" r="1.6"/><circle cx="15" cy="18" r="1.6"/>
      </svg>
    </div>

    <div class="w-px h-6 bg-white/15 mx-0.5"></div>

    <!-- ── Select tool (standalone) ── -->
    <button
      bind:this={toolEls['select']}
      class="w-7 h-7 flex items-center justify-center rounded-lg transition-all
        {activeTool === 'select'
          ? 'bg-blue-500 text-white shadow-inner'
          : 'text-gray-300 hover:bg-white/10'}"
      title={SELECT_TOOL.title}
      onclick={() => pickTool('select')}
    >{@render icon('select')}</button>

    <!-- ── Grouped drawing tools: one button per group showing its current
         tool. Switching *within* a group happens in the style panel's tool
         row (no separate dropdown). ── -->
    {#each GROUPS as group}
      {@const face = groupFace(group.tools)}
      {@const active = groupActive(group.tools)}
      <button
        bind:this={toolEls[face.type]}
        class="w-7 h-7 flex items-center justify-center rounded-lg transition-all
          {active ? 'bg-blue-500 text-white shadow-inner' : 'text-gray-300 hover:bg-white/10'}"
        title={face.title}
        onclick={() => pickTool(face.type)}
      >{@render icon(face.type)}</button>
    {/each}

    <!-- ── Eraser (standalone) ── -->
    <button
      bind:this={toolEls['eraser']}
      class="w-7 h-7 flex items-center justify-center rounded-lg transition-all
        {activeTool === 'eraser'
          ? 'bg-blue-500 text-white shadow-inner'
          : 'text-gray-300 hover:bg-white/10'}"
      title={ERASER_TOOL.title}
      onclick={() => pickTool('eraser')}
    >{@render icon('eraser')}</button>

    <div class="w-px h-6 bg-white/15 mx-0.5"></div>

    <!-- ── Undo / Redo ── -->
    <button
      class="w-7 h-7 flex items-center justify-center rounded-lg transition-all
        {canUndo ? 'text-gray-200 hover:bg-white/10' : 'text-gray-600 cursor-not-allowed'}"
      title="撤销 (Ctrl+Z)"
      onclick={onUndo}
      disabled={!canUndo}
    >
      <svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round">
        <path d="M9 14L4 9l5-5"/><path d="M4 9h10a6 6 0 010 12h-3"/>
      </svg>
    </button>
    <button
      class="w-7 h-7 flex items-center justify-center rounded-lg transition-all
        {canRedo ? 'text-gray-200 hover:bg-white/10' : 'text-gray-600 cursor-not-allowed'}"
      title="重做 (Ctrl+Y)"
      onclick={onRedo}
      disabled={!canRedo}
    >
      <svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round">
        <path d="M15 14l5-5-5-5"/><path d="M20 9H10a6 6 0 000 12h3"/>
      </svg>
    </button>

    <div class="w-px h-6 bg-white/15 mx-0.5"></div>

    <!-- ── Actions: uniform icon buttons (colour-coded) + tooltips ── -->
    <button
      class="w-8 h-8 flex items-center justify-center rounded-lg transition-colors {ocrActive ? 'text-white' : 'text-gray-200 hover:bg-white/10'}"
      style={ocrActive ? 'background: var(--accent);' : ''}
      title="提取文字 (OCR)"
      aria-label="提取文字"
      onclick={onOcr}
    >
      <svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2">
        <path d="M4 7V4h16v3M9 20h6M12 4v16"/>
      </svg>
    </button>

    <button
      class="w-8 h-8 flex items-center justify-center rounded-lg text-white bg-blue-500 hover:bg-blue-600 transition-colors"
      title="复制到剪贴板 (Enter)"
      aria-label="复制"
      onclick={onCopy}
    >
      <svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2">
        <rect x="9" y="9" width="13" height="13" rx="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/>
      </svg>
    </button>

    <button
      class="w-8 h-8 flex items-center justify-center rounded-lg text-white bg-emerald-500 hover:bg-emerald-600 transition-colors"
      title="另存为…"
      aria-label="保存"
      onclick={onSave}
    >
      <svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2">
        <path d="M19 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11l5 5v11a2 2 0 0 1-2 2z"/>
        <polyline points="17 21 17 13 7 13 7 21"/><polyline points="7 3 7 8 15 8"/>
      </svg>
    </button>

    <button
      class="w-8 h-8 flex items-center justify-center rounded-lg text-white bg-teal-500 hover:bg-teal-600 transition-colors"
      title="快速保存到图片文件夹"
      aria-label="快存"
      onclick={onQuickSave}
    >
      <svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2">
        <path d="M12 2v13m0 0l-4-4m4 4l4-4"/><path d="M2 17v2a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2v-2"/>
      </svg>
    </button>

    <button
      class="w-8 h-8 flex items-center justify-center rounded-lg text-white bg-violet-500 hover:bg-violet-600 transition-colors"
      title="贴图到桌面"
      aria-label="贴图"
      onclick={onPin}
    >
      <svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2">
        <line x1="12" y1="17" x2="12" y2="22"/>
        <path d="M5 17h14v-1.76a2 2 0 0 0-1.11-1.79l-1.78-.9A2 2 0 0 1 15 10.76V6h1a2 2 0 0 0 0-4H8a2 2 0 0 0 0 4h1v4.76a2 2 0 0 1-1.11 1.79l-1.78.9A2 2 0 0 0 5 15.24Z"/>
      </svg>
    </button>

    <div class="w-px h-6 bg-white/15 mx-0.5"></div>

    <button
      class="w-8 h-8 flex items-center justify-center rounded-lg text-gray-400 hover:text-white hover:bg-red-500/80 transition-all"
      title="取消 (ESC)"
      aria-label="取消"
      onclick={onCancel}
    >
      <svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4">
        <path d="M18 6 6 18M6 6l12 12"/>
      </svg>
    </button>

    <!-- ── Style panel: anchored UNDER the active tool, follows the selection ── -->
    {#if showStyle}
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div
        bind:this={panelEl}
        class="absolute top-full mt-2 bg-[#2a2a2a] rounded-xl px-2 py-1.5 shadow-2xl border border-white/10 z-50 flex items-center gap-2 whitespace-nowrap"
        style="left:{panelCenter}px; transform: translateX(calc(-50% + {panelShift}px)); animation: snapx-pop-in 0.14s var(--ease) both;"
        onmousedown={stopProp}
        onmouseup={stopProp}
      >
        <!-- Little pointer up toward the active tool button (counter the clamp shift). -->
        <div class="absolute -top-1.5 left-1/2 w-3 h-3 rotate-45 bg-[#2a2a2a] border-l border-t border-white/10"
          style="transform: translateX(calc(-50% - {panelShift}px)) rotate(45deg);"></div>

        <!-- All trait controls key off `styleType` (the SELECTED annotation's
             type if any, else the active tool) so the panel edits whatever is
             under focus. Shape-switch first, then size, line, arrow, round,
             fill, text-bg; COLOUR last (least-frequently changed). -->

        <!-- Tool/shape switch row: pick any sibling of the active group right
             here. With a selection, a geometry-compatible target morphs the
             drawn shape in place; otherwise it switches the active draw tool. -->
        {#if switchTools.length > 1}
          <div class="flex items-center gap-1">
            {#each switchTools as t}
              <button
                class="w-7 h-7 flex items-center justify-center rounded-lg transition-all
                  {styleType === t.type ? 'bg-blue-500/80 text-white' : 'text-gray-400 hover:bg-white/10'}"
                title={t.title}
                onclick={() => switchTo(t.type)}
              >{@render icon(t.type)}</button>
            {/each}
          </div>
          <div class="w-px h-6 bg-white/15"></div>
        {/if}

        <!-- Brush / stroke size — quick presets (细/中/粗), then a slider for
             fine-tuning. Scroll anywhere over this area also nudges the size. -->
        {#if HAS_STROKE.has(styleType)}
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div class="flex items-center gap-1.5" onwheel={strokeWheel} title="画笔大小(滚轮缩放)">
            <div class="flex items-center gap-1">
              {#each STROKE_PRESETS as p}
                <button
                  class="w-7 h-7 flex items-center justify-center rounded-lg transition-all
                    {strokeWidth === p.w ? 'bg-blue-500/80 text-white' : 'text-gray-300 hover:bg-white/10'}"
                  title="{p.label} · {p.w}px"
                  onclick={() => onStrokeWidthChange?.(p.w)}
                >
                  <span class="rounded-full bg-current" style="width:{p.dot}px;height:{p.dot}px"></span>
                </button>
              {/each}
            </div>
            <input
              type="range" min="1" max="20" value={strokeWidth}
              class="snapx-range w-16"
              oninput={(e) => onStrokeWidthChange?.(+(e.target as HTMLInputElement).value)}
            />
            <span class="text-[10px] text-gray-300 font-mono tabular-nums w-7 text-right">{strokeWidth}px</span>
          </div>
        {/if}

        <!-- Line style: solid / dashed (shapes & lines) -->
        {#if HAS_DASH.has(styleType)}
          {#if HAS_STROKE.has(styleType)}<div class="w-px h-6 bg-white/15"></div>{/if}
          <div class="flex items-center gap-1">
            <button
              class="w-9 h-7 flex items-center justify-center rounded-lg transition-all
                {lineStyle === 'solid' ? 'bg-blue-500/80 text-white' : 'text-gray-400 hover:bg-white/10'}"
              title="实线"
              onclick={() => onLineStyleChange?.('solid')}
            >
              <svg width="26" height="8" viewBox="0 0 26 8"><line x1="2" y1="4" x2="24" y2="4" stroke="currentColor" stroke-width="2.5"/></svg>
            </button>
            <button
              class="w-9 h-7 flex items-center justify-center rounded-lg transition-all
                {lineStyle === 'dashed' ? 'bg-blue-500/80 text-white' : 'text-gray-400 hover:bg-white/10'}"
              title="虚线"
              onclick={() => onLineStyleChange?.('dashed')}
            >
              <svg width="26" height="8" viewBox="0 0 26 8"><line x1="2" y1="4" x2="24" y2="4" stroke="currentColor" stroke-width="2.5" stroke-dasharray="5 3"/></svg>
            </button>
          </div>
        {/if}

        <!-- Arrow style: end / both / none -->
        {#if styleType === 'arrow'}
          <div class="w-px h-6 bg-white/15"></div>
          <div class="flex items-center gap-1">
            <button
              class="w-7 h-7 flex items-center justify-center rounded-lg transition-all
                {arrowStyle === 'end' ? 'bg-blue-500/80 text-white' : 'text-gray-400 hover:bg-white/10'}"
              title="单向箭头"
              onclick={() => onArrowStyleChange?.('end')}
            >
              <svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M4 12h14M13 7l5 5-5 5"/></svg>
            </button>
            <button
              class="w-7 h-7 flex items-center justify-center rounded-lg transition-all
                {arrowStyle === 'both' ? 'bg-blue-500/80 text-white' : 'text-gray-400 hover:bg-white/10'}"
              title="双向箭头"
              onclick={() => onArrowStyleChange?.('both')}
            >
              <svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M3 12h18M8 7l-5 5 5 5M16 7l5 5-5 5"/></svg>
            </button>
            <button
              class="w-7 h-7 flex items-center justify-center rounded-lg transition-all
                {arrowStyle === 'none' ? 'bg-blue-500/80 text-white' : 'text-gray-400 hover:bg-white/10'}"
              title="无箭头"
              onclick={() => onArrowStyleChange?.('none')}
            >
              <svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="M4 12h16"/></svg>
            </button>
          </div>
        {/if}

        <!-- Rounded corners (rect only) -->
        {#if styleType === 'rect'}
          <div class="w-px h-6 bg-white/15"></div>
          <button
            class="w-7 h-7 flex items-center justify-center rounded-lg transition-all
              {roundCorners ? 'bg-blue-500/80 text-white' : 'text-gray-300 hover:bg-white/10'}"
            title={roundCorners ? '圆角' : '直角'}
            onclick={onRoundToggle}
          >
            <svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <rect x="4" y="4" width="16" height="16" rx={roundCorners ? 5 : 0.5}/>
            </svg>
          </button>
        {/if}

        <!-- Fill toggle (rect / ellipse) -->
        {#if HAS_FILL.has(styleType)}
          <div class="w-px h-6 bg-white/15"></div>
          <button
            class="w-7 h-7 flex items-center justify-center rounded-lg transition-all
              {fillMode ? 'bg-blue-500/80 text-white' : 'text-gray-300 hover:bg-white/10'}"
            title={fillMode ? '实心' : '空心'}
            onclick={onFillToggle}
          >
            <svg class="w-4 h-4" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><rect x="5" y="5" width="14" height="14" rx="1.5" fill={fillMode ? 'currentColor' : 'none'}/></svg>
          </button>
        {/if}

        <!-- Text background toggle -->
        {#if styleType === 'text'}
          <div class="w-px h-6 bg-white/15"></div>
          <button
            class="h-7 px-2 flex items-center justify-center rounded-lg text-xs font-medium transition-all
              {textBg ? 'bg-blue-500/80 text-white' : 'text-gray-300 hover:bg-white/10'}"
            title="文字背景"
            onclick={onTextBgToggle}
          >背景</button>
        {/if}

        <!-- Colours LAST: a few common swatches + free picker, laid out in a row.
             Divider only when some control precedes colour (mosaic/highlight
             expose colour only, so no leading divider for them). -->
        {#if HAS_STROKE.has(styleType) || HAS_DASH.has(styleType) || HAS_FILL.has(styleType) || styleType === 'text'}
          <div class="w-px h-6 bg-white/15"></div>
        {/if}
        <div class="flex items-center gap-1">
          {#each PRESET_COLORS as c}
            <button
              class="w-6 h-6 rounded-full border-2 transition-transform hover:scale-110
                {c === color ? 'border-white scale-110' : 'border-white/15'}"
              style="background:{c}"
              aria-label="选择颜色 {c}"
              onclick={() => onColorChange?.(c)}
            ></button>
          {/each}
          <label
            class="relative w-6 h-6 rounded-full cursor-pointer overflow-hidden border-2 border-white/30 flex items-center justify-center shrink-0"
            title="自定义颜色"
            style="background: conic-gradient(red, yellow, lime, aqua, blue, magenta, red)"
          >
            <input
              type="color" value={color}
              class="absolute inset-0 opacity-0 cursor-pointer"
              oninput={(e) => onColorChange?.((e.target as HTMLInputElement).value)}
            />
          </label>
        </div>
      </div>
    {/if}
  </div>
</div>

<style>
  /* Compact range slider matching the dark glass theme. */
  .snapx-range {
    -webkit-appearance: none;
    appearance: none;
    height: 4px;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.18);
    outline: none;
  }
  .snapx-range::-webkit-slider-thumb {
    -webkit-appearance: none;
    appearance: none;
    width: 14px;
    height: 14px;
    border-radius: 50%;
    background: var(--accent, #3b82f6);
    cursor: pointer;
    border: 2px solid #fff;
  }
  .snapx-range::-moz-range-thumb {
    width: 14px;
    height: 14px;
    border-radius: 50%;
    background: var(--accent, #3b82f6);
    cursor: pointer;
    border: 2px solid #fff;
  }
</style>
