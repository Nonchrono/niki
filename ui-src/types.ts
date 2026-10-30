export interface Journal { id: string; name: string; color: string; created_at?: string }
export interface Attachment { id: string; name: string; mime: string; bytes: number }
export interface Entry {
  id: string; journal_id: string; title: string; body: string; date: string;
  created_at: string; updated_at: string; favorite: boolean; tags: string[];
  location: string; weather: string; attachments: Attachment[];
  deleted_at?: string | null; external_id?: string | null;
}
export interface Template { id: string; name: string; body: string }
export interface Conflict { id: string; entry_id: string; local: Entry | null; remote: Entry | null }
export interface SyncStatus {
  connected: boolean; has_oauth_client: boolean; client_id?: string | null;
  account_email?: string | null; account_name?: string | null; has_sync_key: boolean;
  sync_paused: boolean; last_sync_at?: string | null; last_sync_error?: string | null;
  blocked_remote_object?: string | null; ignored_remote_count: number;
  pending_upload_count: number; conflict_count: number; pending_resolution_count: number;
}
export interface UpdateInfo {
  update_available: boolean; latest_version: string; current_version: string; notes?: string;
}
export interface Snapshot {
  locked: boolean; lock_enabled: boolean; app_version: string; installed_on_pc: boolean;
  updater_config: { endpoint?: string; auto_check?: boolean; skipped_version?: string };
  nickname: string; journals: Journal[]; entries: Entry[]; templates: Template[];
  sync_conflicts: Conflict[]; pending_resolution_count: number;
  sync_status: SyncStatus; synced_entry_ids: string[];
}
export interface EntryInput {
  id?: string | null; expectedEntry?: Entry | null; journalId: string;
  title: string; body: string; date: string; favorite: boolean; tags: string[];
  location: string; weather: string;
}
export interface SavedEntry { id: string; entry: Entry }

declare global {
  interface Window {
    __TAURI__?: {
      core: { invoke: <T>(name: string, args?: Record<string, unknown>) => Promise<T> };
      event?: { listen: (name: string, callback: () => void) => Promise<() => void> };
    };
  }
}
