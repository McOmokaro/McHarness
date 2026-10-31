import React, {type ReactNode} from 'react';
import Link from '@docusaurus/Link';
import styles from './styles.module.css';

/**
 * Ultra-minimal swizzled Footer.
 * All default Docusaurus link columns and logos are stripped — just the
 * wordmark, the tagline, a couple of quiet links and the copyright line.
 */
function Footer(): ReactNode {
  const year = new Date().getFullYear();
  return (
    <footer className={styles.footer}>
      <div className={styles.inner}>
        <div className={styles.brandBlock}>
          <span className={styles.wordmark}>McHarness</span>
          <span className={styles.tagline}>
            Father of Harnesses — one whole, light-speed, featherweight
            multi-agent harness.
          </span>
        </div>
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
        <div className={styles.copyright}>
          Copyright © {year} McHarness.
        </div>
      </div>
    </footer>
  );
}

export default React.memo(Footer);
