import { useRouter } from 'next/router';
import { ExternalLink, Star } from 'react-feather';

function CrystalMark({ size = 22 }) {
  return (
    <svg
      xmlns="http://www.w3.org/2000/svg"
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      aria-hidden="true"
    >
      <path
        d="M12 1.8 17.1 7.25 15.15 17.95 12 22.1 8.85 17.95 6.9 7.25 12 1.8Z"
        fill="#8b5cf6"
        stroke="#5b21b6"
        strokeWidth="0.75"
        strokeLinejoin="round"
      />
      <path
        d="M12 1.8V22.1L8.85 17.95 6.9 7.25 12 1.8Z"
        fill="#7c3aed"
      />
      <path
        d="M12 1.8 17.1 7.25 12 10.15 6.9 7.25 12 1.8Z"
        fill="#e9d5ff"
        fillOpacity="0.34"
      />
      <path
        d="M12 10.15 14.15 18.35 12 22.1 9.85 18.35 12 10.15Z"
        fill="#f5d0fe"
        fillOpacity="0.36"
      />
      <path
        d="M10.2 4.1 11.25 7.55 10.45 16.75 9.2 18.45 10.2 4.1Z"
        fill="#ddd6fe"
        fillOpacity="0.2"
      />
      <path
        d="M13.75 3.95 15.6 7.7 14.65 16.95 13.1 18.85 13.75 3.95Z"
        fill="#ffffff"
        fillOpacity="0.24"
      />
      <path
        d="M7.25 11.75 2.55 14.95 5.95 20.55 10.15 18.15 9.2 14.25 7.25 11.75Z"
        fill="#6d28d9"
        stroke="#4c1d95"
        strokeWidth="0.75"
        strokeLinejoin="round"
      />
      <path
        d="M16.75 11.75 21.45 14.95 18.05 20.55 13.85 18.15 14.8 14.25 16.75 11.75Z"
        fill="#6d28d9"
        stroke="#4c1d95"
        strokeWidth="0.75"
        strokeLinejoin="round"
      />
      <path
        d="M7.95 12.85 9.55 14.9 8.95 17.1 6.35 18.8 7.95 12.85Z"
        fill="#c4b5fd"
        fillOpacity="0.3"
      />
      <path
        d="M16.05 12.85 14.45 14.9 15.05 17.1 17.65 18.8 16.05 12.85Z"
        fill="#f0abfc"
        fillOpacity="0.24"
      />
    </svg>
  );
}

function Logo() {
  return (
    <span className="lithos-nav-brand">
      <CrystalMark size={22} />
      <span className="lithos-nav-wordmark">
        LITHOS
      </span>
    </span>
  );
}

function SidebarTitle({ title, type }) {
  return (
    <span
      className={
        type === 'separator'
          ? 'lithos-sidebar-section-label'
          : 'lithos-sidebar-title'
      }
    >
      {title}
    </span>
  );
}

function HiddenEditLink() {
  return null;
}

function FooterLink({ href, children, external = false }) {
  return (
    <a
      href={href}
      className="lithos-site-footer-link-row"
      target={external ? '_blank' : undefined}
      rel={external ? 'noreferrer' : undefined}
    >
      <span>{children}</span>
      {external ? <ExternalLink size={12} strokeWidth={2.1} /> : null}
    </a>
  );
}

export default {
  logo: <Logo />,
  project: {
    link: 'https://github.com/siriuslatte/lithos',
  },
  docsRepositoryBase:
    'https://github.com/siriuslatte/lithos/tree/main/docs/site',
  primaryHue: 266,
  primarySaturation: 92,
  head: (
    <>
      <link rel="icon" type="image/svg+xml" href="/favicon.svg" />
      <meta name="theme-color" content="#070914" />
      <meta name="viewport" content="width=device-width, initial-scale=1" />
      <meta property="og:title" content="Lithos" />
      <meta
        property="og:description"
        content="Roblox infrastructure-as-code for previewed, stateful deploys."
      />
    </>
  ),
  sidebar: {
    defaultMenuCollapseLevel: 1,
    toggleButton: false,
    titleComponent: SidebarTitle,
  },
  toc: {
    backToTop: false,
    title: 'On this page',
  },
  editLink: {
    component: HiddenEditLink,
  },
  feedback: {
    content: null,
  },
  footer: {
    text: (
      <footer className="lithos-site-footer">
        <div className="lithos-site-footer-grid">
          <div className="lithos-site-footer-brand">
            <span className="lithos-nav-brand">
              <CrystalMark size={22} />
              <span className="lithos-nav-wordmark">LITHOS</span>
            </span>
            <p>
              Roblox infrastructure-as-code for previewed deploys, outputs, and
              rollback-aware state.
            </p>
          </div>

          <div className="lithos-site-footer-column">
            <div className="lithos-site-footer-heading">Links</div>
            <div className="lithos-site-footer-list">
              <FooterLink href="/docs">Docs</FooterLink>
              <FooterLink href="https://github.com/siriuslatte/lithos" external>
                GitHub
              </FooterLink>
              <FooterLink href="https://github.com/siriuslatte/lithos/releases" external>
                Changelog
              </FooterLink>
            </div>
          </div>

          <div className="lithos-site-footer-column">
            <div className="lithos-site-footer-heading">Community</div>
            <div className="lithos-site-footer-list">
              <FooterLink href="https://github.com/siriuslatte/lithos/issues" external>
                Issues
              </FooterLink>
              <FooterLink href="https://github.com/siriuslatte/lithos/blob/main/CONTRIBUTING.md" external>
                Contributing
              </FooterLink>
              <FooterLink href="https://github.com/siriuslatte/lithos/blob/main/SUPPORT.md" external>
                Support
              </FooterLink>
            </div>
          </div>

          <a
            href="https://github.com/siriuslatte/lithos"
            target="_blank"
            rel="noreferrer"
            className="lithos-site-footer-card"
          >
            <span className="lithos-site-footer-card-icon">
              <Star size={15} strokeWidth={2.1} />
            </span>

            <div className="lithos-site-footer-card-copy">
              <div className="lithos-site-footer-card-title">Star us on GitHub</div>
              <p>
                If you like Lithos, consider giving us a star.
              </p>
            </div>
          </a>
        </div>

        <div className="lithos-site-footer-bottom">
          © {new Date().getFullYear()} Lithos Project. MIT licensed. Continuation of Mantle.
        </div>
      </footer>
    ),
  },
  useNextSeoProps() {
    const { route } = useRouter();
    if (route !== '/') {
      return {
        titleTemplate: '%s – Lithos',
      };
    }
  },
};
