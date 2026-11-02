import {themes as prismThemes} from 'prism-react-renderer';

/** @type {import('@docusaurus/types').Config} */
const config = {
  title: 'McHarness',
  tagline: 'Father of Harnesses — one whole, light-speed, featherweight multi-agent harness',
  favicon: 'img/favicon.ico',

  // Your final URL: https://McOmokaro.github.io/McHarness/
  url: 'https://McOmokaro.github.io',
  baseUrl: '/McHarness/',

  // Needed for GitHub Pages
  organizationName: 'McOmokaro',
  projectName: 'McHarness',
  trailingSlash: false,

  onBrokenLinks: 'warn',
  onBrokenMarkdownLinks: 'warn',

  i18n: {
    defaultLocale: 'en',
    locales: ['en'],
  },

  presets: [
    [
      'classic',
      /** @type {import('@docusaurus/preset-classic').Options} */
      ({
        docs: {
          sidebarPath: './sidebars.js',
          routeBasePath: '/', // Makes docs the homepage
        },
        blog: false, // Nuked the blog
        theme: {
          customCss: './src/css/custom.css',
        },
      }),
    ],
  ],

  themes: [
    [
      require.resolve('@easyops-cn/docusaurus-search-local'),
      /** @type {import('@easyops-cn/docusaurus-search-local').PluginOptions} */
      ({
        hashed: true,
        language: ['en'],
        highlightSearchTermsOnTargetPage: true,
        explicitSearchResultPath: true,
        docsRouteBasePath: '/',
      }),
    ],
  ],

  themeConfig:
    /** @type {import('@docusaurus/preset-classic').ThemeConfig} */
    ({
      colorMode: {
        defaultMode: 'dark',
        disableSwitch: false,
        respectPrefersColorScheme: true,
      },
      docs: {
        sidebar: {
          // Accordion behavior: opening one category auto-closes the others.
          autoCollapseCategories: true,
        },
      },
      navbar: {
        title: 'McHarness',
        items: [
          {to: '/', label: 'Docs', position: 'left', activeBaseRegex: '^/$|^/McHarness/$'},
          {to: '/skills', label: 'Skills', position: 'left'},
          {to: '/plugins', label: 'Plugins', position: 'left'},
          {to: '/download', label: 'Download', position: 'left'},
          {
            href: 'https://github.com/McOmokaro/McHarness',
            label: 'Home',
            position: 'right',
          },
          {
            href: 'https://github.com/McOmokaro/McHarness',
            label: 'GitHub',
            position: 'right',
          },
        ],
      },
      footer: {
        style: 'dark',
        copyright: `Copyright © ${new Date().getFullYear()} McHarness.`,
      },
      prism: {
        theme: prismThemes.github,
        darkTheme: prismThemes.dracula,
      },
    }),
};

export default config;
