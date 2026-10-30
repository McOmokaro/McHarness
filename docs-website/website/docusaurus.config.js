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

  onBrokenLinks: 'throw',
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

  themeConfig:
    /** @type {import('@docusaurus/preset-classic').ThemeConfig} */
    ({
      navbar: {
        title: 'McHarness',
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
