import { useEffect, useRef, type ReactNode } from 'react';
import { createPortal } from 'react-dom';
import { Icon } from './Icons';
export function ContextDialog({ title, closeLabel, onClose, children, wide = false }: { title: string; closeLabel: string; onClose: () => void; children: ReactNode; wide?: boolean }) {
  const root = useRef<HTMLDivElement>(null);
  const close = useRef(onClose);
  close.current = onClose;
  useEffect(() => {
    const opener = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const previousOverflow = document.body.style.overflow;
    document.body.style.overflow = 'hidden';
    const overlay = root.current?.parentElement;
    const background = Array.from(document.body.children).filter((element): element is HTMLElement => element instanceof HTMLElement && element !== overlay).map(element => ({ element, inert: element.inert, hidden: element.getAttribute('aria-hidden') }));
    for (const entry of background) { entry.element.inert = true; entry.element.setAttribute('aria-hidden', 'true'); }
    (root.current?.querySelector<HTMLElement>('[data-dialog-autofocus]') ?? root.current?.querySelector<HTMLElement>('button, input, textarea, select, [tabindex="0"]'))?.focus();
    function key(event: KeyboardEvent) {
      if (event.key === 'Escape') { event.preventDefault(); event.stopPropagation(); close.current(); return; }
      if (event.key !== 'Tab') return;
      const controls = Array.from(root.current?.querySelectorAll<HTMLElement>('button:not(:disabled), input:not(:disabled), textarea:not(:disabled), select:not(:disabled), a[href], summary, [tabindex="0"]') ?? []).filter(element => {
        const closed = element.closest('details:not([open])');
        return !element.closest('[hidden], [aria-hidden="true"]') && (!closed || (element.tagName === 'SUMMARY' && element.parentElement === closed)) && element.getAttribute('type') !== 'hidden';
      });
      if (!controls.length) { event.preventDefault(); root.current?.focus(); return; }
      const first = controls[0], last = controls[controls.length - 1];
      if (event.shiftKey && (document.activeElement === first || !root.current?.contains(document.activeElement))) { event.preventDefault(); last.focus(); }
      else if (!event.shiftKey && (document.activeElement === last || !root.current?.contains(document.activeElement))) { event.preventDefault(); first.focus(); }
    }
    document.addEventListener('keydown', key, true);
    return () => { document.removeEventListener('keydown', key, true); document.body.style.overflow = previousOverflow; for (const entry of background) { entry.element.inert = entry.inert; if (entry.hidden === null) entry.element.removeAttribute('aria-hidden'); else entry.element.setAttribute('aria-hidden', entry.hidden); } if (opener?.isConnected) opener.focus(); };
  }, []);
  return createPortal(<div className="dialog-backdrop" onMouseDown={event => { if (event.target === event.currentTarget) onClose(); }}><div ref={root} className={`context-dialog${wide ? ' wide' : ''}`} role="dialog" aria-modal="true" aria-labelledby="context-dialog-title" tabIndex={-1}><div className="dialog-heading"><h2 id="context-dialog-title">{title}</h2><button className="icon-button" aria-label={closeLabel} onClick={onClose}><Icon name="close" /></button></div><div className="dialog-content">{children}</div></div></div>, document.body);
}
