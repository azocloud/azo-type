import { Inter } from 'next/font/google';
import type { Metadata } from 'next';
import { Provider } from '@/components/provider';
import './global.css';

const inter = Inter({
  subsets: ['latin'],
});

export const metadata: Metadata = {
  title: {
    default: 'azo-type — Native Typst IDE with P2P Collaboration',
    template: '%s | azo-type',
  },
  description:
    'A cross-platform native Rust desktop IDE for Typst with live incremental preview, AST outline navigation, standards-compliant PDF export, and zero-server P2P collaboration.',
};

export default function Layout({ children }: LayoutProps<'/'>) {
  return (
    <html lang="en" className={inter.className} suppressHydrationWarning>
      <body className="flex flex-col min-h-screen">
        <Provider>{children}</Provider>
      </body>
    </html>
  );
}
