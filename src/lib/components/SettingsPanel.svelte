<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';

  interface Props {
    onclose?: () => void;
  }

  let { onclose }: Props = $props();

  // ── Shortcut state ─────────────────────────────────────────────────
  interface ShortcutDef { mods: string[]; key: string; display: string; }

  const STORAGE_KEY = 'snapx-shortcut';
  const DEFAULT_SHORTCUT: ShortcutDef = { mods: ['Ctrl', 'Shift'], key: 'X', display: 'Ctrl+Shift+X' };

  function loadSaved(): ShortcutDef {
    try {
      const raw = localStorage.getItem(STORAGE_KEY);
      if (raw) return JSON.parse(raw) as ShortcutDef;
    } catch (_) {}
    return { ...DEFAULT_SHORTCUT };
  }

  let current = $state<ShortcutDef>(loadSaved());
  let pending = $state<ShortcutDef | null>(null);
  let recording = $state(false);
  let saveStatus = $state<'idle' | 'saving' | 'ok' | 'error'>('idle');
  let saveError = $state('');

  function startRecording() {
    recording = true;
    pending = null;
  }

  function cancelRecording() {
    recording = false;
    pending = null;
  }

  function onRecordKey(e: KeyboardEvent) {
    if (!recording) return;
    e.preventDefault();
    e.stopPropagation();

    // Ignore pure modifier presses
    if (['Control', 'Shift', 'Alt', 'Meta', 'Super'].includes(e.key)) return;

    if (e.key === 'Escape') { cancelRecording(); return; }

    const mods: string[] = [];
    if (e.ctrlKey)  mods.push('Ctrl');
    if (e.shiftKey) mods.push('Shift');
    if (e.altKey)   mods.push('Alt');
    if (e.metaKey)  mods.push('Super');

    // Normalize key to uppercase letter / digit / Fn
    let key = e.code;
    if (key.startsWith('Key'))   key = key.slice(3).toUpperCase();
    else if (key.startsWith('Digit')) key = key.slice(5);
    else if (/^F\d+$/.test(key)) { /* keep as-is */ }
    else { return; } // unsupported key

    const display = [...mods, key].join('+');
    pending = { mods, key, display };
    recording = false;
  }

  async function saveShortcut() {
    if (!pending) return;
    saveStatus = 'saving';
    try {
      await invoke('update_global_shortcut', { mods: pending.mods, key: pending.key });
      localStorage.setItem(STORAGE_KEY, JSON.stringify(pending));
      current = { ...pending };
      pending = null;
      saveStatus = 'ok';
      setTimeout(() => { saveStatus = 'idle'; }, 1800);
    } catch (err) {
      saveError = String(err);
      saveStatus = 'error';
    }
  }

  function resetToDefault() {
    pending = { ...DEFAULT_SHORTCUT };
  }
</script>

<svelte:window onkeydown={onRecordKey} />

