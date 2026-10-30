import { useEffect, useRef, useState, type ChangeEvent } from 'react';
import { useApp } from '../app/AppContext';
import { errorMessage, fileBase64 } from '../lib/format';
import type { Entry, EntryInput } from '../types';
import { AttachmentPreview } from './AttachmentPreview';
import { DatePicker } from './DatePicker';
import { RichTextEditor, type RichTextHandle } from './RichTextEditor';
import { needsSourceEditor } from './markdown';

type Fields = Pick<EntryInput, 'title' | 'date' | 'journalId' | 'favorite' | 'tags' | 'location' | 'weather'>;
const fieldsFrom = (entry: Entry): Fields => ({ title: entry.title, date: entry.date, journalId: entry.journal_id, favorite: entry.favorite, tags: entry.tags, location: entry.location, weather: entry.weather });
const draftKey = (id: string) => `niki.editor.draft.${id}`;
function initialDraft(entry: Entry): { fields: Fields; body: string; expected: Entry; restored: boolean } {
  try {
    const raw = localStorage.getItem(draftKey(entry.id));
    if (raw) {
      const draft = JSON.parse(raw) as EntryInput;
      if (draft.id === entry.id && typeof draft.body === 'string' && draft.expectedEntry) {
        if (confirm('이 기록에 저장되지 않은 편집 내용이 있습니다. 복원할까요? 취소하면 임시 편집 내용이 삭제됩니다.')) return { fields: { title: draft.title, date: draft.date, journalId: draft.journalId, favorite: draft.favorite, tags: draft.tags, location: draft.location, weather: draft.weather }, body: draft.body, expected: draft.expectedEntry, restored: true };
        localStorage.removeItem(draftKey(entry.id));
      }
    }
  } catch (error) { console.warn('Draft could not be read:', error); }
  return { fields: fieldsFrom(entry), body: entry.body, expected: structuredClone(entry), restored: false };
}

