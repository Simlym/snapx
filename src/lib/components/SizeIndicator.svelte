<script lang="ts">
  /**
   * SizeIndicator — floating label that shows the selection dimensions.
   * Positioned just above the selection rectangle.
   */

  interface Props {
    x: number;
    y: number;
    width: number;
    height: number;
  }

  let { x, y, width, height }: Props = $props();

  // Position the indicator above the selection, centered horizontally
  let labelX = $derived(x + width / 2);
  let labelY = $derived(y > 30 ? y - 12 : y + height + 24);
</script>

<div
  class="fixed pointer-events-none z-50"
  style="left: {labelX}px; top: {labelY}px; transform: translateX(-50%);"
>
  <div class="bg-black/75 text-white text-xs px-2 py-1 rounded whitespace-nowrap backdrop-blur-sm">
    {Math.round(width)} × {Math.round(height)}
  </div>
</div>
