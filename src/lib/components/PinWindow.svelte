<script lang="ts">
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { listen, emit } from '@tauri-apps/api/event';
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';

  interface Props { imageData: string; }
  let { imageData }: Props = $props();

  const currentWindow = getCurrentWindow();
  let imgSrc = $derived(`data:image/png;base64,${imageData}`);

  let showControls = $state(false);
  let hideTimer: ReturnType<typeof setTimeout> | null = null;
  let opacity = $state(100);
  let scale = $state(1.0);
  let alwaysOnTop = $state(true);

  // ── Right-click context menu ───────────────────────────────────────
  let menuOpen = $state(false);
  let menuX = $state(0);
  let menuY = $state(0);
  let toast = $state<string | null>(null);
  let toastTimer: ReturnType<typeof setTimeout> | null = null;

  function flash(msg: string) {
    toast = msg;
    if (toastTimer) clearTimeout(toastTimer);
    toastTimer = setTimeout(() => (toast = null), 1100);
  }

  function onContextMenu(e: MouseEvent) {
    e.preventDefault();
    // Clamp so the menu stays inside the window.
    const mw = 168, mh = 232;
    menuX = Math.min(e.clientX, window.innerWidth - mw - 6);
    menuY = Math.min(e.clientY, window.innerHeight - mh - 6);
    menuOpen = true;
  }

  function closeMenu() { menuOpen = false; }

  async function copyImage() {
    closeMenu();
    try { await invoke('save_to_clipboard', { imageData }); flash('已复制'); }
    catch (err) { console.error('Copy failed:', err); flash('复制失败'); }
  }

  async function saveImage() {
    closeMenu();
    try { await invoke('save_to_file_dialog', { imageData }); }
    catch (err) { console.error('Save failed:', err); }
  }

  async function closeAllPins() {
    closeMenu();
    await emit('close-all-pins');
  }

  function onMouseEnter() {
    if (hideTimer) { clearTimeout(hideTimer); hideTimer = null; }
    showControls = true;
  }
  function onMouseLeave() {
    hideTimer = setTimeout(() => { showControls = false; }, 1200);
  }

  async function startDrag(e: MouseEvent) {
    if (e.button !== 0) return; // left button only — right opens the menu
    if (menuOpen) { closeMenu(); return; }
    if ((e.target as HTMLElement).closest('[data-controls]')) return;
    e.preventDefault();
    await currentWindow.startDragging();
  }

  async function closeWindow() { await currentWindow.close(); }

  async function toggleAlwaysOnTop() {
    alwaysOnTop = !alwaysOnTop;
    await currentWindow.setAlwaysOnTop(alwaysOnTop);
  }

  function onWheel(e: WheelEvent) {
    e.preventDefault();
    const delta = e.deltaY > 0 ? -0.1 : 0.1;
    scale = Math.max(0.2, Math.min(5.0, Math.round((scale + delta) * 10) / 10));
  }

  function onKeyDown(e: KeyboardEvent) {
    if (e.key === 'Escape') { if (menuOpen) closeMenu(); else closeWindow(); }
  }

  onMount(() => {
    const unlisten = listen('close-all-pins', () => closeWindow());
    return () => { unlisten.then(fn => fn()); };
  });
</script>

<svelte:window onkeydown={onKeyDown} />

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="fixed inset-0 overflow-hidden select-none"
  style="opacity:{opacity / 100}"
  onmousedown={startDrag}
  oncontextmenu={onContextMenu}
  onmouseenter={onMouseEnter}
  onmouseleave={onMouseLeave}
  onwheel={onWheel}
