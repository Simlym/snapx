<script lang="ts">
  interface Props {
    screenshotSrc: string;
    mouseX: number;
    mouseY: number;
  }

  let { screenshotSrc, mouseX, mouseY }: Props = $props();

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
  <div class="bg-black/80 text-white text-[11px] font-mono text-center py-0.5 leading-tight tracking-wider">
    {mouseX} × {mouseY}
  </div>
</div>
