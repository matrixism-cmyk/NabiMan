import React, { useState } from 'react';
import { useT } from '../i18n';
import { buildCrashReport, copyText, errorText, isChunkLoadError } from './crashReport';
import './error-boundary.css';

interface Props {
  children?: React.ReactNode;
  /** Names the region in the copied report ("panel:ssl", "terminal", …). */
  region?: string;
  /** `app` replaces the whole page; `pane` stays inside its region. */
  variant?: 'app' | 'pane';
  /** A new value clears a caught error, e.g. the tab the outlet shows. */
  resetKey?: unknown;
}

interface State { error: unknown; componentStack: string; failed: boolean; resetKey: unknown }

/**
 * Catches a render error below it so one broken part of the screen shows a
 * plain explanation instead of blanking everything around it.
 */
export class ErrorBoundary extends React.Component<Props, State> {
  state: State = { error: null, componentStack: '', failed: false, resetKey: this.props.resetKey };

  static getDerivedStateFromError(error: unknown): Partial<State> {
    return { error, failed: true };
  }

  static getDerivedStateFromProps(props: Props, state: State): Partial<State> | null {
    if (props.resetKey === state.resetKey) return null;
    return { error: null, componentStack: '', failed: false, resetKey: props.resetKey };
  }

  componentDidCatch(_error: unknown, info: React.ErrorInfo) {
    this.setState({ componentStack: info.componentStack || '' });
  }

  private reset = () => this.setState({ error: null, componentStack: '', failed: false });

  render() {
    if (!this.state.failed) return this.props.children ?? null;
    const { error, componentStack } = this.state;
    return (
      <CrashFallback error={error} componentStack={componentStack} region={this.props.region}
        variant={this.props.variant || 'pane'} onRetry={this.reset} />
    );
  }
}

interface FallbackProps {
  error: unknown;
  componentStack: string;
  region?: string;
  variant: 'app' | 'pane';
  onRetry: () => void;
}

export function CrashFallback({ error, componentStack, region, variant, onRetry }: FallbackProps) {
  const { t } = useT();
  const [copy, setCopy] = useState<'idle' | 'copied' | 'failed'>('idle');
  const [report, setReport] = useState('');
  const chunk = isChunkLoadError(error);
  const onCopy = async () => {
    const text = await buildCrashReport({ error, componentStack, region });
    setReport(text);
    setCopy((await copyText(text)) ? 'copied' : 'failed');
  };
  const reload = () => window.location.reload();
  const app = variant === 'app';
  return (
    <div className={app ? 'crash crash-app' : 'crash crash-pane'} role="alert">
      <h2 className="crash-title">{t(app ? 'crash.title' : 'crash.paneTitle')}</h2>
      <p className="crash-body">{t(chunk ? 'crash.chunkBody' : app ? 'crash.body' : 'crash.paneBody')}</p>
      <p className="crash-error"><code>{errorText(error)}</code></p>
      <div className="btn-group">
        {app || chunk
          ? <button type="button" className="btn btn-primary" onClick={reload}>{t('crash.reload')}</button>
          : <button type="button" className="btn btn-primary" onClick={onRetry}>{t('crash.retry')}</button>}
        <button type="button" className="btn btn-secondary" onClick={onCopy}>{t('crash.copy')}</button>
        {copy === 'copied' && <span className="crash-copied" role="status">{t('crash.copied')}</span>}
      </div>
      {copy === 'failed' && <>
        <p className="crash-body">{t('crash.copyFailed')}</p>
        <textarea className="crash-report" readOnly value={report} rows={8} onFocus={(e) => e.currentTarget.select()} />
      </>}
    </div>
  );
}

export default ErrorBoundary;
