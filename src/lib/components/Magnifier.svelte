<script lang="ts">
  interface Props {
    screenshotSrc: string;
    mouseX: number;
    mouseY: number;
    /** HEX colour under the cursor (eyedropper). */
    color?: string;
    /** Flash a "copied" confirmation in the readout. */
    copied?: boolean;
  }

  let { screenshotSrc, mouseX, mouseY, color = '', copied = false }: Props = $props();

  // HEX → "r, g, b" for the readout line.
  let rgb = $derived.by(() => {
    const m = /^#?([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})$/i.exec(color);
    if (!m) return '';
    return `${parseInt(m[1], 16)}, ${parseInt(m[2], 16)}, ${parseInt(m[3], 16)}`;
  });

  const ZOOM = 4;
  const SIZE = 164;

  let canvas: HTMLCanvasElement;
  let loadedImg: HTMLImageElement | null = $state(null);

  $effect(() => {
    const img = new Image();
    img.onload = () => { loadedImg = img; };
    img.src = screenshotSrc;
  });

  $effect(() => {
    if (!loadedImg || !canvas) return;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    const scaleX = loadedImg.naturalWidth / window.innerWidth;
    const scaleY = loadedImg.naturalHeight / window.innerHeight;

    const srcW = SIZE / ZOOM;
    const srcH = SIZE / ZOOM;
    const srcX = mouseX * scaleX - srcW / 2;
    const srcY = mouseY * scaleY - srcH / 2;

    ctx.clearRect(0, 0, SIZE, SIZE);
    ctx.drawImage(loadedImg, srcX, srcY, srcW, srcH, 0, 0, SIZE, SIZE);

    // Crosshair
    ctx.strokeStyle = 'rgba(255,255,255,0.85)';
    ctx.lineWidth = 1;
    ctx.beginPath();
    ctx.moveTo(SIZE / 2, 0); ctx.lineTo(SIZE / 2, SIZE);
    ctx.moveTo(0, SIZE / 2); ctx.lineTo(SIZE, SIZE / 2);
    ctx.stroke();

    // Centre pixel highlight
    const half = ZOOM / 2;
    ctx.strokeStyle = '#ef4444';
    ctx.lineWidth = 1.5;
    ctx.strokeRect(SIZE / 2 - half, SIZE / 2 - half, ZOOM, ZOOM);
  });

  let pos = $derived.by(() => {
    const off = 20;
    const maxX = typeof window !== 'undefined' ? window.innerWidth : 1920;
    const maxY = typeof window !== 'undefined' ? window.innerHeight : 1080;
    const lx = mouseX + off + SIZE > maxX ? mouseX - SIZE - off : mouseX + off;
    const ly = mouseY + off + SIZE + 22 > maxY ? mouseY - SIZE - off - 22 : mouseY + off;
    return `left:${lx}px;top:${ly}px`;
  });
</script>

<div
  class="fixed z-50 pointer-events-none rounded-lg overflow-hidden shadow-2xl border-2 border-white/30"
  style={pos}
>
  <canvas bind:this={canvas} width={SIZE} height={SIZE}></canvas>
  <div class="bg-black/85 text-white text-center leading-tight py-1 px-1.5">
    <div class="text-[11px] font-mono tracking-wider opacity-75">{mouseX} × {mouseY}</div>
    {#if color}
      <div class="flex items-center justify-center gap-1.5 mt-0.5">
        <span class="inline-block w-3 h-3 rounded-sm border border-white/50" style="background:{color}"></span>
        <span class="text-[11px] font-mono font-semibold">{copied ? '已复制!' : color.toUpperCase()}</span>
      </div>
      {#if rgb && !copied}
        <div class="text-[9px] font-mono opacity-60">rgb({rgb})</div>
      {/if}
      <div class="text-[9px] opacity-45 mt-0.5">按 C 复制颜色</div>
    {/if}
  </div>
</div>
