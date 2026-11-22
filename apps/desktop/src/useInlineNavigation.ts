import { useCallback, useLayoutEffect, useRef, useState } from 'react';

export type InlinePage = 'projects' | 'execution' | 'search' | 'settings' | 'conversion' | 'standalone';
type Destination = { opener: HTMLElement | null; scrollTop: number };
type Visit = { page: InlinePage; from: Destination };

export function useInlineNavigation() {
  const [visits, setVisits] = useState<Visit[]>([]);
  const active = visits.at(-1)?.page ?? null;
  const scrollRef = useRef<HTMLDivElement>(null);
  const returning = useRef<Destination | null>(null);

  const focusActivePage = useCallback(() => {
    const main = scrollRef.current;
    const page = main?.querySelector<HTMLElement>('.inline-page:not([hidden])');
    const target = page?.querySelector<HTMLElement>('[data-page-autofocus]') ?? page?.querySelector('h1') ?? main;
    target?.focus({ preventScroll: true });
  }, []);

  const open = useCallback((page: InlinePage, opener?: HTMLElement) => {
    if (page === active) {
      focusActivePage();
      return;
    }
    returning.current = null;
    const previous = visits.findIndex(entry => entry.page === page);
    if (previous !== -1) {
      setVisits(visits.slice(0, previous + 1));
      return;
    }
    const focused = document.activeElement;
    setVisits([...visits, {
      page, from: {
        opener: opener ?? (focused instanceof HTMLElement ? focused : null),
        scrollTop: scrollRef.current?.scrollTop ?? 0,
      },
    }]);
  }, [active, visits, focusActivePage]);

  const back = useCallback(() => {
    const current = visits.at(-1);
    if (!current) return;
    returning.current = current.from;
    setVisits(visits.slice(0, -1));
  }, [visits]);

  const backToBase = useCallback(() => {
    if (!visits.length) return;
    returning.current = visits[0].from;
    setVisits([]);
  }, [visits]);

  const clear = useCallback(() => {
    returning.current = null;
    setVisits(current => current.length ? [] : current);
  }, []);

  useLayoutEffect(() => {
    const destination = returning.current;
    returning.current = null;
    if (scrollRef.current) scrollRef.current.scrollTop = destination?.scrollTop ?? 0;
    if (destination?.opener?.isConnected && !destination.opener.closest('[hidden]')) {
      destination.opener.focus({ preventScroll: true });
      if (document.activeElement === destination.opener) return;
    }
    if (active || destination) focusActivePage();
  }, [active, focusActivePage]);

  return { active, visits, open, back, backToBase, clear, scrollRef };
}