>
  <img
    src={imgSrc}
    alt="pinned screenshot"
    class="w-full h-full object-fill pointer-events-none block"
    draggable={false}
    style="transform:scale({scale});transform-origin:center;box-shadow:0 2px 20px rgba(0,0,0,0.6);outline:1px solid rgba(124,109,243,0.9);outline-offset:-1px;"
  />

  {#if showControls}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      data-controls=""
      class="absolute top-1.5 right-1.5 flex items-center gap-1 z-50"
      onmousedown={(e) => e.stopPropagation()}
    >
      <!-- Scale indicator -->
      {#if scale !== 1}
        <div class="bg-black/70 rounded px-1.5 py-0.5 text-[10px] text-white/60 font-mono">
          {Math.round(scale * 100)}%
        </div>
      {/if}

      <!-- Opacity slider -->
      <div class="flex items-center gap-1 bg-black/70 rounded-lg px-2 py-1 backdrop-blur-sm">
        <span class="text-white/60 text-[10px]">透明</span>
        <input
          type="range" min="20" max="100" step="5"
          bind:value={opacity}
          class="w-16 h-1 accent-violet-400 cursor-pointer"
        />
        <span class="text-white/60 text-[10px] w-6 text-right">{opacity}%</span>
      </div>

      <!-- Always-on-top toggle -->
      <button
        class="w-6 h-6 rounded flex items-center justify-center backdrop-blur-sm text-xs transition-colors
          {alwaysOnTop ? 'bg-violet-500/80 text-white' : 'bg-black/60 text-white/50 hover:text-white'}"
        title={alwaysOnTop ? '取消置顶' : '置顶'}
        onclick={toggleAlwaysOnTop}
      >
        <svg viewBox="0 0 24 24" class="w-3.5 h-3.5" fill="none" stroke="currentColor" stroke-width="2.5">
          <line x1="12" y1="17" x2="12" y2="22"/>
          <path d="M5 17h14v-1.76a2 2 0 0 0-1.11-1.79l-1.78-.9A2 2 0 0 1 15 10.76V6h1a2 2 0 0 0 0-4H8a2 2 0 0 0 0 4h1v4.76a2 2 0 0 1-1.11 1.79l-1.78.9A2 2 0 0 0 5 15.24Z"/>
        </svg>
      </button>

      <!-- Close -->
      <button
        class="w-6 h-6 rounded bg-black/60 hover:bg-red-500 text-white/80 hover:text-white flex items-center justify-center text-xs backdrop-blur-sm transition-colors"
        title="关闭 (ESC)"
        onclick={closeWindow}
      >✕</button>
    </div>

    <!-- Scroll hint -->
    <div class="absolute bottom-1.5 left-1/2 -translate-x-1/2 pointer-events-none">
      <div class="bg-black/50 text-white/40 text-[9px] px-2 py-0.5 rounded-full backdrop-blur-sm">
        滚轮缩放
      </div>
    </div>
  {/if}

  <!-- Toast -->
  {#if toast}
    <div class="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 pointer-events-none z-[60]">
      <div class="bg-black/75 text-white text-xs font-medium px-3 py-1.5 rounded-lg backdrop-blur-sm">
        {toast}
      </div>
    </div>
  {/if}

  <!-- Right-click context menu -->
  {#if menuOpen}
    <!-- Backdrop swallows the next click to dismiss the menu -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="fixed inset-0 z-[70]"
      onmousedown={(e) => { e.stopPropagation(); closeMenu(); }}
      oncontextmenu={(e) => { e.preventDefault(); closeMenu(); }}
    ></div>

    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      data-controls=""
      class="fixed z-[71] min-w-[160px] py-1 rounded-lg bg-[#26262b]/95 backdrop-blur-md border border-white/10 shadow-2xl text-sm text-white/90 select-none"
      style="left:{menuX}px; top:{menuY}px;"
      onmousedown={(e) => e.stopPropagation()}
      oncontextmenu={(e) => e.preventDefault()}
    >
      <button class="menu-item" onclick={copyImage}>
        <span class="menu-ico">📋</span>复制图片
      </button>
      <button class="menu-item" onclick={saveImage}>
        <span class="menu-ico">💾</span>保存为文件…
      </button>

      <div class="my-1 border-t border-white/10"></div>

      <button class="menu-item" onclick={() => { closeMenu(); toggleAlwaysOnTop(); }}>
        <span class="menu-ico">📌</span>{alwaysOnTop ? '取消置顶' : '置顶显示'}
      </button>

      <div class="my-1 border-t border-white/10"></div>

      <button class="menu-item menu-danger" onclick={() => { closeMenu(); closeWindow(); }}>
        <span class="menu-ico">🗑️</span>销毁此贴图
      </button>
      <button class="menu-item menu-danger" onclick={closeAllPins}>
        <span class="menu-ico">❌</span>关闭全部贴图
      </button>
    </div>
  {/if}
</div>

<style>
  .menu-item {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 6px 12px;
    text-align: left;
    background: transparent;
    border: none;
    color: inherit;
    cursor: pointer;
    transition: background 0.1s;
  }
  .menu-item:hover { background: rgba(124, 109, 243, 0.85); color: #fff; }
  .menu-danger:hover { background: rgba(239, 68, 68, 0.85); }
  .menu-ico { width: 16px; text-align: center; font-size: 12px; }
</style>
