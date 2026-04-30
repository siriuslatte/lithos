const withNextra = require('nextra')({
  theme: 'nextra-theme-docs',
  themeConfig: './theme.config.jsx',
});

// Configure for static export so the site can be hosted on GitHub Pages.
// `NEXT_PUBLIC_BASE_PATH` is set by the deploy workflow to the repository
// path (e.g. `/lithos`) for project Pages. Leave empty for user/org pages
// or custom domains.
const basePath = process.env.NEXT_PUBLIC_BASE_PATH || '';

module.exports = withNextra({
  output: 'export',
  images: { unoptimized: true },
  trailingSlash: true,
  basePath,
  assetPrefix: basePath || undefined,
});
