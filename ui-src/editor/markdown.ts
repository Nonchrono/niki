import { Node } from '@tiptap/core';
import { marked } from 'marked';

const mediaId = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
export const localImageId = (href: unknown) => typeof href === 'string' && href.startsWith('niki-media:') && mediaId.test(href.slice(11)) ? href.slice(11) : null;
const safeAlt = (text: string) => String(text || '이미지').replace(/[\[\]\r\n]/g, ' ').slice(0, 200);
export const imageMarkdown = (id: string, alt: string) => `![${safeAlt(alt)}](niki-media:${id})`;

export const NikiImage = Node.create({
  name: 'nikiImage', inline: true, group: 'inline', atom: true, selectable: true,
  addOptions() { return { resolveImage: (_id: string): Promise<string> => Promise.reject(new Error('이미지를 불러올 수 없습니다')) }; },
  addAttributes() { return { id: { default: null }, alt: { default: '이미지' } }; },
  parseHTML() { return [{ tag: 'img[data-niki-media]', getAttrs: (element: HTMLElement) => ({ id: element.getAttribute('data-niki-media'), alt: element.getAttribute('alt') }) }]; },
  renderHTML({ node }) { return ['img', { 'data-niki-media': node.attrs.id, alt: node.attrs.alt }]; },
  markdownTokenName: 'image',
  parseMarkdown(token, helpers) {
    const id = localImageId(token.href);
    return id ? helpers.createNode('nikiImage', { id, alt: token.text || '이미지' }) : [];
  },
  renderMarkdown(node) { return imageMarkdown(node.attrs?.id, node.attrs?.alt); },
  addNodeView() { return ({ node }) => {
    const image = document.createElement('img');
    image.dataset.nikiMedia = node.attrs.id;
    image.alt = node.attrs.alt || '이미지';
    this.options.resolveImage(node.attrs.id).then((url: string) => { image.src = url; }).catch(() => { image.title = '이미지를 불러올 수 없습니다'; });
    return { dom: image };
  }; },
});

// Keep unsupported imported Markdown as source so an edit cannot silently discard it.
export function needsSourceEditor(text: string) {
  const tokens = marked.lexer(text, { gfm: true, breaks: true });
  let unsupported = Object.keys(tokens.links || {}).length > 0;
  marked.walkTokens(tokens, token => {
    if (['html', 'table', 'def'].includes(token.type) || token.type === 'image' && !localImageId(token.href)) unsupported = true;
  });
  return unsupported || /\n[ \t]*\n[ \t]*\n/.test(text);
}
export function plainText(text: string): string {
  const read = (tokens: any[]): string => (tokens || []).map(token => {
    const value = token.items ? token.items.map((item: any) => read(item.tokens)).join('\n') : token.tokens ? read(token.tokens) : token.text || (token.type === 'br' ? '\n' : '');
    return value + (['paragraph', 'heading', 'code', 'blockquote', 'list', 'space'].includes(token.type) ? '\n' : '');
  }).join('');
  return read(marked.lexer(text, { gfm: true, breaks: true }) as any[]).trim();
}
