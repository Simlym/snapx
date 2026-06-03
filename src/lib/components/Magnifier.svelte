<script lang="ts">
  interface Props {
    screenshotSrc: string;
    mouseX: number;
    mouseY: number;
    color?: string;
    copied?: boolean;
    colorFormat?: 'hex' | 'rgb' | 'hsl';
  }

  let { screenshotSrc, mouseX, mouseY, color = '', copied = false, colorFormat = 'rgb' }: Props = $props();

  function hexToRgb(hex: string): [number, number, number] | null {
    const m = /^#?([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})$/i.exec(hex);
    if (!m) return null;
    return [parseInt(m[1], 16), parseInt(m[2], 16), parseInt(m[3], 16)];
  }

  let displayColor = $derived.by(() => {
    if (!color) return '';
    const rgb = hexToRgb(color);
    if (!rgb) return color.toUpperCase();
    const [r, g, b] = rgb;
    if (colorFormat === 'rgb') return `rgb(${r}, ${g}, ${b})`;
    if (colorFormat === 'hsl') {
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
    return color.toUpperCase();
  });

  const ZOOM = 4;
  const SIZE = 160;

  let canvas: HTMLCanvasElement;
  let loadedImg: HTMLImageElement | null = $state(null);

  $effect(() => {
    let cancelled = false;
    const img = new Image();
    img.onload = () => { if (!cancelled) loadedImg = img; };
    img.src = screenshotSrc;
    return () => { cancelled = true; };
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
    ctx.imageSmoothingEnabled = false;
    ctx.drawImage(loadedImg, srcX, srcY, srcW, srcH, 0, 0, SIZE, SIZE);

    // Pixel grid (subtle dark lines at every zoom-cell boundary)
    ctx.strokeStyle = 'rgba(0,0,0,0.22)';
    ctx.lineWidth = 0.5;
    for (let x = 0; x <= SIZE; x += ZOOM) {
      ctx.beginPath(); ctx.moveTo(x, 0); ctx.lineTo(x, SIZE); ctx.stroke();
    }
    for (let y = 0; y <= SIZE; y += ZOOM) {
      ctx.beginPath(); ctx.moveTo(0, y); ctx.lineTo(SIZE, y); ctx.stroke();
    }

    // Crosshair — blue, same accent colour as the selection border
    ctx.strokeStyle = 'rgba(59,130,246,0.92)';
    ctx.lineWidth = 1;
    ctx.beginPath();
    ctx.moveTo(SIZE / 2, 0);    ctx.lineTo(SIZE / 2, SIZE);
    ctx.moveTo(0, SIZE / 2);    ctx.lineTo(SIZE, SIZE / 2);
    ctx.stroke();

    // Centre pixel highlight
    const half = ZOOM / 2;
    ctx.strokeStyle = 'rgba(255,255,255,0.9)';
    ctx.lineWidth = 1.5;
    ctx.strokeRect(SIZE / 2 - half, SIZE / 2 - half, ZOOM, ZOOM);
  });

  // Panel height estimate for offset calculation
  const PANEL_H = $derived(color ? 82 : 24);

  let pos = $derived.by(() => {
    const off = 18;
    const maxX = typeof window !== 'undefined' ? window.innerWidth  : 1920;
    const maxY = typeof window !== 'undefined' ? window.innerHeight : 1080;
    const lx = mouseX + off + SIZE > maxX ? mouseX - SIZE - off : mouseX + off;
    const ly = mouseY + off + SIZE + PANEL_H > maxY
      ? mouseY - SIZE - PANEL_H - off
      : mouseY + off;
    return `left:${lx}px;top:${ly}px`;
  });
</script>

<div
  class="fixed z-50 pointer-events-none rounded-lg overflow-hidden shadow-2xl border border-white/15"
  style={pos}
>
  <canvas bind:this={canvas} width={SIZE} height={SIZE}></canvas>
  <div class="bg-[#1c1c1c]/92 text-white text-center leading-tight py-1.5 px-2.5">
    <div class="text-[11px] font-mono tracking-wide text-white/75">({mouseX},&nbsp;{mouseY})</div>
    {#if color}
      <div class="flex items-center justify-center gap-1.5 mt-0.5">
        <span
          class="inline-block w-[14px] h-[14px] rounded-sm border border-white/35 flex-shrink-0"
          style="background:{color}"
        ></span>
        <span class="text-[11px] font-mono font-semibold text-white">
          {copied ? '已复制!' : displayColor}
        </span>
      </div>
      {#if !copied}
        <div class="mt-1 space-y-0.5 text-[9px] text-white/40 font-mono">
          <div>Shift: 切换颜色格式</div>
          <div>C: 复制色值</div>
        </div>
      {/if}
    {/if}
  </div>
</div>
