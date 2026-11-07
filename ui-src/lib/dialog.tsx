import { useEffect, useRef, useState } from 'react';
import { Button } from '../components/ui/button';
import { Input } from '../components/ui/input';

// In-app replacements for window.confirm / window.prompt, which the macOS
// WebView does not show. Both return promises; calls queue up one at a time.
interface Request {
  kind: 'confirm' | 'text';
  message: string;
  okLabel: string;
  cancelLabel: string;
  danger: boolean;
  multiline: boolean;
  value: string;
  resolve: (value: boolean | string | null) => void;
}
type Options = { okLabel?: string; cancelLabel?: string; danger?: boolean };

const queue: Request[] = [];
let listener: (() => void) | null = null;
const push = (request: Request) => { queue.push(request); listener?.(); };

export const askConfirm = (message: string, options: Options = {}) => new Promise<boolean>(resolve => push({
  kind: 'confirm', message, okLabel: options.okLabel ?? '확인', cancelLabel: options.cancelLabel ?? '취소',
  danger: options.danger ?? false, multiline: false, value: '', resolve: value => resolve(value === true),
}));

export const askText = (message: string, defaultValue = '', options: { multiline?: boolean } = {}) => new Promise<string | null>(resolve => push({
  kind: 'text', message, okLabel: '확인', cancelLabel: '취소', danger: false,
  multiline: options.multiline ?? false, value: defaultValue, resolve: value => resolve(typeof value === 'string' ? value : null),
}));

export function DialogHost() {
  const [, setTick] = useState(0);
  const [text, setText] = useState('');
  const current = queue[0];
  const lastShown = useRef<Request | undefined>(undefined);
  useEffect(() => { listener = () => setTick(value => value + 1); return () => { listener = null; }; }, []);
  useEffect(() => {
    if (current && lastShown.current !== current) { lastShown.current = current; setText(current.value); }
  }, [current]);
  if (!current) return null;
  const finish = (result: boolean | string | null) => { queue.shift(); current.resolve(result); setTick(value => value + 1); };
  const ok = () => finish(current.kind === 'confirm' ? true : text);
  const cancel = () => finish(current.kind === 'confirm' ? false : null);
  return <div className="dialog-backdrop" role="presentation" onKeyDown={event => { if (event.key === 'Escape') cancel(); }}>
    <div className="dialog-card" role="dialog" aria-modal="true" aria-label={current.message}>
      <p className="dialog-message">{current.message}</p>
      {current.kind === 'text' && (current.multiline
        ? <textarea className="dialog-input" autoFocus rows={6} value={text} onChange={event => setText(event.target.value)} />
        : <Input className="dialog-input" autoFocus value={text} onChange={event => setText(event.target.value)}
          onKeyDown={event => { if (event.key === 'Enter' && !event.nativeEvent.isComposing) ok(); }} />)}
      <div className="dialog-actions">
        <Button variant="outline" size="sm" data-action="dialog-cancel" onClick={cancel}>{current.cancelLabel}</Button>
        <Button variant={current.danger ? 'destructive' : 'default'} size="sm" data-action="dialog-ok" autoFocus={current.kind === 'confirm'} onClick={ok}>{current.okLabel}</Button>
      </div>
    </div>
  </div>;
}
