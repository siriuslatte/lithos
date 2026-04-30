import { useRouter } from 'next/router';

const icon = (
  <svg
    xmlns="http://www.w3.org/2000/svg"
    width="22"
    height="22"
    viewBox="0 0 24 24"
    fill="none"
    aria-hidden="true"
  >
    <defs>
      <linearGradient id="lithos-logo-grad" x1="0" y1="0" x2="24" y2="24" gradientUnits="userSpaceOnUse">
        <stop offset="0" stopColor="#34d399" />
        <stop offset="1" stopColor="#22d3ee" />
      </linearGradient>
    </defs>
    {/* Chiseled cap (faceted stone) */}
    <path
      d="M12 1.5 L16 5 L12 8.5 L8 5 Z"
      fill="url(#lithos-logo-grad)"
    />
    {/* Top stratum */}
    <rect x="6.5" y="9.25" width="11" height="3.25" rx="0.6" fill="url(#lithos-logo-grad)" />
    {/* Middle stratum */}
    <rect x="4.25" y="13.25" width="15.5" height="3.25" rx="0.6" fill="url(#lithos-logo-grad)" opacity="0.85" />
    {/* Bottom stratum */}
    <rect x="2" y="17.25" width="20" height="3.25" rx="0.6" fill="url(#lithos-logo-grad)" opacity="0.7" />
  </svg>
);

function Logo() {
  return (
    <span style={{ display: 'inline-flex', alignItems: 'center' }}>
      {icon}
      <span
        style={{
          marginLeft: '.5em',
          fontWeight: 700,
          letterSpacing: '-0.01em',
          fontSize: '1.05em',
        }}
      >
        Lithos
      </span>
    </span>
  );
}

export default {
  logo: <Logo />,
  project: {
    link: 'https://github.com/siriuslatte/lithos',
  },
  docsRepositoryBase:
    'https://github.com/siriuslatte/lithos/tree/main/docs/site',
  primaryHue: 158,
  primarySaturation: 65,
  head: (
    <>
      <link rel="icon" type="image/svg+xml" href="/favicon.svg" />
      <meta name="theme-color" content="#0a0f14" />
      <meta name="viewport" content="width=device-width, initial-scale=1" />
      <meta property="og:title" content="Lithos" />
      <meta
        property="og:description"
        content="Roblox infrastructure-as-code and deployment tool."
      />
    </>
  ),
  sidebar: {
    defaultMenuCollapseLevel: 1,
    toggleButton: true,
  },
  toc: {
    backToTop: true,
  },
  feedback: {
    content: null,
  },
  footer: {
    text: (
      <span style={{ color: 'var(--lithos-muted)', fontSize: '0.85rem' }}>
        Lithos · MIT licensed · Continuation of{' '}
        <a
          href="https://github.com/siriuslatte/lithos"
          target="_blank"
          rel="noreferrer"
          style={{ color: 'var(--lithos-accent)' }}
        >
          Mantle
        </a>{' '}
        by Blake Mealey.
      </span>
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
