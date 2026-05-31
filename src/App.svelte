<script lang="ts">
  /**
   * App.svelte — SnapX main frontend.
   * Window routing by label:
   * - "main"    → settings UI + capture orchestration
   * - "overlay" → transparent capture + annotation overlay
   * - "pin-*"   → always-on-top pinned image window
   */
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import { invoke } from '@tauri-apps/api/core';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import CaptureOverlay from './lib/components/CaptureOverlay.svelte';
  import PinWindow from './lib/components/PinWindow.svelte';
  import SettingsPanel from './lib/components/SettingsPanel.svelte';
  import QuickAccess from './lib/components/QuickAccess.svelte';

  const currentWindow = getCurrentWindow();
  const windowLabel: string = currentWindow.label;

  const isOverlay = windowLabel === 'overlay';
  const isMain    = windowLabel === 'main';
  const isPin     = windowLabel.startsWith('pin-');
  const isQuick   = windowLabel.startsWith('quick-');

  // ── Overlay state ──────────────────────────────────────────────────
  let overlayVisible   = $state(false);
  let screenshotData   = $state<string | null>(null);
  let screenshotWidth  = $state(0);
  let screenshotHeight = $state(0);
  let overlayScrollMode = $state(false);
  /** "png" = base64 PNG, "rgba" = raw RGBA pixels (base64) needing canvas decode */
  let screenshotFormat = $state<'png' | 'rgba'>('png');

  // ── Long-screenshot (scroll capture) state, runs in the main window ──
  let scrollActive = $state(false);
  let scrollRegion: { x: number; y: number; w: number; h: number } | null = null;
  let scrollFrames = $state(0);
  let scrollHeight = $state(0);
  let scrollBusy = $state(false);

  // ── Delay-capture countdown ────────────────────────────────────────
  let countdown = $state<number | null>(null);

  // ── Pin state ──────────────────────────────────────────────────────
  let pinImageData = $state<string | null>(null);

  // ── Quick-access (post-capture corner floater) state ───────────────
  let quickImageData = $state<string | null>(null);
  let quickW = $state(0);
  let quickH = $state(0);

  // ── Settings state ─────────────────────────────────────────────────
  let settingsVisible = $state(false);

  // Overlay is switched to fullscreen only once, then kept fullscreen across
  // captures (we just show/hide it). This skips the costly per-capture
  // fullscreen transition — the single biggest win for perceived speed.
  let overlayFullscreenSet = false;

  let unlisteners: UnlistenFn[] = [];

  async function init() {
    if (isPin) {
      const params = new URLSearchParams(window.location.search);
      const pinId = params.get('id');
      if (pinId) {
        try {
          const raw = await invoke<string | null>('get_pin_data', { id: pinId });
          if (raw) {
            const parsed = JSON.parse(raw) as { data: string; width: number; height: number };
            pinImageData = parsed.data;
            await invoke('remove_pin_data', { id: pinId });
            await sizeAndShowPin(parsed.width, parsed.height);
          }
        } catch (e) { console.error('Failed to load pin data:', e); }
      }
      return;
    }

    if (isQuick) {
      const params = new URLSearchParams(window.location.search);
      const id = params.get('id');
      if (id) {
        try {
          const raw = await invoke<string | null>('get_pin_data', { id });
          if (raw) {
            const p = JSON.parse(raw) as { data: string; width: number; height: number };
            quickImageData = p.data;
            quickW = p.width;
            quickH = p.height;
            await invoke('remove_pin_data', { id });
            await positionQuickAccess();
          }
        } catch (e) { console.error('Failed to load quick data:', e); }
      }
      return;
    }

    if (isMain) {
      const ul = await listen<void>('show-settings', async () => {
        settingsVisible = true;
        await currentWindow.show();
        await currentWindow.setFocus();
      });
      unlisteners.push(ul);

      const ulClose = await currentWindow.onCloseRequested(async (event) => {
        event.preventDefault();
        settingsVisible = false;
        await currentWindow.hide();
      });
      unlisteners.push(ulClose);

      // Re-apply saved shortcut
      try {
        const raw = localStorage.getItem('snapx-shortcut');
        if (raw) {
          const { mods, key } = JSON.parse(raw) as { mods: string[]; key: string };
          await invoke('update_global_shortcut', { mods, key });
        }
      } catch (_) {}
    }

    if (isMain) {
      const ul1 = await listen<string>('trigger-capture', async (event) => {
        await handleCaptureTrigger(event.payload);
      });
      unlisteners.push(ul1);

      const ul2 = await listen<void>('paste-pin', async () => {
        await handlePastePin();
      });
      unlisteners.push(ul2);

      const ul4 = await listen<{ x: number; y: number; w: number; h: number }>(
        'scroll-start',
        async (event) => { await beginScrollSession(event.payload); }
      );
      unlisteners.push(ul4);
    }

    if (isOverlay) {
      // Make the overlay fullscreen once, up front, while it's still hidden.
      // Rust shows this window with a bare show() on the shortcut path, so it
      // must already be fullscreen — we can't rely on a JS-side transition then.
      try {
        await currentWindow.setFullscreen(true);
        overlayFullscreenSet = true;
      } catch (e) { console.error('overlay fullscreen failed:', e); }

      // New fast path: Rust shows the overlay window and fires this the instant
      // the shortcut/tray is hit. We render the selection UI immediately over
      // the live desktop — NO screenshot is taken yet. The full-screen grab is
      // deferred until the user commits a region (see captureForSelection).
      const ul3 = await listen<string>('start-selection', async (event) => {
        const t0 = performance.now();
        perf(`[overlay] start-selection received, mode=${event.payload}`);
        // Reset so a fresh selection starts clean each time.
        screenshotData    = null;
        overlayScrollMode = event.payload === 'scroll';
        overlayVisible    = true;
        requestAnimationFrame(() => requestAnimationFrame(() => {
          perf(`[overlay] selection UI painted: +${(performance.now() - t0).toFixed(0)}ms (INSTANT)`);
        }));
      });
      unlisteners.push(ul3);
    }
  }

  // Capture ONLY the selected region (physical px) to serve as the annotation
  // backdrop. Called the moment the user commits a region — i.e. after framing
  // (and any edge tweaks), so we encode just that small crop instead of a 4K
  // full screen. That keeps annotate/copy/save/pin snappy.
  async function captureForSelection(
    region: { x: number; y: number; w: number; h: number }
  ): Promise<{ data: string; width: number; height: number } | null> {
    const t0 = performance.now();
    try {
      // The overlay's dim mask must NOT be baked into the grab. xcap captures
      // the real desktop including this transparent window, so we hide it for
      // the duration of the capture, then bring it straight back.
      await currentWindow.hide();
      const result = await invoke<{ image_data: string; width: number; height: number }>(
        'capture_region',
        { monitorIndex: 0, x: region.x, y: region.y, width: region.w, height: region.h }
      );
      await currentWindow.show();
      await currentWindow.setFocus();
      perf(`[overlay] captureForSelection (region ${region.w}x${region.h}): +${(performance.now() - t0).toFixed(0)}ms (${(result.image_data.length / 1024).toFixed(0)}KB)`);
      screenshotData   = result.image_data;
      screenshotWidth  = result.width;
      screenshotHeight = result.height;
      return { data: result.image_data, width: result.width, height: result.height };
    } catch (err) {
      console.error('captureForSelection failed:', err);
      return null;
    }
  }

  // Fire-and-forget perf log that lands in the `pnpm tauri dev` terminal
  // alongside the Rust `[perf]` prints, so the whole chain is in one stream.
  function perf(msg: string) {
    invoke('perf_log', { msg }).catch(() => {});
  }

  // Runs in the MAIN window. Only handles modes that don't go straight to the
  // selection overlay: "fullscreen" (grab + clipboard, no UI) and "delayed-*"
  // (countdown, then hand off to the overlay for region selection). The common
  // region/scroll paths are now triggered directly from Rust → overlay, so the
  // trigger is instant and never round-trips a screenshot through this window.
  async function handleCaptureTrigger(mode: string) {
    try {
      // Delayed capture: mode = "delayed-2" or "delayed-5" → countdown, then
      // open the selection overlay just like a normal region capture.
      if (mode.startsWith('delayed-')) {
        const secs = parseInt(mode.split('-')[1]) || 0;
        await currentWindow.show();
        for (let i = secs; i > 0; i--) {
          countdown = i;
          await new Promise((r) => setTimeout(r, 1000));
        }
        countdown = null;
        await currentWindow.hide();
        await showSelectionOverlay('region');
        return;
      }

      if (mode === 'fullscreen') {
        const result = await invoke<{ image_data: string }>('capture_screens', { monitorIndex: 0 });
        await invoke('save_to_clipboard', { imageData: result.image_data });
        return;
      }
    } catch (err) {
      countdown = null;
      console.error('Capture failed:', err);
    }
  }

  // Show the (already-fullscreen) overlay window and tell it to begin selecting.
  // Mirrors what Rust does on the global shortcut; used by the delayed-capture
  // path which must run its countdown in the main window first.
  async function showSelectionOverlay(mode: 'region' | 'scroll') {
    const { WebviewWindow } = await import('@tauri-apps/api/webviewWindow');
    const overlayWin = await WebviewWindow.getByLabel('overlay');
    if (!overlayWin) return;
    if (!overlayFullscreenSet) {
      await overlayWin.setFullscreen(true);
      overlayFullscreenSet = true;
    }
    await overlayWin.show();
    await overlayWin.setFocus();
    await overlayWin.emit('start-selection', mode);
  }

  async function handlePastePin() {
    try {
      const imageData = await invoke<string | null>('read_clipboard_image');
      if (!imageData) return;
      const pinId = crypto.randomUUID();
      const pinLabel = `pin-${Date.now()}`;
      await invoke('store_pin_data', {
        id: pinId,
        data: JSON.stringify({ data: imageData, width: 400, height: 300 }),
      });
      const { WebviewWindow } = await import('@tauri-apps/api/webviewWindow');
      new WebviewWindow(pinLabel, {
        url: `/?win=pin&id=${encodeURIComponent(pinId)}`,
        title: 'SnapX 贴图',
        width: 400, height: 300,
        decorations: false, transparent: true,
        alwaysOnTop: true, skipTaskbar: false,
        resizable: true, center: true, shadow: false,
      });
    } catch (err) { console.error('Paste pin failed:', err); }
  }

  async function onCopy(imageData: string) {
    try { await invoke('save_to_clipboard', { imageData }); }
    catch (err) { console.error('Clipboard write failed:', err); }
    await hideOverlay();
    showQuickAccess(imageData);
  }

  async function onSave(imageData: string) {
    await hideOverlay();
    try { await invoke('save_to_file_dialog', { imageData }); }
    catch (err) { console.error('Save failed:', err); }
  }

  async function onQuickSave(imageData: string) {
    await hideOverlay();
    try {
      const path = await invoke<string>('save_to_quicksave', { imageData });
      console.log('Quick saved to:', path);
    } catch (err) { console.error('Quick save failed:', err); }
    showQuickAccess(imageData);
  }

  // Spawn the post-capture corner floater. Disabled via a localStorage flag.
  async function showQuickAccess(imageData: string) {
    if (localStorage.getItem('snapx-quickaccess') === 'off') return;
    try {
      const dims = await new Promise<{ w: number; h: number }>((res) => {
        const im = new Image();
        im.onload = () => res({ w: im.naturalWidth, h: im.naturalHeight });
        im.onerror = () => res({ w: 0, h: 0 });
        im.src = `data:image/png;base64,${imageData}`;
      });
      const id = crypto.randomUUID();
      await invoke('store_pin_data', {
        id,
        data: JSON.stringify({ data: imageData, width: dims.w, height: dims.h }),
      });
      const { WebviewWindow } = await import('@tauri-apps/api/webviewWindow');
      new WebviewWindow(`quick-${Date.now()}`, {
        url: `/?win=quick&id=${encodeURIComponent(id)}`,
        title: 'SnapX',
        width: 280, height: 188,
        decorations: false, transparent: true,
        alwaysOnTop: true, skipTaskbar: true,
        resizable: false, shadow: false, visible: false,
      });
    } catch (e) { console.error('Quick access failed:', e); }
  }

  // Pin the quick-access window to the bottom-right of its current monitor.
  async function positionQuickAccess() {
    const { PhysicalPosition, currentMonitor, primaryMonitor } =
      await import('@tauri-apps/api/window');
    try {
      const mon = (await currentMonitor()) ?? (await primaryMonitor());
      const size = await currentWindow.outerSize();
      if (mon) {
        const margin = Math.round(24 * mon.scaleFactor);
        const taskbar = Math.round(56 * mon.scaleFactor);
        const x = mon.position.x + mon.size.width - size.width - margin;
        const y = mon.position.y + mon.size.height - size.height - taskbar;
        await currentWindow.setPosition(new PhysicalPosition(x, y));
      }
    } catch (e) { console.error('Quick position failed:', e); }
    await currentWindow.show();
    await currentWindow.setFocus();
  }

  // pxW/pxH are the composite image's *physical* pixel dimensions.
  async function onPin(imageData: string, pxW: number, pxH: number) {
    const pinId = crypto.randomUUID();
    const pinLabel = `pin-${Date.now()}`;
    try {
      await invoke('store_pin_data', {
        id: pinId,
        data: JSON.stringify({ data: imageData, width: pxW, height: pxH }),
      });
      const { WebviewWindow } = await import('@tauri-apps/api/webviewWindow');
      // Created hidden; the pin window sizes itself to the image's physical
      // pixels and shows itself once its data loads (see sizeAndShowPin).
      new WebviewWindow(pinLabel, {
        url: `/?win=pin&id=${encodeURIComponent(pinId)}`,
        title: 'SnapX 贴图',
        decorations: false, transparent: true,
        alwaysOnTop: true, skipTaskbar: false,
        resizable: true, shadow: false, visible: false,
      });
    } catch (err) { console.error('Pin failed:', err); }
    await hideOverlay();
  }

  // Runs inside the pin window: set its content area to the image's exact
  // physical-pixel size so the image maps 1:1 → crisp on any DPI / monitor.
  async function sizeAndShowPin(pxW: number, pxH: number) {
    const { PhysicalSize, primaryMonitor } = await import('@tauri-apps/api/window');
    let w = pxW, h = pxH;
    try {
      const mon = await primaryMonitor();
      if (mon) {
        const k = Math.min(1, (mon.size.width * 0.9) / w, (mon.size.height * 0.9) / h);
        w = Math.round(w * k);
        h = Math.round(h * k);
      }
    } catch (_) { /* clamp is best-effort */ }
    try {
      await currentWindow.setSize(new PhysicalSize(w, h));
      await currentWindow.center();
    } catch (e) {
      console.error('Pin window sizing failed:', e);
    }
    // Always show, even if sizing failed, so the pin never gets stuck hidden.
    await currentWindow.show();
    await currentWindow.setFocus();
  }

  // Long screenshot: overlay (in its own window) selected a region → tell the
  // main window to run the scroll-capture session, then hide the overlay.
  async function onScrollRegion(region: { x: number; y: number; w: number; h: number }) {
    try {
      const { WebviewWindow } = await import('@tauri-apps/api/webviewWindow');
      const mainWin = await WebviewWindow.getByLabel('main');
      await mainWin?.emit('scroll-start', region);
    } catch (e) { console.error('scroll-start emit failed:', e); }
    await hideOverlay();
  }

  // Begin a scroll-capture session (runs in the main window).
  async function beginScrollSession(region: { x: number; y: number; w: number; h: number }) {
    scrollRegion = region;
    scrollFrames = 0;
    scrollHeight = 0;
    scrollActive = true;
    try {
      await invoke('scroll_begin');
    } catch (e) { console.error('scroll_begin failed:', e); }
    await currentWindow.show();
    await currentWindow.setFocus();
    // Capture the first frame immediately.
    await captureScrollFrame();
  }

  // Capture one frame of the selected region and feed it to the stitcher.
  async function captureScrollFrame() {
    if (!scrollRegion || scrollBusy) return;
    scrollBusy = true;
    try {
      const r = scrollRegion;
      const res = await invoke<{ image_data: string }>('capture_region', {
        monitorIndex: 0, x: r.x, y: r.y, width: r.w, height: r.h,
      });
      scrollHeight = await invoke<number>('scroll_add_frame', { imageData: res.image_data });
      scrollFrames += 1;
    } catch (e) { console.error('scroll frame failed:', e); }
    finally { scrollBusy = false; }
  }

  // Finish the session → stitched long image becomes a pin.
  async function finishScrollSession() {
    scrollActive = false;
    try {
      const res = await invoke<{ image_data: string; width: number; height: number }>('scroll_finish');
      await currentWindow.hide();
      await onPin(res.image_data, res.width, res.height);
    } catch (e) { console.error('scroll_finish failed:', e); await currentWindow.hide(); }
    scrollRegion = null;
  }

  async function cancelScrollSession() {
    scrollActive = false;
    scrollRegion = null;
    try { await invoke('scroll_finish'); } catch (_) { /* discard */ }
    await currentWindow.hide();
  }

  async function onCancel() {
    if (overlayScrollMode) {
      // Cancelled region selection in scroll mode — nothing to clean up.
    }
    await hideOverlay();
  }

  async function hideOverlay() {
    overlayVisible = false;
    screenshotData = null;
    // Keep the window fullscreen (just hide it) so the next capture skips the
    // fullscreen transition entirely.
    await currentWindow.hide();
  }

  $effect(() => {
    init();
    return () => { unlisteners.forEach((fn) => fn()); };
  });
