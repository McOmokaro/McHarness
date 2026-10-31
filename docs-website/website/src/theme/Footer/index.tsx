import React, {type ReactNode} from 'react';
import FooterLayout from '@theme/Footer/Layout';
import {useThemeConfig} from '@docusaurus/theme-common';

function Footer(): ReactNode {
  const {footer} = useThemeConfig();
  // Minimalist: only show copyright, strip all links and logos
  if (!footer) {
    return null;
  }
  const {copyright} = footer;
  return (
    <FooterLayout
      style={footer.style}
      links={null}
      logo={null}
      copyright={copyright && <div className="footer__copyright">{copyright}</div>}
    />
  );
}

export default React.memo(Footer);