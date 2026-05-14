import type { ReactNode } from 'react';
import Link from 'next/link';
import { ArrowRight, FileText, MessageCircle } from 'react-feather';

interface PageHeroProps {
  section: string;
}

/**
 * Section pill shown above the H1 on docs pages.
 *
 * Use this at the very top of each MDX page so every page has a
 * consistent breadcrumb-style chip identifying its section.
 */
export function PageHero({ section }: PageHeroProps) {
  return (
    <div className="lithos-doc-hero">
      <span className="lithos-pill">{section}</span>
    </div>
  );
}

interface FilePathProps {
  children: ReactNode;
}

/**
 * Inline display of a file path (e.g. `./lithos.yml`) styled like the
 * little file pill in the reference layout. Use right under the section
 * heading that introduces a config file.
 */
export function FilePath({ children }: FilePathProps) {
  return (
    <span className="lithos-file-path">
      <FileText size={14} strokeWidth={2.2} aria-hidden="true" />
      <span>{children}</span>
    </span>
  );
}

interface NextStep {
  href: string;
  title: string;
  description: string;
}

interface NextStepsProps {
  items: NextStep[];
}

/**
 * Grid of "next steps" cards usually placed at the bottom of a page.
 */
export function NextSteps({ items }: NextStepsProps) {
  return (
    <ul className="lithos-next-steps">
      {items.map((item) => (
        <li key={item.href}>
          <Link href={item.href}>
            <span className="lithos-next-title">
              {item.title}
              <ArrowRight
                size={14}
                strokeWidth={2.2}
                style={{ marginLeft: '0.4rem', verticalAlign: 'middle' }}
              />
            </span>
            <span className="lithos-next-desc">{item.description}</span>
          </Link>
        </li>
      ))}
    </ul>
  );
}

interface HelpCardProps {
  href?: string;
  title?: string;
  subtitle?: string;
}

/**
 * Small "Need help?" card that links to GitHub issues by default.
 */
export function HelpCard({
  href = 'https://github.com/siriuslatte/lithos/issues',
  title = 'Need help?',
  subtitle = 'Open an issue on GitHub',
}: HelpCardProps) {
  return (
    <a
      href={href}
      className="lithos-help-card"
      target="_blank"
      rel="noreferrer"
    >
      <span className="lithos-help-card-icon">
        <MessageCircle size={18} strokeWidth={2.1} />
      </span>
      <span>
        <span className="lithos-help-card-title">{title}</span>
        <br />
        <span className="lithos-help-card-sub">{subtitle}</span>
      </span>
    </a>
  );
}
