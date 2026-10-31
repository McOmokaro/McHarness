import React, {type ReactNode} from 'react';
import Link from '@docusaurus/Link';
import styles from './styles.module.css';

/**
 * Ultra-minimal swizzled Navbar.
 * Default Docusaurus logo, search bar and nav items are stripped.
 * Layout mirrors the Hermes Agent docs: quiet wordmark on the left,
 * a few understated links on the right, Apple typography throughout.
 */
export default function Navbar(): ReactNode {
  return (
    <nav className={styles.navbar} aria-label="Main">
      <div className={styles.inner}>
        <Link to="/" className={styles.brand}>
          McHarness
        </Link>
        <div className={styles.links}>
          <Link to="/" className={styles.link}>
            Docs
          </Link>
          <Link to="/roadmap" className={styles.link}>
            Roadmap
          </Link>
          <a
            className={styles.link}
            href="https://github.com/McOmokaro/McHarness"
            target="_blank"
            rel="noreferrer noopener">
            GitHub
          </a>
        </div>
      </div>
    </nav>
  );
}
