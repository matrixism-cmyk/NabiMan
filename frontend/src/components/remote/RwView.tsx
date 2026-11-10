import React from 'react';
import { ErrorBoundary } from '../../shared/ui/ErrorBoundary';

/** One of the workspace's views; a crash inside it stays inside it. */
export default function RwView({ region, resetKey, children, ...rest }: React.HTMLAttributes<HTMLDivElement> & { region: string; resetKey?: unknown }) {
  return <div {...rest}><ErrorBoundary region={region} resetKey={resetKey}>{children}</ErrorBoundary></div>;
}
