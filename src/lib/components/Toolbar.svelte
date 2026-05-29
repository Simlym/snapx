<script lang="ts">
  export type ToolType =
    | 'select' | 'rect' | 'ellipse' | 'arrow' | 'line'
    | 'text' | 'pen' | 'mosaic' | 'highlight' | 'counter' | 'eraser';

  interface Props {
    selX: number; selY: number; selW: number; selH: number;
    phase: 'selecting' | 'annotating';
    activeTool: ToolType;
    color: string;
    strokeWidth: number;
    fillMode: boolean;
    textBg: boolean;
    canUndo: boolean;
    canRedo: boolean;
    onAnnotate?: () => void;
    onToolChange?: (t: ToolType) => void;
    onColorChange?: (c: string) => void;
    onStrokeWidthChange?: (w: number) => void;
    onFillToggle?: () => void;
    onTextBgToggle?: () => void;
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
    phase, activeTool, color, strokeWidth, fillMode, textBg, canUndo, canRedo,
    onAnnotate, onToolChange, onColorChange, onStrokeWidthChange,
    onFillToggle, onTextBgToggle,
    onUndo, onRedo, onOcr, onCopy, onSave, onQuickSave, onPin, onCancel,
  }: Props = $props();

  const PRESET_COLORS = [
    '#ef4444', '#f97316', '#eab308', '#22c55e',
    '#3b82f6', '#8b5cf6', '#ec4899', '#ffffff', '#000000',
  ];

  const STROKE_WIDTHS = [
    { label: '细', value: 2 },
    { label: '中', value: 4 },
    { label: '粗', value: 7 },
  ];

  const TOOLS: { type: ToolType; label: string; title: string }[] = [
    { type: 'select',    label: '↖',  title: '选择/移动 (S)' },
    { type: 'rect',      label: '▭',  title: '矩形 (R)' },
    { type: 'ellipse',   label: '◯',  title: '椭圆 (E)' },
    { type: 'arrow',     label: '↗',  title: '箭头 (A)' },
    { type: 'line',      label: '╱',  title: '直线 (L)' },
    { type: 'text',      label: 'T',  title: '文字 (T)' },
    { type: 'pen',       label: '✏', title: '画笔 (P)' },
    { type: 'mosaic',    label: '▒',  title: '马赛克 (M)' },
    { type: 'highlight', label: '▨',  title: '高亮 (H)' },
    { type: 'counter',   label: '①',  title: '序号 (N)' },
    { type: 'eraser',    label: '⌫',  title: '橡皮擦 (X)' },
  ];

  // Show stroke/color controls for these tools
  const HAS_STYLE = new Set<ToolType>(['rect','ellipse','arrow','line','text','pen','mosaic','highlight','counter']);
  // Show fill toggle for these tools
  const HAS_FILL = new Set<ToolType>(['rect', 'ellipse']);

  let showColorPicker = $state(false);

  let toolbarStyle = $derived.by(() => {
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
  class="fixed z-50 select-none"
  style={toolbarStyle}
  onmousedown={stopProp}
  onmouseup={stopProp}
>
  <div class="flex items-center gap-0.5 bg-[#1e1e1e] rounded-xl shadow-2xl px-2 py-1.5 border border-white/10">

    {#if phase === 'annotating'}
      <!-- ── Tools ── -->
      {#each TOOLS as tool}
        <button
          class="w-7 h-7 rounded-lg text-xs font-bold transition-all
            {activeTool === tool.type
              ? 'bg-blue-500 text-white shadow-inner'
              : 'text-gray-300 hover:bg-white/10'}"
          title={tool.title}
          onclick={() => onToolChange?.(tool.type)}
        >{tool.label}</button>
      {/each}

      <div class="w-px h-6 bg-white/15 mx-0.5"></div>

      <!-- ── Style controls (color, stroke, fill) ── -->
      {#if HAS_STYLE.has(activeTool)}
        <!-- Color picker -->
        <div class="relative">
          <button
            class="w-6 h-6 rounded-full border-2 border-white/40 shadow-inner transition-transform hover:scale-110"
            style="background:{color}"
            title="颜色"
            onclick={() => showColorPicker = !showColorPicker}
          ></button>
          {#if showColorPicker}
            <!-- svelte-ignore a11y_no_static_element_interactions -->
            <div
              class="absolute bottom-9 left-1/2 -translate-x-1/2 bg-[#2a2a2a] rounded-xl p-2.5 shadow-2xl border border-white/10 z-50"
              onmousedown={stopProp}
            >
              <div class="grid grid-cols-5 gap-1.5 mb-2">
                {#each PRESET_COLORS as c}
                  <button
                    class="w-6 h-6 rounded-full border-2 transition-transform hover:scale-125
                      {c === color ? 'border-white scale-110' : 'border-transparent'}"
                    style="background:{c}"
                    aria-label="选择颜色 {c}"
                    onclick={() => { onColorChange?.(c); showColorPicker = false; }}
                  ></button>
                {/each}
              </div>
              <input
                type="color" value={color}
                class="w-full h-7 rounded cursor-pointer border-0"
                title="自定义颜色"
                oninput={(e) => onColorChange?.((e.target as HTMLInputElement).value)}
              />
            </div>
          {/if}
        </div>

        <!-- Stroke width (not for mosaic/highlight/counter) -->
        {#if activeTool !== 'mosaic' && activeTool !== 'highlight'}
          <div class="flex items-center gap-0.5">
            {#each STROKE_WIDTHS as sw}
              <button
                class="w-6 h-6 flex items-center justify-center rounded text-xs transition-all
                  {strokeWidth === sw.value ? 'bg-blue-500/80 text-white' : 'text-gray-400 hover:bg-white/10'}"
                title="线宽: {sw.label}"
                onclick={() => onStrokeWidthChange?.(sw.value)}
              >
                <div class="rounded-full bg-current" style="width:{sw.value + 1}px;height:{sw.value + 1}px"></div>
              </button>
            {/each}
          </div>
        {/if}

        <!-- Fill toggle (rect / ellipse) -->
        {#if HAS_FILL.has(activeTool)}
          <button
            class="w-7 h-7 rounded-lg text-xs font-bold transition-all
              {fillMode ? 'bg-blue-500/80 text-white' : 'text-gray-400 hover:bg-white/10'}"
            title={fillMode ? '切换为空心' : '切换为实心'}
            onclick={onFillToggle}
          >
            {fillMode ? '■' : '□'}
          </button>
        {/if}

        <!-- Text background toggle -->
        {#if activeTool === 'text'}
          <button
            class="w-7 h-7 rounded-lg text-xs font-bold transition-all
              {textBg ? 'bg-blue-500/80 text-white' : 'text-gray-400 hover:bg-white/10'}"
            title={textBg ? '取消文字背景' : '添加文字背景'}
            onclick={onTextBgToggle}
          >背</button>
        {/if}

        <div class="w-px h-6 bg-white/15 mx-0.5"></div>
      {/if}

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

    {:else}
      <!-- ── Selecting phase: quick annotate entry ── -->
      <button
        class="flex items-center gap-1 px-2 py-1 text-xs text-gray-200 hover:bg-white/10 rounded-lg transition-colors"
        title="进入标注模式"
        onclick={onAnnotate}
      >✏ 标注</button>
      <div class="w-px h-6 bg-white/15 mx-0.5"></div>
    {/if}

    <!-- ── Actions ── -->
    <button
      class="flex items-center gap-1 px-2 py-1 text-xs text-gray-200 hover:bg-white/10 rounded-lg transition-colors font-medium"
      title="提取文字 (OCR)"
      onclick={onOcr}
    >
      <svg class="w-3 h-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
        <path d="M4 7V4h16v3M9 20h6M12 4v16"/>
      </svg>
      取字
    </button>

    <button
      class="flex items-center gap-1 px-2 py-1 text-xs text-white bg-blue-500 hover:bg-blue-600 rounded-lg transition-colors font-medium"
      title="复制到剪贴板 (Enter)"
      onclick={onCopy}
    >
      <svg class="w-3 h-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
        <rect x="9" y="9" width="13" height="13" rx="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/>
      </svg>
      复制
    </button>

    <button
      class="flex items-center gap-1 px-2 py-1 text-xs text-white bg-emerald-500 hover:bg-emerald-600 rounded-lg transition-colors font-medium"
      title="另存为…"
      onclick={onSave}
    >
      <svg class="w-3 h-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
        <path d="M19 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11l5 5v11a2 2 0 0 1-2 2z"/>
        <polyline points="17 21 17 13 7 13 7 21"/><polyline points="7 3 7 8 15 8"/>
      </svg>
      保存
    </button>

    <button
      class="flex items-center gap-1 px-2 py-1 text-xs text-white bg-teal-500 hover:bg-teal-600 rounded-lg transition-colors font-medium"
      title="快速保存到图片文件夹"
      onclick={onQuickSave}
    >
      <svg class="w-3 h-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
        <path d="M12 2v13m0 0l-4-4m4 4l4-4"/><path d="M2 17v2a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2v-2"/>
      </svg>
      快存
    </button>

    <button
      class="flex items-center gap-1 px-2 py-1 text-xs text-white bg-violet-500 hover:bg-violet-600 rounded-lg transition-colors font-medium"
      title="贴图到桌面"
      onclick={onPin}
    >
      <svg class="w-3 h-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
        <line x1="12" y1="17" x2="12" y2="22"/>
        <path d="M5 17h14v-1.76a2 2 0 0 0-1.11-1.79l-1.78-.9A2 2 0 0 1 15 10.76V6h1a2 2 0 0 0 0-4H8a2 2 0 0 0 0 4h1v4.76a2 2 0 0 1-1.11 1.79l-1.78.9A2 2 0 0 0 5 15.24Z"/>
      </svg>
      贴图
    </button>

    <button
      class="w-7 h-7 flex items-center justify-center rounded-lg text-gray-400 hover:text-white hover:bg-white/10 transition-all"
      title="取消 (ESC)"
      onclick={onCancel}
    >✕</button>
  </div>
</div>
