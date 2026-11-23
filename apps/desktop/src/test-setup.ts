import '@testing-library/jest-dom/vitest';
import { cleanup } from '@testing-library/react';
import { createElement, type ComponentType, type ReactNode } from 'react';
import { afterEach, vi } from 'vitest';
import { MantineProvider, Popover } from '@mantine/core';
import { appearance, desktopTheme } from './desktop-theme';

const jsdomTheme = {
  ...desktopTheme,
  components: { Popover: Popover.extend({ defaultProps: { hideDetached: false } }) },
};

if (typeof window !== 'undefined' && !window.matchMedia) {
  window.matchMedia = (query: string) => ({
    media: query, matches: false, onchange: null,
    addListener() {}, removeListener() {}, addEventListener() {}, removeEventListener() {},
    dispatchEvent: () => true,
  });
}

if (typeof window !== 'undefined') {
  if (!window.ResizeObserver) {
    window.ResizeObserver = class {
      observe() {}
      unobserve() {}
      disconnect() {}
    };
  }
  if (!HTMLElement.prototype.scrollIntoView) HTMLElement.prototype.scrollIntoView = () => {};
}

vi.mock('@testing-library/react', async importOriginal => {
  const library = await importOriginal<typeof import('@testing-library/react')>();
  return {
    ...library,
    render: (ui: ReactNode, options?: Parameters<typeof library.render>[1]) => {
      const Wrapper: ComponentType<{ children: ReactNode }> | undefined = options?.wrapper;
      return library.render(ui, {
        ...options,
        wrapper: ({ children }) => createElement(MantineProvider, {
          theme: jsdomTheme, defaultColorScheme: 'light', colorSchemeManager: appearance,
          children: Wrapper ? createElement(Wrapper, { children }) : children,
        }),
      });
    },
  };
});

afterEach(cleanup);
