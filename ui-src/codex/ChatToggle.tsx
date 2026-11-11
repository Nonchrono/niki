import { Button } from '../components/ui/button';
import { useChat } from './ChatContext';

export function ChatToggle() {
  const { connected, open, toggle } = useChat();
  if (!connected || open) return null;
  return <Button variant="default" size="icon" className="codex-fab" data-action="open-summary"
    title="Niki와 대화" aria-label="Niki와 대화" aria-expanded={open} aria-controls="diary-summary-sidebar" onClick={toggle}>
    <svg className="size-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinejoin="round" aria-hidden="true">
      <path d="m10 5 2.2 5.8L18 13l-5.8 2.2L10 21l-2.2-5.8L2 13l5.8-2.2Z" />
      <path d="m19 1 1.1 2.9L23 5l-2.9 1.1L19 9l-1.1-2.9L15 5l2.9-1.1Z" />
      <path d="m20 16 .8 2.2L23 19l-2.2.8L20 22l-.8-2.2L17 19l2.2-.8Z" />
    </svg>
  </Button>;
}