export function EntryEditor({ entry }: { entry: Entry }) {
  const { data, run, refresh, action, toast, registerLeave, go, media, clearMedia } = useApp();
  const [seed, setSeed] = useState(() => initialDraft(entry));
  const [fields, setFields] = useState<Fields>(seed.fields);
  const [tagInput, setTagInput] = useState(seed.fields.tags.join(', '));
  const fieldsRef = useRef(fields);
  const expected = useRef(seed.expected);
  const editor = useRef<RichTextHandle>(null);
  const [status, setStatus] = useState(seed.restored ? '저장 대기 중…' : '모든 변경 사항 저장됨');
  const [conflict, setConflict] = useState(false);
  const [menu, setMenu] = useState(false);
  const [attachments, setAttachments] = useState(entry.attachments);
  const [uploading, setUploading] = useState(false);
  const attachInput = useRef<HTMLInputElement>(null);
  const imageInput = useRef<HTMLInputElement>(null);
  const dirty = useRef(seed.restored);
  const saveTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const savePromise = useRef<Promise<boolean> | null>(null);
  const uploadPromise = useRef<Promise<void> | null>(null);
  const draftWarning = useRef(false);

  const input = (): EntryInput => ({ id: entry.id, expectedEntry: expected.current, ...fieldsRef.current, body: editor.current?.getBody() ?? seed.body });
  const persist = () => { try { localStorage.setItem(draftKey(entry.id), JSON.stringify(input())); } catch { if (!draftWarning.current) { draftWarning.current = true; toast('편집 내용의 임시 저장에 실패했습니다. 앱을 닫기 전에 저장 상태를 확인하세요.', true); } } };
  const clearDraft = () => { try { localStorage.removeItem(draftKey(entry.id)); } catch (error) { console.warn(error); } };
  const changed = () => {
    dirty.current = true; setStatus('저장 대기 중…'); persist();
    if (saveTimer.current) clearTimeout(saveTimer.current);
    saveTimer.current = setTimeout(() => { void saveCurrent(); }, 550);
  };
  const updateField = <K extends keyof Fields>(key: K, value: Fields[K]) => {
    const next = { ...fieldsRef.current, [key]: value };
    fieldsRef.current = next; setFields(next); changed();
  };
  const saveCurrent = async (duringUpload = false): Promise<boolean> => {
    if (uploadPromise.current && !duringUpload) { try { await uploadPromise.current; } catch { return false; } }
    if (saveTimer.current) { clearTimeout(saveTimer.current); saveTimer.current = null; }
    if (savePromise.current && !await savePromise.current) return false;
    while (dirty.current) {
      const draft = input();
      dirty.current = false; setStatus('저장 중…');
      savePromise.current = (async () => {
        try {
          const saved = await run<{ id: string; entry: Entry }>('save_entry_checked', { input: draft });
          expected.current = saved.entry;
          if (dirty.current) persist(); else clearDraft();
          await refresh();
          setStatus('모든 변경 사항 저장됨'); setConflict(false);
          return true;
        } catch (error) {
          dirty.current = true;
          persist();
          const stale = errorMessage(error).includes('다른 기기에서 이 기록이 변경');
          setConflict(stale); setStatus(stale ? '기록이 변경되어 저장할 수 없습니다' : '저장 실패');
          toast(errorMessage(error), true);
          return false;
        }
      })();
      const ok = await savePromise.current;
      savePromise.current = null;
      if (!ok) return false;
    }
    return true;
  };
  const leave = async () => {
    if (uploadPromise.current) { try { await uploadPromise.current; } catch { return false; } }
    return dirty.current || savePromise.current || saveTimer.current ? saveCurrent() : true;
  };
  // Register once; callbacks read refs and are refreshed on every render below.
  const leaveRef = useRef(leave); leaveRef.current = leave;
  useEffect(() => { registerLeave(() => leaveRef.current()); return () => registerLeave(null); }, [registerLeave]);
  useEffect(() => { if (seed.restored) saveTimer.current = setTimeout(() => { void saveCurrent(); }, 550); }, []);
  useEffect(() => () => { if (saveTimer.current) clearTimeout(saveTimer.current); }, []);
  useEffect(() => {
    if (entry.updated_at === expected.current.updated_at || dirty.current || savePromise.current || document.querySelector('.editor-inner')?.contains(document.activeElement)) return;
    const next = initialDraft(entry);
    expected.current = next.expected; fieldsRef.current = next.fields;
    setFields(next.fields); setTagInput(next.fields.tags.join(', ')); setSeed(next); setAttachments(entry.attachments);
  }, [entry.updated_at]);

  const insertImage = (event: ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0]; event.target.value = '';
    if (!file) return;
    if (file.size > 20_000_000) { toast('본문 이미지는 20 MB 이하만 넣을 수 있습니다', true); return; }
    const task = (async () => {
      if (!await saveCurrent(true)) throw Error('기존 편집 내용을 먼저 저장해야 합니다');
      editor.current?.setEditable(false); setUploading(true);
      try {
        const encoded = await fileBase64(file);
        const added = await run<{ attachment: Entry['attachments'][number]; entry: Entry }>('add_inline_image', { entryId: entry.id, name: file.name, mime: file.type || 'application/octet-stream', data: encoded });
        expected.current = added.entry;
        setAttachments(added.entry.attachments);
        editor.current?.setEditable(true);
        if (!editor.current?.insertImage(added.attachment.id, added.attachment.name)) throw Error('이미지는 첨부됐지만 본문에 넣지 못했습니다. 첨부 목록에서 확인하세요');
        if (!await saveCurrent(true)) throw Error('이미지 첨부는 저장됐지만 본문 저장을 확인해야 합니다');
        await refresh(); toast('본문에 이미지를 넣었습니다');
      } finally { editor.current?.setEditable(true); setUploading(false); }
    })();
    uploadPromise.current = task;
    void task.catch(error => toast(errorMessage(error), true)).finally(() => { uploadPromise.current = null; });
  };
  const attachFiles = (event: ChangeEvent<HTMLInputElement>) => { const files = Array.from(event.target.files || []); event.target.value = ''; void action(async () => { if (!await leave()) throw Error('기존 편집 내용을 저장하지 못했습니다'); for (const file of files) await run('add_attachment', { entryId: entry.id, name: file.name, mime: file.type || 'application/octet-stream', data: await fileBase64(file) }); const next = await refresh(); const updated = next.entries.find(item => item.id === entry.id); if (updated) { expected.current = updated; setAttachments(updated.attachments); } toast('첨부 파일 저장됨'); }); };
  const copyDraft = () => void action(async () => { const draft = input(); const newId = await run<string>('save_entry', { input: { ...draft, id: null, expectedEntry: null } }); clearDraft(); dirty.current = false; setConflict(false); await refresh(); await go(`/journals/${draft.journalId}/entries/${newId}`); toast('편집 내용을 별도 기록으로 저장했습니다'); });
  const syncBadge = data?.sync_conflicts.some(item => item.entry_id === entry.id) ? ' · ⚠ 동기화 충돌' : data?.sync_status.connected ? data.sync_status.sync_paused ? ' · ⏸ 동기화 일시 중지됨' : data.synced_entry_ids.includes(entry.id) ? ' · ☁ Drive 동기화됨' : ' · ↻ 동기화 대기 중' : ' · 로컬 저장';

  return <div className="editor-inner"><div className="editor-top"><span className="status" id="save-status">{status}{status === '모든 변경 사항 저장됨' ? syncBadge : ''}</span><span className="spacer" />{conflict && <button className="ghost" data-action="save-draft-copy" onClick={copyDraft}>내 편집 내용 별도 저장</button>}<button className="icon-button" data-action="favorite-entry" title="즐겨찾기" onClick={() => updateField('favorite', !fields.favorite)}>{fields.favorite ? '♥' : '♡'}</button><button className="ghost" data-action="print" onClick={() => void action(async () => { if (await leave()) window.print(); })}>인쇄 / PDF</button><div className="entry-menu-anchor"><button className="icon-button" data-action="entry-menu" title="기록 메뉴" aria-label="기록 메뉴" aria-haspopup="menu" onClick={() => setMenu(!menu)}>⋯</button>{menu && <div className="context-menu" role="menu"><button className="context-menu-item danger" role="menuitem" data-action="trash-entry" onClick={() => void action(async () => { setMenu(false); if (!confirm('이 기록을 휴지통으로 이동할까요?')) return; if (!await leave()) throw Error('편집 내용을 저장하지 못했습니다'); await run('trash_entry', { id: entry.id }); await refresh(); await go('/all'); })}>× 휴지통으로 이동</button></div>}</div></div>
    <input className="editor-title" id="entry-title" placeholder="제목 없는 기록" value={fields.title} onChange={event => updateField('title', event.target.value)} />
    <div className="editor-meta"><DatePicker value={fields.date} onChange={value => updateField('date', value)} /><span>◈ <select id="entry-journal" aria-label="일기장" value={fields.journalId} onChange={event => updateField('journalId', event.target.value)}>{data?.journals.map(journal => <option key={journal.id} value={journal.id}>{journal.name}</option>)}</select></span><span>⌗ <input id="entry-tags" placeholder="태그, 쉼표 구분" value={tagInput} onChange={event => { setTagInput(event.target.value); updateField('tags', event.target.value.split(',').map(tag => tag.trim()).filter(Boolean)); }} /></span></div>
    <div className="toolbar" aria-label="본문 서식">{[['bold', 'B', '굵게'], ['italic', 'I', '기울임'], ['heading', 'H', '제목'], ['quote', '❞', '인용'], ['list', '☷', '글머리 목록'], ['check', '☑', '체크리스트']].map(([kind, label, title]) => <button key={kind} className="tool" data-action="format" data-format={kind} title={title} aria-label={title} disabled={needsSourceEditor(seed.body)} onMouseDown={event => event.preventDefault()} onClick={() => editor.current?.format(kind)}>{label}</button>)}<span className="tool sep" /><button className="tool image-tool" data-action="insert-image" title="본문에 이미지 넣기" aria-label="본문에 이미지 넣기" disabled={uploading} onClick={() => { editor.current?.rememberSelection(); imageInput.current?.click(); }}>▧ 이미지</button><button className="tool" data-action="attach" title="첨부" aria-label="첨부" onClick={() => attachInput.current?.click()}>📎</button><select id="insert-template" className="filter-select" aria-label="템플릿 삽입" defaultValue="" onChange={event => { const template = data?.templates.find(item => item.id === event.target.value); if (template && !editor.current?.insertTemplate(template.body)) toast('이 템플릿은 현재 서식 편집기에서 지원하지 않습니다.', true); event.target.value = ''; }}><option value="">템플릿</option>{data?.templates.map(template => <option key={template.id} value={template.id}>{template.name}</option>)}</select></div>
    <RichTextEditor key={seed.expected.updated_at + seed.body} ref={editor} initialBody={seed.body} onChange={changed} resolveImage={media} />
    <div className="attach-grid">{attachments.map(attachment => <AttachmentPreview key={attachment.id} attachment={attachment} onRemove={() => void action(async () => { if (!confirm('첨부 파일을 삭제할까요?')) return; if (!await leave()) throw Error('편집 내용을 저장하지 못했습니다'); await run('remove_attachment', { entryId: entry.id, attachmentId: attachment.id }); clearMedia(attachment.id); const next = await refresh(); const updated = next.entries.find(item => item.id === entry.id); if (updated) { expected.current = updated; setAttachments(updated.attachments); setSeed({ fields: fieldsRef.current, body: updated.body, expected: updated, restored: false }); } })} />)}</div>
    <div className="extra-fields"><label>장소<input id="entry-location" placeholder="장소" value={fields.location} onChange={event => updateField('location', event.target.value)} /></label><label>날씨 / 메모<input id="entry-weather" placeholder="날씨" value={fields.weather} onChange={event => updateField('weather', event.target.value)} /></label></div>
    <input id="attach-file" ref={attachInput} type="file" accept="image/*,video/*,audio/*,.pdf" multiple hidden onChange={attachFiles} /><input id="inline-image-file" ref={imageInput} type="file" accept="image/png,image/jpeg,image/gif,image/webp" hidden onChange={insertImage} />
  </div>;
}
