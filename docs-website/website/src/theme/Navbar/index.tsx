import React, {type ReactNode} from 'react';
import Link from '@docusaurus/Link';
import {useThemeConfig} from '@docusaurus/theme-common';
import NavbarColorModeToggle from '@theme/Navbar/ColorModeToggle';
import styles from './styles.module.css';

type NavbarItemConfig = {
  to?: string;
  href?: string;
  label: string;
  position?: 'left' | 'right';
};

/**
 * Ultra-minimal swizzled Navbar with theme config integration.
 * Uses themeConfig.navbar.items for proper rendering and includes color mode toggle.
 */
export default function Navbar(): ReactNode {
  const {navbar} = useThemeConfig();
  const items = (navbar.items || []) as NavbarItemConfig[];

  return (
    <nav className={`${styles.navbar} navbar`} aria-label="Main">
      <div className={styles.inner}>
        <Link to="/" className={styles.brand}>
          McHarness
        </Link>
        <div className={styles.links}>
          {items.map((item, index) => {
            // Render doc links (to) and external links (href)
            if (item.to) {
              return (
                <Link key={index} to={item.to} className={styles.link}>
                  {item.label}
                </Link>
              );
            }
            if (item.href) {
              return (
                <a
                  key={index}
                  className={styles.link}
                  href={item.href}
                  target="_blank"
                  rel="noreferrer noopener">
                  {item.label}
                </a>
              );
            }
            return null;
          })}
          <NavbarColorModeToggle className={styles.colorModeToggle} />
        </div>
      </div>
    </nav>
  );
}