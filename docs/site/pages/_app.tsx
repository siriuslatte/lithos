import 'nextra-theme-docs/style.css';
import '../styles.css';
import { MDXProvider } from '@mdx-js/react';
import type { AppProps } from 'next/app';
import { ConfigFormatTab, ConfigFormatTabs } from '../components/config-format-tabs';

const mdxComponents = {
  ConfigFormatTabs,
  ConfigFormatTab,
};

export default function MyApp({ Component, pageProps }: AppProps) {
  return (
    <MDXProvider components={mdxComponents}>
      <Component {...pageProps} />
    </MDXProvider>
  );
}
