import React, {type ReactNode} from 'react';
import NavbarLayout from '@theme/Navbar/Layout';

export default function Navbar(): ReactNode {
  return (
    <NavbarLayout>
      <div className="navbar__title">
        McHarness
      </div>
    </NavbarLayout>
  );
}