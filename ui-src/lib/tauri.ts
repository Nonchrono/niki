const mutations = new Set(['save_entry', 'save_entry_checked', 'save_journal', 'delete_journal', 'trash_entry', 'restore_entry', 'empty_trash', 'save_template', 'delete_template', 'add_attachment', 'add_inline_image', 'remove_attachment', 'import_data', 'resolve_sync_conflict', 'set_nickname']);
let onMutation: (() => void) | null = null;
export const setMutationListener = (listener: (() => void) | null) => { onMutation = listener; };
export async function invoke<T>(name: string, args: Record<string, unknown> = {}): Promise<T> {
  if (!window.__TAURI__?.core) throw new Error('데스크톱 앱에서 열어 주세요');
  const result = await window.__TAURI__.core.invoke<T>(name, args);
  if (mutations.has(name)) onMutation?.();
  return result;
}
