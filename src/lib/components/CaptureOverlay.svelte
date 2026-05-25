<script lang="ts">
  import SizeIndicator from './SizeIndicator.svelte';
  import Toolbar from './Toolbar.svelte';

  // ── Props ──
  interface Props {
    screenshotData: string;
    screenshotWidth: number;
    screenshotHeight: number;
    onselect?: (region: { x: number; y: number; width: number; height: number }) => void;
    oncancel?: () => void;
  }

  let {
    screenshotData,
    screenshotWidth,
    screenshotHeight,
    onselect,
    oncancel,
  }: Props = $props();

  // ── Selection state ──
  let isDragging = $state(false);
  let startX = $state(0);
  let startY = $state(0);
  let currentX = $state(0);
  let currentY = $state(0);
  let selectionConfirmed = $state(false);

  // ── Derived selection rect (normalized so x/y is always top-left) ──
  let rect = $derived.by(() => {
    const x = Math.min(startX, currentX);
    const y = Math.min(startY, currentY);
    const width = Math.abs(currentX - startX);
    const height = Math.abs(currentY - startY);
    return { x, y, width, height };
  });

  let hasSelection = $derived(rect.width > 2 && rect.height > 2);

  // ── The background image URL ──
  let bgImageUrl = $derived(`data:image/png;base64,${screenshotData}`);

  // ── Mouse handlers ──
  function onMouseDown(e: MouseEvent) {
    if (e.button !== 0) return; // left click only
    if (selectionConfirmed) return;

    isDragging = true;
    startX = e.clientX;
    startY = e.clientY;
    currentX = e.clientX;
    currentY = e.clientY;
  }

  function onMouseMove(e: MouseEvent) {
    if (!isDragging) return;
    currentX = e.clientX;
    currentY = e.clientY;
  }

  function onMouseUp(_e: MouseEvent) {
    if (!isDragging) return;
    isDragging = false;

    if (hasSelection) {
      selectionConfirmed = true;
    }
  }

  function onDblClick(_e: MouseEvent) {
    // Double-click = capture full screen
    onselect?.({
      x: 0,
      y: 0,
      width: screenshotWidth,
      height: screenshotHeight,
    });
  }

  function onContextMenu(e: MouseEvent) {
    e.preventDefault();
    oncancel?.();
  }

  function onKeyDown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      if (selectionConfirmed) {
        // Deselect current selection
        selectionConfirmed = false;
        startX = 0;
        startY = 0;
        currentX = 0;
        currentY = 0;
      } else {
        oncancel?.();
      }
    }

    if (e.key === 'Enter' && selectionConfirmed && hasSelection) {
      confirmSelection();
    }
  }

  function confirmSelection() {
    onselect?.({
      x: Math.round(rect.x),
      y: Math.round(rect.y),
      width: Math.round(rect.width),
      height: Math.round(rect.height),
    });
  }
</script>

<svelte:window onkeydown={onKeyDown} />

<div
  class="fixed inset-0 capture-cursor"
  role="dialog"
  aria-label="Screenshot region selector"
  onmousedown={onMouseDown}
  onmousemove={onMouseMove}
  onmouseup={onMouseUp}
  ondblclick={onDblClick}
  oncontextmenu={onContextMenu}
>
  <!--
    Layered rendering:
    1. Screenshot image fills the whole screen
    2. Semi-transparent dark overlay on top (masks everything)
    3. The selected region "punches through" the overlay to reveal the screenshot beneath
  -->

  <!-- 1. Full screenshot image underneath -->
  <img
    src={bgImageUrl}
    alt=""
    class="absolute inset-0 w-full h-full object-fill pointer-events-none"
    draggable={false}
  />

  <!-- 2. Semi-transparent overlay with a rectangular "hole" for the selection -->
  <svg class="absolute inset-0 w-full h-full pointer-events-none">
    <defs>
      <mask id="selection-mask">
        <!-- White = visible overlay, Black = transparent (shows screenshot) -->
        <rect x="0" y="0" width="100%" height="100%" fill="white" />
        {#if hasSelection}
          <rect
            x={rect.x}
            y={rect.y}
            width={rect.width}
            height={rect.height}
            fill="black"
          />
        {/if}
      </mask>
    </defs>
    <!-- Dark overlay masked by selection -->
    <rect
      x="0"
      y="0"
      width="100%"
      height="100%"
      fill="rgba(0,0,0,0.5)"
      mask="url(#selection-mask)"
    />

    <!-- Selection border -->
    {#if hasSelection}
      <rect
        x={rect.x}
        y={rect.y}
        width={rect.width}
        height={rect.height}
        fill="none"
        stroke="rgba(59,130,246,0.8)"
        stroke-width="2"
      />
      <!-- Corner handles -->
      {@const corners = [
        [rect.x, rect.y],
        [rect.x + rect.width, rect.y],
        [rect.x, rect.y + rect.height],
        [rect.x + rect.width, rect.y + rect.height],
      ]}
      {#each corners as [cx, cy]}
        <rect
          x={cx! - 4}
          y={cy! - 4}
          width="8"
          height="8"
          fill="white"
          stroke="rgba(59,130,246,0.8)"
          stroke-width="1"
        />
      {/each}
    {/if}
  </svg>

  <!-- 3. Size indicator (shows width×height near the selection) -->
  {#if hasSelection}
    <SizeIndicator
      x={rect.x}
      y={rect.y}
      width={rect.width}
      height={rect.height}
    />
  {/if}

  <!-- 4. Toolbar (shown when selection is confirmed) -->
  {#if selectionConfirmed && hasSelection}
    <Toolbar
      x={rect.x}
      y={rect.y}
      width={rect.width}
      height={rect.height}
      onconfirm={confirmSelection}
      oncancel={() => {
        selectionConfirmed = false;
        startX = 0;
        startY = 0;
        currentX = 0;
        currentY = 0;
      }}
    />
  {/if}

  <!-- Help text (shown when no selection) -->
  {#if !hasSelection && !isDragging}
    <div class="absolute inset-0 flex items-center justify-center pointer-events-none">
      <div class="bg-black/70 text-white px-6 py-3 rounded-lg text-sm backdrop-blur-sm">
        拖拽选择截图区域 · 双击全屏截图 · 右键或 ESC 取消
      </div>
    </div>
  {/if}
</div>
