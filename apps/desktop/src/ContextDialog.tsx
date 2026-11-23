import { Modal, Portal } from '@mantine/core';
import { useLayoutEffect, useRef, type ReactNode } from 'react';

export function ContextDialog({ opened, title, closeLabel, onClose, children }: {
  opened: boolean; title: string; closeLabel: string; onClose: () => void; children: ReactNode;
}) {
  const portal = useRef<HTMLDivElement>(null);

  useLayoutEffect(() => {
    if (!opened) return;
    const opener = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const background = Array.from(document.body.children)
      .filter((element): element is HTMLElement => element instanceof HTMLElement && element !== portal.current)
      .map(element => ({ element, inert: element.inert, hidden: element.getAttribute('aria-hidden') }));

    for (const { element } of background) {
      element.inert = true;
      element.setAttribute('aria-hidden', 'true');
    }
    return () => {
      for (const { element, inert, hidden } of background) {
        element.inert = inert;
        if (hidden === null) element.removeAttribute('aria-hidden');
        else element.setAttribute('aria-hidden', hidden);
      }
      if (opener?.isConnected) opener.focus();
    };
  }, [opened]);

  if (!opened) return null;
  return <Portal ref={portal} reuseTargetNode={false}>
    <Modal opened withinPortal={false} returnFocus={false} onClose={onClose} title={title}
      centered size="620px" transitionProps={{ duration: 0 }}
      closeButtonProps={{ 'aria-label': closeLabel }}
      classNames={{ content: 'context-dialog', header: 'dialog-heading', body: 'dialog-content' }}>
      {children}
    </Modal>
  </Portal>;
}
