import { Editor } from '@tiptap/core';
import { marked } from 'marked';
import StarterKit from '@tiptap/starter-kit';
import { Markdown } from '@tiptap/markdown';
import TaskList from '@tiptap/extension-task-list';
import TaskItem from '@tiptap/extension-task-item';
import Placeholder from '@tiptap/extension-placeholder';

// Unsupported structures remain editable as their original source. Never silently
// discard an imported table, image, or HTML when another part of the entry changes.
function needsSourceEditor(text) {
  const tokens=marked.lexer(text,{gfm:true,breaks:true});
  let unsupported=Object.keys(tokens.links || {}).length>0;
  marked.walkTokens(tokens, token=>{
    if(['html','image','table','def'].includes(token.type)) unsupported=true;
  });
  return unsupported || /\n[ \t]*\n[ \t]*\n/.test(text);
}
function plainText(text) {
  const read=tokens=>(tokens||[]).map(token=>{
    let value;
    if(token.items) value=token.items.map(item=>read(item.tokens)).join('\n');
    else if(token.tokens) value=read(token.tokens);
    else value=token.text || (token.type==='br'?'\n':'');
    return value+(['paragraph','heading','code','blockquote','list','space'].includes(token.type)?'\n':'');
  }).join('');
  return read(marked.lexer(text,{gfm:true,breaks:true})).trim();
}

function create(element, markdown, onChange) {
  if (needsSourceEditor(markdown)) {
    const notice=document.createElement('p');
    notice.className='editor-source-notice';
    notice.textContent='이 기록에는 서식 편집기에서 지원하지 않는 내용이 있어 원문으로 표시합니다.';
    const textarea=document.createElement('textarea');
    textarea.className='editor-source'; textarea.value=markdown;
    textarea.setAttribute('aria-label','기록 본문');
    textarea.addEventListener('input',()=>onChange());
    element.append(notice,textarea);
    return { getBody:()=>textarea.value, format:()=>{}, insertTemplate: text=>{ textarea.value=text+(textarea.value?'\n\n'+textarea.value:''); onChange(); }, destroy:()=>{}, source:true };
  }
  let dirty=false;
  const editor=new Editor({
    element,
    extensions:[StarterKit.configure({link:{openOnClick:false,autolink:false},dropcursor:false}),Markdown.configure({markedOptions:{breaks:true}}),TaskList,TaskItem.configure({nested:true}),Placeholder.configure({placeholder:'오늘의 이야기를 시작해 보세요...'})],
    content:markdown,contentType:'markdown',enableInputRules:false,enablePasteRules:false,
    editorProps:{attributes:{class:'editor-body',role:'textbox','aria-label':'기록 본문','aria-multiline':'true'}},
    onUpdate:()=>{dirty=true;onChange();},
    onTransaction:({editor})=>{
      const names={bold:'bold',italic:'italic',heading:'heading',quote:'blockquote',list:'bulletList',check:'taskList'};
      element.parentElement?.querySelectorAll('[data-format]').forEach(button=>{const active=editor.isActive(names[button.dataset.format]);button.classList.toggle('active',active);button.setAttribute('aria-pressed',String(active));});
    },
  });
  return {
    getBody:()=>dirty?editor.getMarkdown():markdown,
    format(kind){
      const chain=editor.chain().focus();
      const commands={bold:()=>chain.toggleBold(),italic:()=>chain.toggleItalic(),heading:()=>chain.toggleHeading({level:2}),quote:()=>chain.toggleBlockquote(),list:()=>chain.toggleBulletList(),check:()=>chain.toggleTaskList()};
      commands[kind]?.().run();
    },
    insertTemplate(text){
      // Do not drop content that the visual editor cannot represent.
      if(needsSourceEditor(text)) return false;
      const position=0;
      editor.chain().focus().insertContentAt(position,text+'\n\n',{contentType:'markdown'}).run();
      return true;
    },
    destroy:()=>editor.destroy(),
    source:false,
  };
}
window.NikiEditor={create,needsSourceEditor,plainText};
