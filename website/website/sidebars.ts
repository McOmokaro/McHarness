/**
 * Sidebar configuration for McHarness documentation.
 * Structured identically to Hermes Agent documentation.
 */

import type {SidebarsConfig} from '@docusaurus/plugin-content-docs';

/**
 * Creating a sidebar enables you to:
 - create an ordered group of docs
 - render a sidebar for each doc of that group
 - provide next/previous navigation

 The sidebars can be generated from the filesystem, or explicitly defined here.

 Create as many sidebars as you want.
 */

const sidebars: SidebarsConfig = {
  tutorialSidebar: [
    {
      type: 'category',
      label: 'Getting Started',
      collapsed: false,
      items: [
        {
          type: 'doc',
          label: 'Installation',
          to: '/docs/installation',
        },
        {
          type: 'doc',
          label: 'Quickstart Tutorial',
          to: '/docs/quickstart',
        },
        {
          type: 'doc',
          label: 'Learning Path',
          to: '/docs/learning-path',
        },
      ],
    },
    {
      type: 'category',
      label: 'User Guide',
      collapsed: true,
      items: [
        {
          type: 'doc',
          label: 'Configuration',
          to: '/docs/user-guide/configuration',
        },
        {
          type: 'doc',
          label: 'Messaging Gateway',
          to: '/docs/user-guide/messaging',
        },
        {
          type: 'doc',
          label: 'Bot Mode',
          to: '/docs/user-guide/bot-mode',
        },
        {
          type: 'doc',
          label: 'Features & Tools',
          to: '/docs/user-guide/features/tools',
        },
        {
          type: 'doc',
          label: 'Memory System',
          to: '/docs/user-guide/features/memory',
        },
        {
          type: 'doc',
          label: 'Skills System',
          to: '/docs/user-guide/features/skills',
        },
        {
          type: 'doc',
          label: 'MCP Integration',
          to: '/docs/user-guide/features/mcp',
        },
        {
          type: 'doc',
          label: 'Voice Mode',
          to: '/docs/user-guide/features/voice-mode',
        },
        {
          type: 'doc',
          label: 'Security',
          to: '/docs/user-guide/security',
        },
        {
          type: 'doc',
          label: 'Tips & Best Practices',
          to: '/docs/guides/tips',
        },
      ],
    },
    {
      type: 'category',
      label: 'Developer Guide',
      collapsed: true,
      items: [
        {
          type: 'doc',
          label: 'Architecture',
          to: '/docs/developer-guide/architecture',
        },
        {
          type: 'doc',
          label: 'Contributing',
          to: '/docs/developer-guide/contributing',
        },
      ],
    },
    {
      type: 'category',
      label: 'Reference',
      collapsed: true,
      items: [
        {
          type: 'doc',
          label: 'CLI Commands',
          to: '/docs/reference/cli-commands',
        },
        {
          type: 'doc',
          label: 'FAQ',
          to: '/docs/reference/faq',
        },
      ],
    },
    {
      type: 'category',
      label: 'Introduction',
      collapsed: false,
      items: [
        {
          type: 'doc',
          label: 'Introduction',
          to: '/docs/intro',
        },
      ],
    },
    {
      type: 'category',
      label: 'Roadmap',
      collapsed: false,
      items: [
        {
          type: 'doc',
          label: 'Roadmap',
          to: '/docs/roadmap',
        },
      ],
    },
    {
      type: 'category',
      label: 'REA Integration',
      collapsed: false,
      items: [
        {
          type: 'doc',
          label: 'REA Integration',
          to: '/docs/rea-integration',
        },
      ],
    },
  ],
};

export default sidebars;