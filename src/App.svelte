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

  const currentWindow = getCurrentWindow();
  const windowLabel: string = currentWindow.label;

  const isOverlay = windowLabel === 'overlay';
  const isMain    = windowLabel === 'main';
  const isPin     = windowLabel.startsWith('pin-');

  // ── Overlay state ──────────────────────────────────────────────────
  let overlayVisible   = $state(false);
  let screenshotData   = $state<string | null>(null);
  let screenshotWidth  = $state(0);
  let screenshotHeight = $state(0);

  // ── Delay-capture countdown ────────────────────────────────────────
  let countdown = $state<number | null>(null);

  // ── Pin state ──────────────────────────────────────────────────────
  let pinImageData = $state<string | null>(null);

  // ── Settings state ─────────────────────────────────────────────────
  let settingsVisible = $state(false);

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
    }

    if (isOverlay) {
      const ul3 = await listen<{ image_data: string; width: number; height: number }>(
        'show-overlay',
        async (event) => {
          screenshotData   = event.payload.image_data;
          screenshotWidth  = event.payload.width;
          screenshotHeight = event.payload.height;
          overlayVisible   = true;
        }
      );
      unlisteners.push(ul3);
    }
  }

  async function handleCaptureTrigger(mode: string) {
    try {
      // Delayed capture: mode = "delayed-2" or "delayed-5"
      if (mode.startsWith('delayed-')) {
        const secs = parseInt(mode.split('-')[1]) || 0;
        await currentWindow.show();
        for (let i = secs; i > 0; i--) {
          countdown = i;
          await new Promise((r) => setTimeout(r, 1000));
        }
        countdown = null;
        await currentWindow.hide();
        mode = 'region';
      }

      await new Promise((r) => setTimeout(r, 250));

      const result = await invoke<{ image_data: string; width: number; height: number; monitor_id: number }>(
        'capture_screens', { monitorIndex: 0 }
      );

      if (mode === 'fullscreen') {
        await invoke('save_to_clipboard', { imageData: result.image_data });
        return;
      }

      const { WebviewWindow } = await import('@tauri-apps/api/webviewWindow');
      const overlayWin = await WebviewWindow.getByLabel('overlay');
      if (overlayWin) {
        await overlayWin.emit('show-overlay', {
          image_data: result.image_data,
          width: result.width,
          height: result.height,
        });
        await overlayWin.show();
        await overlayWin.setFocus();
        await overlayWin.setFullscreen(true);
      }
    } catch (err) {
      countdown = null;
      console.error('Capture failed:', err);
    }
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

  async function onCancel() { await hideOverlay(); }

  async function hideOverlay() {
    overlayVisible = false;
    screenshotData = null;
    await currentWindow.setFullscreen(false);
    await currentWindow.hide();
  }

  $effect(() => {
    init();
    return () => { unlisteners.forEach((fn) => fn()); };
  });
</script>

{#if isOverlay && overlayVisible && screenshotData}
  <CaptureOverlay
    {screenshotData}
    {screenshotWidth}
    {screenshotHeight}
    oncopy={onCopy}
    onsave={onSave}
    onquicksave={onQuickSave}
    onpin={onPin}
    oncancel={onCancel}
  />
{/if}

{#if isPin && pinImageData}
  <PinWindow imageData={pinImageData} />
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