<div class="min-h-screen bg-[#1a1a1a] text-white flex flex-col">
  <!-- Header -->
  <div class="flex items-center justify-between px-6 py-4 border-b border-white/10">
    <div class="flex items-center gap-2">
      <svg class="w-5 h-5 text-violet-400" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <circle cx="12" cy="12" r="3"/><path d="M19.07 4.93a10 10 0 0 1 0 14.14M4.93 4.93a10 10 0 0 0 0 14.14"/>
      </svg>
      <span class="font-semibold text-lg">SnapX 设置</span>
    </div>
    {#if onclose}
      <button
        class="w-8 h-8 flex items-center justify-center rounded-lg text-gray-400 hover:text-white hover:bg-white/10 transition-colors"
        onclick={onclose}
      >✕</button>
    {/if}
  </div>

  <div class="flex-1 overflow-y-auto p-6 space-y-8 max-w-lg">

    <!-- ── Shortcut section ─────────────────────────────────── -->
    <section>
      <h2 class="text-sm font-semibold text-white/50 uppercase tracking-wider mb-4">快捷键</h2>

      <div class="bg-white/5 rounded-xl p-4 space-y-4 border border-white/8">
        <div class="flex items-center justify-between">
          <div>
            <div class="text-sm font-medium">截图快捷键</div>
            <div class="text-xs text-white/40 mt-0.5">触发区域截图的全局快捷键</div>
          </div>
          <!-- Current shortcut badge -->
          <div class="flex gap-1">
            {#each current.mods as mod}
              <kbd class="px-2 py-0.5 bg-white/10 rounded text-xs font-mono border border-white/15">{mod}</kbd>
            {/each}
            <kbd class="px-2 py-0.5 bg-white/10 rounded text-xs font-mono border border-white/15">{current.key}</kbd>
          </div>
        </div>

        <!-- Recording area -->
        {#if recording}
          <div class="flex items-center gap-3 bg-violet-500/15 border border-violet-500/40 rounded-lg px-4 py-3">
            <span class="w-2 h-2 rounded-full bg-violet-400 animate-pulse"></span>
            <span class="text-sm text-violet-300">请按下新的快捷键组合…</span>
            <button
              class="ml-auto text-xs text-white/40 hover:text-white transition-colors"
              onclick={cancelRecording}
            >取消 (ESC)</button>
          </div>
        {:else if pending}
          <div class="flex items-center justify-between bg-white/5 rounded-lg px-4 py-2.5">
            <div class="flex items-center gap-2">
              <span class="text-xs text-white/50">新快捷键：</span>
              <div class="flex gap-1">
                {#each pending.mods as mod}
                  <kbd class="px-2 py-0.5 bg-emerald-500/20 rounded text-xs font-mono border border-emerald-500/30 text-emerald-300">{mod}</kbd>
                {/each}
                <kbd class="px-2 py-0.5 bg-emerald-500/20 rounded text-xs font-mono border border-emerald-500/30 text-emerald-300">{pending.key}</kbd>
              </div>
            </div>
            <div class="flex gap-2">
              <button
                class="text-xs text-white/40 hover:text-white"
                onclick={() => pending = null}
              >重新录制</button>
            </div>
          </div>
        {/if}

        <!-- Action buttons -->
        <div class="flex items-center gap-2 pt-1">
          {#if !recording}
            <button
              class="flex-1 py-2 text-sm rounded-lg bg-white/8 hover:bg-white/12 transition-colors border border-white/10"
              onclick={startRecording}
            >点击录制新快捷键</button>
          {/if}

          <button
            class="py-2 px-3 text-xs rounded-lg text-white/40 hover:text-white/70 transition-colors"
            title="恢复默认 (Ctrl+Shift+X)"
            onclick={resetToDefault}
          >恢复默认</button>

          {#if pending}
            <button
              class="py-2 px-4 text-sm rounded-lg font-medium transition-colors
                {saveStatus === 'saving' ? 'bg-violet-500/50 cursor-wait' : 'bg-violet-500 hover:bg-violet-600'}"
              onclick={saveShortcut}
              disabled={saveStatus === 'saving'}
            >
              {saveStatus === 'saving' ? '保存中…' : '保存'}
            </button>
          {/if}
        </div>

        {#if saveStatus === 'ok'}
          <div class="text-xs text-emerald-400">快捷键已更新</div>
        {:else if saveStatus === 'error'}
          <div class="text-xs text-red-400">保存失败：{saveError}</div>
        {/if}
      </div>
    </section>

    <!-- ── About section ───────────────────────────────────── -->
    <section>
      <h2 class="text-sm font-semibold text-white/50 uppercase tracking-wider mb-4">关于</h2>
      <div class="bg-white/5 rounded-xl p-4 border border-white/8 space-y-1.5">
        <div class="flex justify-between text-sm">
          <span class="text-white/50">版本</span>
          <span class="font-mono text-white/80">0.1.0</span>
        </div>
        <div class="flex justify-between text-sm">
          <span class="text-white/50">技术栈</span>
          <span class="text-white/80">Tauri v2 · Svelte 5 · Rust</span>
        </div>
      </div>
    </section>
  </div>
</div>
