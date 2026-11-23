import '@testing-library/jest-dom/vitest';
import { cleanup } from '@testing-library/react';
import { createElement, type ComponentType, type ReactNode } from 'react';
import { afterEach, vi } from 'vitest';
import { DesktopTheme } from './desktop-theme';

if (typeof window !== 'undefined' && !window.matchMedia) {
  window.matchMedia = (query: string) => ({
    media: query, matches: false, onchange: null,
    addListener() {}, removeListener() {}, addEventListener() {}, removeEventListener() {},
    dispatchEvent: () => true,
  });
}

vi.mock('@testing-library/react', async importOriginal => {
  const library = await importOriginal<typeof import('@testing-library/react')>();
  return {
    ...library,
    render: (ui: ReactNode, options?: Parameters<typeof library.render>[1]) => {
      const Wrapper: ComponentType<{ children: ReactNode }> | undefined = options?.wrapper;
      return library.render(ui, {
        ...options,
        wrapper: ({ children }) => createElement(DesktopTheme, { children: Wrapper ? createElement(Wrapper, { children }) : children }),
      });
    },
  };
});

afterEach(cleanup);
