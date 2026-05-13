import Image from 'next/image';
import Link from 'next/link';
import {
  ArrowRight,
  Cloud,
  Database,
  ExternalLink,
  FileText,
  GitMerge,
  Layers,
  RotateCcw,
  Shield,
} from 'react-feather';

const FEATURES = [
  {
    icon: <GitMerge size={18} strokeWidth={2.15} />,
    title: 'Preview every deploy',
    description:
      'See exact create, update, and delete operations before Lithos mutates Roblox.',
  },
  {
    icon: <Shield size={18} strokeWidth={2.15} />,
    title: 'Reconcile drift',
    description:
      'Compare recorded state with the live API first, then apply only the changes that still make sense.',
  },
  {
    icon: <RotateCcw size={18} strokeWidth={2.15} />,
    title: 'Rollback with checkpoints',
    description:
      'Each deploy records enough history for `lithos undo` to drive resources back toward the last checkpoint.',
  },
  {
    icon: <Layers size={18} strokeWidth={2.15} />,
    title: 'Keep environments declarative',
    description:
      'Run the same config through `dev` and `prod` without copying Creator Hub checklists by hand.',
  },
];

const FLOW_STAGES = [
  {
    icon: <FileText size={18} strokeWidth={2.15} />,
    title: 'Configuration',
    description: 'The desired graph from `lithos.yml` or `lithos.json`.',
  },
  {
    icon: <Database size={18} strokeWidth={2.15} />,
    title: 'State',
    description: 'The last successful graph plus recent deployment checkpoints.',
  },
  {
    icon: <Cloud size={18} strokeWidth={2.15} />,
    title: 'Live API',
    description: 'What Roblox actually has right now.',
  },
];

const NEXT_STEPS = [
  {
    href: '/docs/getting-started',
    title: 'Deploy a real example',
    description:
      'Install Lithos, authenticate, and run a full `dev` deployment end-to-end.',
  },
  {
    href: '/docs/concepts/state',
    title: 'Understand state',
    description:
      'Learn how checkpoints, reconciliation, and remote state storage work.',
  },
  {
    href: '/docs/configuration/reference',
    title: 'Browse the schema',
    description:
      'Look up every supported field for experiences, places, badges, passes, and more.',
  },
];

export function DocsIntroShowcase() {
  return (
    <div className="lithos-docs-showcase">
      <section className="lithos-docs-hero-card">
        <div className="lithos-docs-hero-copy">
          <span className="lithos-pill">Preview first</span>

          <p className="lithos-docs-hero-title">
            Stateful Roblox deploys with drift reconciliation built in.
          </p>

          <p className="lithos-docs-hero-text">
            Lithos turns Creator Hub metadata into reviewable infrastructure-as-code,
            so teams can plan changes, ship only the intended mutations, and roll
            back from recorded checkpoints when a deploy goes sideways.
          </p>

          <div className="lithos-docs-hero-actions">
            <Link href="/docs/getting-started" className="lithos-cta">
              Get started
              <ArrowRight size={16} strokeWidth={2.2} />
            </Link>

            <a
              href="https://github.com/siriuslatte/lithos"
              target="_blank"
              rel="noreferrer"
              className="lithos-cta-secondary"
            >
              View on GitHub
              <ExternalLink size={15} strokeWidth={2.1} />
            </a>
          </div>
        </div>

        <div className="lithos-docs-hero-visual" aria-hidden="true">
          <Image
            src="/img/banner.png"
            alt=""
            fill
            priority
            sizes="(max-width: 1024px) 100vw, 42vw"
            className="lithos-docs-hero-image"
          />
          <div className="lithos-docs-hero-visual-glow" />
          <div className="lithos-docs-hero-chip">
            Config + state + live API before any destructive change
          </div>
        </div>
      </section>

      <section className="lithos-docs-feature-grid" aria-label="Lithos capabilities">
        {FEATURES.map((feature) => (
          <div className="lithos-docs-feature-card" key={feature.title}>
            <span className="lithos-docs-feature-icon">{feature.icon}</span>
            <div className="lithos-docs-feature-title">{feature.title}</div>
            <p className="lithos-docs-feature-text">{feature.description}</p>
          </div>
        ))}
      </section>

      <section className="lithos-docs-flow-card" aria-label="Lithos deployment model">
        <div className="lithos-docs-section-kicker">Deployment model</div>
        <p className="lithos-docs-flow-lede">
          Every deploy is a reconciliation pass across these three inputs.
        </p>

        <div className="lithos-docs-flow-row">
          {FLOW_STAGES.map((stage, index) => (
            <div className="lithos-docs-flow-item" key={stage.title}>
              <div className="lithos-docs-flow-stage">
                <span className="lithos-docs-flow-icon">{stage.icon}</span>
                <div>
                  <div className="lithos-docs-flow-title">{stage.title}</div>
                  <p className="lithos-docs-flow-text">{stage.description}</p>
                </div>
              </div>

              {index < FLOW_STAGES.length - 1 ? (
                <span className="lithos-docs-flow-arrow" aria-hidden="true">
                  <ArrowRight size={18} strokeWidth={2.15} />
                </span>
              ) : null}
            </div>
          ))}
        </div>
      </section>

      <section className="lithos-docs-link-grid" aria-label="Suggested next reads">
        {NEXT_STEPS.map((card) => (
          <Link href={card.href} key={card.title} className="lithos-docs-link-card">
            <div className="lithos-docs-link-title-row">
              <div className="lithos-docs-link-title">{card.title}</div>
              <ArrowRight size={16} strokeWidth={2.15} />
            </div>
            <p className="lithos-docs-link-text">{card.description}</p>
          </Link>
        ))}
      </section>
    </div>
  );
}