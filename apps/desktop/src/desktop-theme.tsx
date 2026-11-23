import { createTheme, localStorageColorSchemeManager, MantineProvider } from '@mantine/core';
import type { ReactNode } from 'react';
import '@mantine/core/styles.css';

const theme = createTheme({
  fontFamily: 'system-ui, -apple-system, BlinkMacSystemFont, "PingFang SC", "Noto Sans CJK SC", sans-serif',
  fontSizes: { xs: '12px', sm: '13px', md: '14px', lg: '18px', xl: '24px' },
  primaryColor: 'teal',
  primaryShade: 9,
  autoContrast: true,
  defaultRadius: 'sm',
  spacing: { xs: '8px', sm: '12px', md: '16px', lg: '24px', xl: '32px' },
  breakpoints: { xs: '37.5em' },
  headings: {
    fontWeight: '600',
    sizes: {
      h1: { fontSize: '24px', lineHeight: '1.35' },
      h2: { fontSize: '18px', lineHeight: '1.4' },
      h3: { fontSize: '14px', lineHeight: '1.45' },
      h4: { fontSize: '14px', lineHeight: '1.45' },
    },
  },
});

const appearance = localStorageColorSchemeManager({ key: 'agentbaton.appearance' });

export function DesktopTheme({ children }: { children: ReactNode }) {
  return <MantineProvider theme={theme} defaultColorScheme="light" colorSchemeManager={appearance}>
    {children}
  </MantineProvider>;
}
