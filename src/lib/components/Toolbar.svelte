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
  }

  let {
    selX, selY, selW, selH,
    phase, activeTool, color, strokeWidth, fillMode, textBg,
    lineStyle, roundCorners, arrowStyle, styleType, hasSelection, canUndo, canRedo,
    onToolChange, onColorChange, onStrokeWidthChange,
    onFillToggle, onTextBgToggle, onLineStyleChange, onRoundToggle, onArrowStyleChange,
    onShapeSwitch,
    onUndo, onRedo, onOcr, onCopy, onSave, onQuickSave, onPin, onCancel,
  }: Props = $props();

  // A small set of the most-used colours; a free colour-picker sits beside them.
  const PRESET_COLORS = [
    '#ef4444', '#f97316', '#eab308', '#22c55e', '#3b82f6', '#ffffff', '#000000',
  ];

  interface ToolDef { type: ToolType; label: string; title: string }

  // Standalone buttons (always visible, no dropdown).
  const SELECT_TOOL: ToolDef = { type: 'select', label: '↖', title: '选择/移动 (S)' };
  const ERASER_TOOL: ToolDef = { type: 'eraser', label: '⌫', title: '橡皮擦 (X)' };

  // Drawing tools grouped into dropdowns to keep the bar short.
  const GROUPS: { name: string; tools: ToolDef[] }[] = [
    { name: '形状', tools: [
      { type: 'rect',    label: '▭', title: '矩形 (R)' },
      { type: 'ellipse', label: '◯', title: '椭圆 (E)' },
      { type: 'arrow',   label: '↗', title: '箭头 (A)' },
      { type: 'line',    label: '╱', title: '直线 (L)' },
    ]},
    { name: '标记', tools: [
      { type: 'text',      label: 'T',  title: '文字 (T)' },
      { type: 'pen',       label: '✏', title: '画笔 (P)' },
      { type: 'counter',   label: '①',  title: '序号 (N)' },
      { type: 'highlight', label: '▨',  title: '高亮 (H)' },
    ]},
    { name: '遮掩', tools: [
      { type: 'mosaic', label: '▒', title: '马赛克 (M)' },
    ]},
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

  // ── Tool group switch menu (▾) ─────────────────────────────────────
  let openGroup = $state<string | null>(null);
  function toggleGroup(name: string) { openGroup = openGroup === name ? null : name; }

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
    openGroup = null;
    // Selecting a styleable tool auto-opens its style panel. The select tool
    // keeps it open too — it'll show once an annotation is actually selected
    // (governed by hasSelection in showStyle).
    styleOpen = HAS_STYLE.has(t) || t === 'select';
  }

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
    void openGroup; void phase;
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
      class="w-7 h-7 rounded-lg text-xs font-bold transition-all
        {activeTool === 'select'
          ? 'bg-blue-500 text-white shadow-inner'
          : 'text-gray-300 hover:bg-white/10'}"
      title={SELECT_TOOL.title}
      onclick={() => pickTool('select')}
    >{SELECT_TOOL.label}</button>

    <!-- ── Grouped drawing tools: main icon + ▾ switch ── -->
    {#each GROUPS as group}
      {@const face = groupFace(group.tools)}
      {@const active = groupActive(group.tools)}
      <div class="relative flex items-stretch">
        <!-- Main icon: directly activates the group's current tool. -->
        <button
          bind:this={toolEls[face.type]}
          class="w-7 h-7 flex items-center justify-center text-xs font-bold transition-all
            {group.tools.length > 1 ? 'rounded-l-lg' : 'rounded-lg'}
            {active ? 'bg-blue-500 text-white shadow-inner' : 'text-gray-300 hover:bg-white/10'}"
          title={face.title}
          onclick={() => pickTool(face.type)}
        >{face.label}</button>
        <!-- Switch caret: opens the group's tool menu (multi-tool groups only). -->
        {#if group.tools.length > 1}
          <button
            class="w-4 h-7 flex items-center justify-center rounded-r-lg text-[9px] transition-all border-l border-black/20
              {active ? 'bg-blue-500/80 text-white hover:bg-blue-500' : 'text-gray-400 hover:bg-white/10'}
              {openGroup === group.name ? 'bg-white/15' : ''}"
            title="切换{group.name}"
            onclick={() => toggleGroup(group.name)}
          >▾</button>
        {/if}
        {#if openGroup === group.name}
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div
            class="absolute bottom-9 left-1/2 -translate-x-1/2 flex gap-0.5 bg-[#2a2a2a] rounded-xl p-1.5 shadow-2xl border border-white/10 z-50"
            onmousedown={stopProp}
          >
            {#each group.tools as tool}
              <button
                class="w-8 h-8 rounded-lg text-sm font-bold transition-all
                  {activeTool === tool.type ? 'bg-blue-500 text-white' : 'text-gray-300 hover:bg-white/10'}"
                title={tool.title}
                onclick={() => pickTool(tool.type)}
              >{tool.label}</button>
            {/each}
          </div>
        {/if}
      </div>
    {/each}

    <!-- ── Eraser (standalone) ── -->
    <button
      bind:this={toolEls['eraser']}
      class="w-7 h-7 rounded-lg text-xs font-bold transition-all
        {activeTool === 'eraser'
          ? 'bg-blue-500 text-white shadow-inner'
          : 'text-gray-300 hover:bg-white/10'}"
      title={ERASER_TOOL.title}
      onclick={() => pickTool('eraser')}
    >{ERASER_TOOL.label}</button>

    <div class="w-px h-6 bg-white/15 mx-0.5"></div>

    <!-- ── Undo / Redo ── -->
    <button
      class="w-7 h-7 rounded-lg text-sm transition-all
        {canUndo ? 'text-gray-200 hover:bg-white/10' : 'text-gray-600 cursor-not-allowed'}"
      title="撤销 (Ctrl+Z)"
      onclick={onUndo}
      disabled={!canUndo}
    >↩</button>
    <button
      class="w-7 h-7 rounded-lg text-sm transition-all
        {canRedo ? 'text-gray-200 hover:bg-white/10' : 'text-gray-600 cursor-not-allowed'}"
      title="重做 (Ctrl+Y)"
      onclick={onRedo}
      disabled={!canRedo}
    >↪</button>

    <div class="w-px h-6 bg-white/15 mx-0.5"></div>

    <!-- ── Actions: uniform icon buttons (colour-coded) + tooltips ── -->
    <button
      class="w-8 h-8 flex items-center justify-center rounded-lg text-gray-200 hover:bg-white/10 transition-colors"
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

        <!-- Shape switch (only for a SELECTED annotation): rect⇄ellipse or
             arrow⇄line — change a drawn shape's type in place. -->
        {#if hasSelection && (styleType === 'rect' || styleType === 'ellipse')}
          <div class="flex items-center gap-1">
            <button
              class="w-7 h-7 flex items-center justify-center rounded-lg text-sm font-bold transition-all
                {styleType === 'rect' ? 'bg-blue-500/80 text-white' : 'text-gray-400 hover:bg-white/10'}"
              title="矩形"
              onclick={() => onShapeSwitch?.('rect')}
            >▭</button>
            <button
              class="w-7 h-7 flex items-center justify-center rounded-lg text-sm font-bold transition-all
                {styleType === 'ellipse' ? 'bg-blue-500/80 text-white' : 'text-gray-400 hover:bg-white/10'}"
              title="椭圆"
              onclick={() => onShapeSwitch?.('ellipse')}
            >◯</button>
          </div>
          <div class="w-px h-6 bg-white/15"></div>
        {:else if hasSelection && (styleType === 'arrow' || styleType === 'line')}
          <div class="flex items-center gap-1">
            <button
              class="w-7 h-7 flex items-center justify-center rounded-lg text-sm font-bold transition-all
                {styleType === 'arrow' ? 'bg-blue-500/80 text-white' : 'text-gray-400 hover:bg-white/10'}"
              title="箭头"
              onclick={() => onShapeSwitch?.('arrow')}
            >↗</button>
            <button
              class="w-7 h-7 flex items-center justify-center rounded-lg text-sm font-bold transition-all
                {styleType === 'line' ? 'bg-blue-500/80 text-white' : 'text-gray-400 hover:bg-white/10'}"
              title="直线"
              onclick={() => onShapeSwitch?.('line')}
            >╱</button>
          </div>
          <div class="w-px h-6 bg-white/15"></div>
        {/if}

        <!-- Brush / stroke size — scroll over this area or drag the slider. -->
        {#if HAS_STROKE.has(styleType)}
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div class="flex items-center gap-1.5" onwheel={strokeWheel} title="画笔大小(滚轮缩放)">
            <input
              type="range" min="1" max="20" value={strokeWidth}
              class="snapx-range w-20"
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
            >→</button>
            <button
              class="w-7 h-7 flex items-center justify-center rounded-lg transition-all
                {arrowStyle === 'both' ? 'bg-blue-500/80 text-white' : 'text-gray-400 hover:bg-white/10'}"
              title="双向箭头"
              onclick={() => onArrowStyleChange?.('both')}
            >↔</button>
            <button
              class="w-7 h-7 flex items-center justify-center rounded-lg transition-all
                {arrowStyle === 'none' ? 'bg-blue-500/80 text-white' : 'text-gray-400 hover:bg-white/10'}"
              title="无箭头"
              onclick={() => onArrowStyleChange?.('none')}
            >—</button>
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
            class="w-7 h-7 flex items-center justify-center rounded-lg text-sm transition-all
              {fillMode ? 'bg-blue-500/80 text-white' : 'text-gray-300 hover:bg-white/10'}"
            title={fillMode ? '实心' : '空心'}
            onclick={onFillToggle}
          >{fillMode ? '■' : '□'}</button>
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
