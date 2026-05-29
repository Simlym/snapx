<script lang="ts">
  /**
   * QuickAccess — a small floating thumbnail shown in the bottom-right corner
   * right after a capture is copied/quick-saved. It keeps the capture in an
   * "actionable" state (copy again, save, pin) instead of vanishing, which is
   * the friction CleanShot X's overlay removes. Auto-dismisses unless hovered.
   */
  import { invoke } from '@tauri-apps/api/core';
  import { getCurrentWindow } from '@tauri-apps/api/window';

  interface Props {
    /** base64 PNG of the captured image */
    imageData: string;
    /** physical pixel dimensions (for pinning at 1:1) */
    width: number;
    height: number;
  }
  let { imageData, width, height }: Props = $props();

  const win = getCurrentWindow();
  let hovering = $state(false);
  let toast = $state<string | null>(null);
  let dismissTimer: ReturnType<typeof setTimeout> | null = null;

  function scheduleDismiss(ms = 4500) {
    if (dismissTimer) clearTimeout(dismissTimer);
    dismissTimer = setTimeout(() => { if (!hovering) close(); }, ms);
  }

  async function close() {
    try { await win.close(); } catch (_) { /* already gone */ }
  }

  function flash(msg: string) {
    toast = msg;
    setTimeout(() => (toast = null), 1100);
  }

  async function copyAgain() {
    try { await invoke('save_to_clipboard', { imageData }); flash('已复制'); }
    catch (e) { console.error(e); }
    scheduleDismiss(2500);
  }

  async function save() {
    try { await invoke('save_to_file_dialog', { imageData }); }
    catch (e) { console.error(e); }
    await close();
  }

  async function pin() {
    try {
      const pinId = crypto.randomUUID();
      await invoke('store_pin_data', {
        id: pinId,
        data: JSON.stringify({ data: imageData, width, height }),
      });
      const { WebviewWindow } = await import('@tauri-apps/api/webviewWindow');
      new WebviewWindow(`pin-${Date.now()}`, {
        url: `/?win=pin&id=${encodeURIComponent(pinId)}`,
        title: 'SnapX 贴图',
        decorations: false, transparent: true,
        alwaysOnTop: true, skipTaskbar: false,
        resizable: true, shadow: false, visible: false,
      });
    } catch (e) { console.error(e); }
    await close();
  }

  $effect(() => { scheduleDismiss(); });
</script>

<div
  class="quick"
  role="group"
  onmouseenter={() => { hovering = true; if (dismissTimer) clearTimeout(dismissTimer); }}
  onmouseleave={() => { hovering = false; scheduleDismiss(2000); }}
>
  <button class="thumb" title="拖拽或点击贴图" onclick={pin} aria-label="贴图">
    <img src="data:image/png;base64,{imageData}" alt="" draggable="false" />
    {#if toast}<span class="toast">{toast}</span>{/if}
  </button>
  <div class="bar">
    <span class="label">已截图</span>
    <div class="spacer"></div>
    <button class="act" title="复制" onclick={copyAgain} aria-label="复制">📋</button>
    <button class="act" title="保存" onclick={save} aria-label="保存">💾</button>
    <button class="act" title="贴图" onclick={pin} aria-label="贴图">📌</button>
    <button class="act close" title="关闭" onclick={close} aria-label="关闭">✕</button>
  </div>
</div>

<style>
  .quick {
    position: fixed;
    inset: 0;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    border-radius: var(--radius-lg, 14px);
    background: var(--glass-bg, rgba(32, 33, 36, 0.92));
    border: 1px solid var(--glass-border, rgba(255, 255, 255, 0.1));
    box-shadow: var(--glass-shadow, 0 8px 28px rgba(0, 0, 0, 0.45));
    backdrop-filter: blur(16px) saturate(140%);
    -webkit-backdrop-filter: blur(16px) saturate(140%);
    animation: snapx-pop-in 0.18s var(--ease, ease) both;
  }
  .thumb {
    position: relative;
    flex: 1;
    min-height: 0;
    border: none;
    padding: 0;
    background: rgba(0, 0, 0, 0.25);
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .thumb img {
    max-width: 100%;
    max-height: 100%;
    object-fit: contain;
    display: block;
  }
  .toast {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(0, 0, 0, 0.55);
    color: #fff;
    font-size: 13px;
    font-weight: 600;
    animation: snapx-fade-in 0.12s ease both;
  }
  .bar {
    display: flex;
    align-items: center;
    gap: 2px;
    padding: 4px 6px;
    background: rgba(0, 0, 0, 0.25);
  }
  .label {
    color: rgba(255, 255, 255, 0.7);
    font-size: 11px;
    padding-left: 2px;
  }
  .spacer { flex: 1; }
  .act {
    width: 26px;
    height: 26px;
    border: none;
    background: transparent;
    color: #e6e6e6;
    border-radius: var(--radius-sm, 6px);
    cursor: pointer;
    font-size: 13px;
    transition: background 0.12s, transform 0.08s;
  }
  .act:hover { background: rgba(255, 255, 255, 0.14); }
  .act:active { transform: scale(0.9); }
  .act.close:hover { background: rgba(255, 59, 48, 0.7); }
</style>
