import { useEffect, useRef, type ReactNode } from 'react';

export function Modal({ title, close, children }: { title: string; close: () => void; children: ReactNode }) {
  const ref = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    const dialog = ref.current;
    const previous = document.activeElement as HTMLElement | null;
    dialog?.showModal();
    return () => { dialog?.close(); previous?.focus(); };
  }, []);
  return <dialog ref={ref} className="chatgpt-dialog" aria-label={title} onCancel={event => { event.preventDefault(); close(); }}>
    <h2>{title}</h2>{children}
  </dialog>;
}