</script>

{#if isOverlay && overlayVisible}
  <CaptureOverlay
    bind:screenshotData
    bind:screenshotWidth
    bind:screenshotHeight
    scrollMode={overlayScrollMode}
    {captureForSelection}
    oncopy={onCopy}
    onsave={onSave}
    onquicksave={onQuickSave}
    onpin={onPin}
    oncancel={onCancel}
    onscroll={onScrollRegion}
  />
{/if}

{#if isPin && pinImageData}
  <PinWindow imageData={pinImageData} />
{/if}

{#if isQuick && quickImageData}
  <QuickAccess imageData={quickImageData} width={quickW} height={quickH} />
{/if}

{#if isMain && settingsVisible}
  <SettingsPanel onclose={async () => {
    settingsVisible = false;
    await currentWindow.hide();
  }} />
{/if}

<!-- Delay countdown (shown in main window) -->
{#if isMain && countdown !== null}
  <div class="fixed inset-0 bg-black/70 flex items-center justify-center pointer-events-none">
    <div class="text-white text-center">
      <div class="text-9xl font-bold tabular-nums leading-none drop-shadow-2xl">{countdown}</div>
      <div class="text-white/60 text-lg mt-4">秒后截图…</div>
    </div>
  </div>
{/if}

<!-- Long-screenshot control bar (shown in main window during a scroll session) -->
{#if isMain && scrollActive}
  <div class="fixed inset-0 flex items-center justify-center" style="background: var(--surface, #1a1a1a);">
    <div class="text-center text-white space-y-5 px-8">
      <div class="text-lg font-medium">长截图捕获中</div>
      <div class="text-sm text-white/60 max-w-xs mx-auto leading-relaxed">
        切换到目标窗口向下滚动一屏，回到这里点「捕获下一帧」。重复直到内容到底，再点「完成」。
      </div>
      <div class="flex items-center justify-center gap-6 text-sm">
        <div><span class="text-white/50">帧数</span> <span class="font-mono font-semibold">{scrollFrames}</span></div>
        <div><span class="text-white/50">高度</span> <span class="font-mono font-semibold">{scrollHeight}px</span></div>
      </div>
      <div class="flex items-center justify-center gap-3 pt-2">
        <button
          class="px-4 py-2 rounded-lg text-white font-medium transition-transform active:scale-95 disabled:opacity-50"
          style="background: var(--accent, #3b82f6);"
          onclick={captureScrollFrame}
          disabled={scrollBusy}
        >{scrollBusy ? '捕获中…' : '捕获下一帧'}</button>
        <button
          class="px-4 py-2 rounded-lg bg-emerald-600 hover:bg-emerald-500 text-white font-medium transition-transform active:scale-95"
          onclick={finishScrollSession}
        >完成</button>
        <button
          class="px-4 py-2 rounded-lg text-white/60 hover:text-white hover:bg-white/10 transition-colors"
          onclick={cancelScrollSession}
        >取消</button>
      </div>
    </div>
  </div>
{/if}
