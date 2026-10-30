import { forwardRef, useEffect, useImperativeHandle, useRef } from 'react';
import { Editor } from '@tiptap/core';
import StarterKit from '@tiptap/starter-kit';
import { Markdown } from '@tiptap/markdown';
import TaskList from '@tiptap/extension-task-list';
import TaskItem from '@tiptap/extension-task-item';
import Placeholder from '@tiptap/extension-placeholder';
import { imageMarkdown, needsSourceEditor, NikiImage } from './markdown';

export interface RichTextHandle {
  getBody: () => string; format: (kind: string) => void; rememberSelection: () => void;
  insertImage: (id: string, alt: string) => boolean; setEditable: (value: boolean) => void;
  insertTemplate: (text: string) => boolean; source: boolean;
}
interface Props { initialBody: string; onChange: () => void; resolveImage: (id: string) => Promise<string> }

export const RichTextEditor = forwardRef<RichTextHandle, Props>(function RichTextEditor({ initialBody, onChange, resolveImage }, ref) {
  const host = useRef<HTMLDivElement>(null);
  const textarea = useRef<HTMLTextAreaElement>(null);
  const editor = useRef<Editor | null>(null);
  const dirty = useRef(false);
  const imagePosition = useRef<number | null>(null);
  const onChangeRef = useRef(onChange);
  const resolveRef = useRef(resolveImage);
  onChangeRef.current = onChange;
  resolveRef.current = resolveImage;
  const source = needsSourceEditor(initialBody);

  useEffect(() => {
    if (source || !host.current) return;
    const instance = new Editor({
      element: host.current,
      extensions: [StarterKit.configure({ link: { openOnClick: false, autolink: false }, dropcursor: false }), Markdown.configure({ markedOptions: { breaks: true } }), TaskList, TaskItem.configure({ nested: true }), NikiImage.configure({ resolveImage: (id: string) => resolveRef.current(id) }), Placeholder.configure({ placeholder: '오늘의 이야기를 시작해 보세요...' })],
      content: initialBody, contentType: 'markdown', enableInputRules: false, enablePasteRules: false,
      editorProps: { attributes: { class: 'editor-body', role: 'textbox', 'aria-label': '기록 본문', 'aria-multiline': 'true' } },
      onUpdate: () => { dirty.current = true; onChangeRef.current(); },
    });
    editor.current = instance;
    return () => { instance.destroy(); editor.current = null; };
  }, [initialBody, source]);

  useImperativeHandle(ref, () => ({
    source,
    getBody: () => source ? textarea.current?.value ?? initialBody : dirty.current ? editor.current?.getMarkdown().replace(/\n{3,}/g, '\n\n') ?? initialBody : initialBody,
    rememberSelection: () => { imagePosition.current = source ? textarea.current?.selectionStart ?? 0 : editor.current?.state.selection.from ?? 0; },
    insertImage: (id, alt) => {
      if (!/^[0-9a-f]{8}-[0-9a-f-]{27}$/i.test(id)) return false;
      if (source) {
        const field = textarea.current;
        if (!field) return false;
        const position = imagePosition.current ?? field.selectionStart;
        field.setRangeText(imageMarkdown(id, alt), position, position, 'end');
        onChangeRef.current();
      } else {
        const instance = editor.current;
        if (!instance) return false;
        instance.chain().focus().insertContentAt(imagePosition.current ?? instance.state.selection.from, { type: 'nikiImage', attrs: { id, alt } }).run();
      }
      imagePosition.current = null;
      return true;
    },
    setEditable: value => { if (source && textarea.current) textarea.current.disabled = !value; else editor.current?.setEditable(value); },
    format: kind => {
      const chain = editor.current?.chain().focus();
      if (!chain || source) return;
      if (kind === 'bold') chain.toggleBold().run();
      if (kind === 'italic') chain.toggleItalic().run();
      if (kind === 'heading') chain.toggleHeading({ level: 2 }).run();
      if (kind === 'quote') chain.toggleBlockquote().run();
      if (kind === 'list') chain.toggleBulletList().run();
      if (kind === 'check') chain.toggleTaskList().run();
    },
    insertTemplate: text => {
      if (needsSourceEditor(text)) return false;
      if (source) {
        if (!textarea.current) return false;
        textarea.current.value = text + (textarea.current.value ? '\n\n' + textarea.current.value : '');
        onChangeRef.current();
      } else editor.current?.chain().focus().insertContentAt(0, text + '\n\n', { contentType: 'markdown' }).run();
      return true;
    },
  }), [initialBody, source]);

  return source ? <><p className="editor-source-notice">이 기록에는 서식 편집기에서 지원하지 않는 내용이 있어 원문으로 표시합니다.</p><textarea className="editor-source" ref={textarea} defaultValue={initialBody} aria-label="기록 본문" onInput={() => onChangeRef.current()} /></> : <div ref={host} />;
});
