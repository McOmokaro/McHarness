/** @type {import('@docusaurus/types').DocusaurusConfig} */
module.exports = {
  title: 'McHarness',
  tagline: 'The Father of Harnesses',
  url: 'https://McOmokaro.github.io',
  baseUrl: '/McHarness/',

  // Set the production domain here
  organizationName: 'McOmokaro',
  projectName: 'McHarness',

  // Optional: Preset configuration for classic preset
  presets: [
    [
      'classic',
      {
        docs: {
          sidebarPath: './sidebars.js',
          // Please change this to your repo.
          // editUrl: 'https://github.com/your-org/your-repo/tree/main',
        },
        theme: {
          customCss: './src/css/custom.css',
        },
      },
    ],
  ],

  // Theme configuration for Apple-inspired design
  themeConfig: {
    navbar: {
      title: 'McHarness',
      logo: 'img/logo.svg',
      items: [
        { type: 'docSidebar', position: 'left', sidebarId: 'tutorialSidebar', label: 'Introduction' },
        { type: 'docSidebar', position: 'left', sidebarId: 'roadmapSidebar', label: 'Roadmap' },
        { type: 'docSidebar', position: 'left', sidebarId: 'reaSidebar', label: 'REA Integration' },
        {
          href: 'https://github.com/McOmokaro/McHarness',
          label: 'GitHub',
          position: 'right',
        },
      ],
    },
    footer: {
      style: 'dark',
      links: [
        {
          title: 'Community',
          items: [
            {
              label: 'Discord',
              href: 'https://discord.gg/mcharness',
            },
          ],
        },
        {
          title: 'More',
          items: [
            {
              label: 'Blog',
              to: '/McHarness/blog',
            },
            {
              label: 'GitHub',
              href: 'https://github.com/McOmokaro/McHarness',
            },
          ],
        },
      ],
      copyright: `Copyright © ${new Date().getFullYear()} McHarness. Built with Docusaurus.`,
    },
  },
};