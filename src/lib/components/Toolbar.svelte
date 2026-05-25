<script lang="ts">
  /**
   * Toolbar — action bar that appears after user confirms a selection.
   * Positioned below the selection rectangle.
   */

  interface Props {
    x: number;
    y: number;
    width: number;
    height: number;
    onconfirm?: () => void;
    oncancel?: () => void;
  }

  let { x, y, width, height, onconfirm, oncancel }: Props = $props();

  // Position toolbar below the selection, centered
  let toolbarX = $derived(x + width / 2);
  let toolbarY = $derived(y + height + 12);
</script>

<div
  class="fixed z-50"
  style="left: {toolbarX}px; top: {toolbarY}px; transform: translateX(-50%);"
>
  <div class="flex items-center gap-2 bg-white rounded-lg shadow-xl px-2 py-1.5">
    <!-- Save to clipboard -->
    <button
      class="flex items-center gap-1.5 px-3 py-1.5 text-sm text-white bg-blue-500 rounded-md hover:bg-blue-600 transition-colors"
      onclick={() => onconfirm?.()}
    >
      <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
          d="M9 5H7a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2V7a2 2 0 00-2-2h-2M9 5a2 2 0 002 2h2a2 2 0 002-2M9 5a2 2 0 012-2h2a2 2 0 012 2" />
      </svg>
      复制
    </button>

    <!-- Cancel -->
    <button
      class="flex items-center gap-1.5 px-3 py-1.5 text-sm text-gray-700 bg-gray-100 rounded-md hover:bg-gray-200 transition-colors"
      onclick={() => oncancel?.()}
    >
      <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
          d="M6 18L18 6M6 6l12 12" />
      </svg>
      取消
    </button>
  </div>
</div>
