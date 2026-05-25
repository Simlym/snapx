<script lang="ts">
  /**
   * App.svelte — SnapX main frontend.
   *
   * Architecture:
   * - Two Tauri windows both load this same frontend.
   * - The "main" window is hidden and only serves as the tray event receiver.
   *   It captures the screen, then sends data to the "overlay" window.
   * - The "overlay" window is transparent + always-on-top + no decorations.
   *   It receives capture data and renders the selection UI.
   *
   * Both windows listen to app-wide events so they can coordinate.
   */
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import { invoke } from '@tauri-apps/api/core';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import CaptureOverlay from './lib/components/CaptureOverlay.svelte';

  const currentWindow = getCurrentWindow();
  const windowLabel: string = currentWindow.label;

  // ── Overlay state ──
  let overlayVisible = $state(false);
  let screenshotData = $state<string | null>(null);
  let screenshotWidth = $state(0);
  let screenshotHeight = $state(0);

  // ── Shared: listen for capture trigger (both windows receive app events) ──
  let unlisteners: UnlistenFn[] = [];

  async function init() {
    // Listen for capture trigger
    const ul1 = await listen<string>('trigger-capture', async (event) => {
      if (windowLabel !== 'main') return; // Only main window initiates capture
      await handleCaptureTrigger(event.payload);
    });
    unlisteners.push(ul1);

    // Overlay listens for data
    const ul2 = await listen<{
      image_data: string;
      width: number;
      height: number;
    }>('show-overlay', async (event) => {
      if (windowLabel !== 'overlay') return;
      screenshotData = event.payload.image_data;
      screenshotWidth = event.payload.width;
      screenshotHeight = event.payload.height;
      overlayVisible = true;
    });
    unlisteners.push(ul2);
  }

  /**
   * Main window only: capture screen, then show overlay window with data.
   */
  async function handleCaptureTrigger(mode: string) {
    try {
      // Small delay so menu / shortcut interaction doesn't appear in screenshot
      await new Promise((r) => setTimeout(r, 250));

      if (mode === 'fullscreen') {
        const result = await invoke<{
          image_data: string;
          width: number;
          height: number;
          monitor_id: number;
        }>('capture_screens', { monitorIndex: 0 });
        await invoke('save_to_clipboard', { imageData: result.image_data });
        return;
      }

      // Capture the screen
      const result = await invoke<{
        image_data: string;
        width: number;
        height: number;
        monitor_id: number;
      }>('capture_screens', { monitorIndex: 0 });

      // Show overlay window and send it the capture data
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
      console.error('Capture failed:', err);
    }
  }

  // ── Overlay: region selected → crop → save → hide ──
  async function onRegionSelected(region: {
    x: number;
    y: number;
    width: number;
    height: number;
  }) {
    try {
      const result = await invoke<{
        image_data: string;
        width: number;
        height: number;
        monitor_id: number;
      }>('capture_region', {
        monitorIndex: 0,
        x: Math.round(region.x),
        y: Math.round(region.y),
        width: Math.round(region.width),
        height: Math.round(region.height),
      });
      await invoke('save_to_clipboard', { imageData: result.image_data });
    } catch (err) {
      console.warn('Server crop failed, canvas fallback:', err);
      const cropped = await canvasCrop(region);
      if (cropped) {
        try {
          await invoke('save_to_clipboard', { imageData: cropped });
        } catch (e) {
          console.error('Clipboard write failed:', e);
        }
      }
    }
    await hideOverlay();
  }

  async function canvasCrop(region: {
    x: number;
    y: number;
    width: number;
    height: number;
  }): Promise<string | null> {
    if (!screenshotData) return null;
    return new Promise((resolve) => {
      const img = new Image();
      img.onload = () => {
        const c = document.createElement('canvas');
        c.width = region.width;
        c.height = region.height;
        const ctx = c.getContext('2d')!;
        ctx.drawImage(img, region.x, region.y, region.width, region.height, 0, 0, region.width, region.height);
        resolve(c.toDataURL('image/png').replace(/^data:image\/png;base64,/, ''));
      };
      img.onerror = () => resolve(null);
      img.src = `data:image/png;base64,${screenshotData}`;
    });
  }

  async function onCancel() {
    await hideOverlay();
  }

  async function hideOverlay() {
    overlayVisible = false;
    screenshotData = null;
    await currentWindow.setFullscreen(false);
    await currentWindow.hide();
  }

  // ── Lifecycle ──
  $effect(() => {
    init();
    return () => {
      unlisteners.forEach((fn) => fn());
    };
  });
</script>

{#if windowLabel === 'overlay' && overlayVisible && screenshotData}
  <CaptureOverlay
    {screenshotData}
    screenshotWidth={screenshotWidth}
    screenshotHeight={screenshotHeight}
    onselect={onRegionSelected}
    oncancel={onCancel}
  />
{/if}
