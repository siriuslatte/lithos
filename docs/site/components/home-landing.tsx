import Image from 'next/image';
import Link from 'next/link';
import type { CSSProperties, ReactElement } from 'react';
import {
  ArrowRight,
  BookOpen,
  Compass,
  Database,
  DownloadCloud,
  ExternalLink,
  Key,
  RotateCcw,
  Shield,
  Sliders,
  Terminal,
  Zap,
} from 'react-feather';

type Highlight = {
  title: string;
  description: string;
  icon: ReactElement;
};

type DocCard = {
  title: string;
  description: string;
  href: string;
  accent: string;
  icon: ReactElement;
};

const HIGHLIGHTS: Highlight[] = [
  {
    title: 'Preview First',
    description:
      'Readable plans, field-level diffs, and explicit destructive warnings before a deploy touches Roblox.',
    icon: <Zap size={18} strokeWidth={2.2} />,
  },
  {
    title: 'Drift Aware',
    description:
      'Lithos reconciles saved state with the live platform so dashboard edits and deletions do not surprise the next rollout.',
    icon: <Sliders size={18} strokeWidth={2.2} />,
  },
  {
    title: 'Rollback Ready',
    description:
      'Checkpointed deploy history powers rollback previews and best-effort undo without hiding what will change.',
    icon: <RotateCcw size={18} strokeWidth={2.2} />,
  },
  {
    title: 'CI Friendly',
    description:
      'The same commands stay deterministic in pipelines, with non-interactive previews, remote state, and explicit flags.',
    icon: <Shield size={18} strokeWidth={2.2} />,
  },
];

const DOC_CARDS: DocCard[] = [
  {
    title: 'Getting Started',
    description: 'Install Lithos, wire up credentials, and run your first preview against a real project.',
    href: '/docs/getting-started',
    accent: '#9f67ff',
    icon: <Compass size={21} strokeWidth={2.1} />,
  },
  {
    title: 'Installation',
    description: 'Set up Foreman, direct downloads, or source builds and pick the path that fits your workflow.',
    href: '/docs/installation',
    accent: '#4f8cff',
    icon: <DownloadCloud size={21} strokeWidth={2.1} />,
  },
  {
    title: 'Authentication',
    description: 'Configure ROBLOSECURITY and Open Cloud keys with the scopes Lithos expects in real deployments.',
    href: '/docs/authentication',
    accent: '#34d399',
    icon: <Key size={21} strokeWidth={2.1} />,
  },
  {
    title: 'CLI Commands',
    description: 'Learn how deploy, diff, outputs, import, destroy, undo, and state behave in the current CLI.',
    href: '/docs/commands',
    accent: '#fb923c',
    icon: <Terminal size={21} strokeWidth={2.1} />,
  },
  {
    title: 'Remote State',
    description: 'Run Lithos with local state, Amazon S3, or Cloudflare R2 and keep teams on the same source of truth.',
    href: '/docs/remote-state',
    accent: '#38bdf8',
    icon: <Database size={21} strokeWidth={2.1} />,
  },
  {
    title: 'Configuration Reference',
    description: 'Browse the generated schema-backed reference for every supported key, value, and nested shape.',
    href: '/docs/configuration/reference',
    accent: '#c084fc',
    icon: <BookOpen size={21} strokeWidth={2.1} />,
  },
];

export function HomeLanding() {
  const bannerSrc = `${process.env.NEXT_PUBLIC_BASE_PATH || ''}/img/banner.png`;

  return (
    <div className="lithos-home-page">
      <section className="lithos-home-hero">
        <div className="lithos-home-hero-media" aria-hidden="true">
          <Image
            src={bannerSrc}
            alt=""
            fill
            priority
            sizes="(min-width: 1280px) 1200px, 100vw"
            className="lithos-home-hero-image"
          />
        </div>
        <div className="lithos-home-hero-overlay" aria-hidden="true" />

        <div className="lithos-home-hero-shell">
          <div className="lithos-home-hero-content">
            <span className="lithos-home-version">v0.3.0</span>

            <h1 className="lithos-home-title">Lithos</h1>

            <p className="lithos-home-lede">
              Roblox infrastructure-as-code for previewed, stateful deploys.
            </p>

            <p className="lithos-home-copy">
              Plan, apply, export environment IDs, and roll back experiences,
              places, products, passes, badges, thumbnails, and more from one
              CLI built for real release workflows.
            </p>

            <div className="lithos-home-actions">
              <Link href="/docs/getting-started" className="lithos-home-primary-button">
                Get Started
                <ArrowRight size={16} strokeWidth={2.2} />
              </Link>

              <a
                href="https://github.com/siriuslatte/lithos"
                target="_blank"
                rel="noreferrer"
                className="lithos-home-secondary-button"
              >
                View on GitHub
                <ExternalLink size={15} strokeWidth={2.1} />
              </a>
            </div>

            <p className="lithos-home-caption">
              Continuation of Mantle. Existing <code>mantle.yml</code> projects
              still work.
            </p>
          </div>
        </div>
      </section>

      <div className="lithos-home-page-content">
        <section className="lithos-home-highlights" aria-label="Lithos highlights">
          {HIGHLIGHTS.map((highlight) => (
            <div className="lithos-home-highlight" key={highlight.title}>
              <span className="lithos-home-highlight-icon">{highlight.icon}</span>
              <h2 className="lithos-home-highlight-title">{highlight.title}</h2>
              <p className="lithos-home-highlight-copy">{highlight.description}</p>
            </div>
          ))}
        </section>

        <section className="lithos-home-section">
          <div className="lithos-home-section-heading">
            <h2 className="lithos-home-section-title">Documentation</h2>
            <p className="lithos-home-section-copy">
              Everything you need to install, configure, preview, and operate Lithos.
            </p>
          </div>

          <div className="lithos-home-doc-grid">
            {DOC_CARDS.map((card) => {
              const accentStyle = {
                '--lithos-doc-card-accent': card.accent,
              } as CSSProperties;

              return (
                <Link
                  href={card.href}
                  key={card.title}
                  className="lithos-home-doc-card"
                  style={accentStyle}
                >
                  <div className="lithos-home-doc-card-top">
                    <span className="lithos-home-doc-card-icon">{card.icon}</span>
                    <ArrowRight
                      size={18}
                      strokeWidth={2.1}
                      className="lithos-home-doc-card-arrow"
                    />
                  </div>

                  <h3 className="lithos-home-doc-card-title">{card.title}</h3>
                  <p className="lithos-home-doc-card-copy">{card.description}</p>
                </Link>
              );
            })}
          </div>

          <div className="lithos-home-callout">
            <div className="lithos-home-callout-copy">
              <span className="lithos-home-callout-icon">
                <Terminal size={18} strokeWidth={2.2} />
              </span>

              <div>
                <div className="lithos-home-callout-title">New to Lithos?</div>
                <p className="lithos-home-callout-text">
                  Start with a small project, preview a diff, and walk the full
                  deploy path before you point it at production.
                </p>
              </div>
            </div>

            <Link href="/docs/getting-started" className="lithos-home-callout-link">
              Read the Getting Started guide
              <ArrowRight size={16} strokeWidth={2.1} />
            </Link>
          </div>
        </section>
      </div>
    </div>
  );
}