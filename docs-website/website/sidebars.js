/** @type {import('@docusaurus/plugin-content-docs').SidebarsConfig} */
const sidebars = {
  docs: [
    {type: 'doc', id: 'index', label: 'McHarness'},
    {type: 'doc', id: 'skills', label: 'Skills'},
    {type: 'doc', id: 'plugins', label: 'Plugins'},
    {type: 'doc', id: 'download', label: 'Download'},
    {
      type: 'category',
      label: 'Getting Started',
      collapsed: true,
      items: ['quickstart'],
    },
    {
      type: 'category',
      label: 'Architecture',
      collapsed: true,
      items: ['architecture', 'rea-integration'],
    },
    {
      type: 'category',
      label: 'The Forge',
      collapsed: true,
      items: ['the-forge'],
    },
    {
      type: 'category',
      label: 'Project',
      collapsed: true,
      items: ['roadmap'],
    },
  ],
};

export default sidebars;
