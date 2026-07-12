// @ts-check
/** @type {import('@docusaurus/types').Config} */
const config = {
  title: "Mari Console",
  favicon: 'img/favicon.ico',
  url: 'https://mari.guru',
  baseUrl: '/console-docs/',
  onBrokenLinks: 'warn',
  presets: [
    [
      'classic',
      /** @type {import('@docusaurus/preset-classic').Options} */
      ({ docs: { routeBasePath: '/', sidebarPath: './sidebars.js' }, blog: false }),
    ],
  ],
};

module.exports = config;
