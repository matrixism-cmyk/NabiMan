import React from 'react';

interface PanelProps {
  title: React.ReactNode;
  subtitle?: React.ReactNode;
  actions?: React.ReactNode;
  children: React.ReactNode;
}

/// 모든 MEC 패널이 공유하는 표준 레이아웃: 제목, 보조 텍스트, 우측 액션 버튼,
/// 본문. 일관된 spacing + typography 를 강제한다.
export default function PanelLayout({
  title,
  subtitle,
  actions,
  children,
}: PanelProps) {
  return (
    <div className="panel" style={{ padding: 0 }}>
      <div
        style={{
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          padding: '16px 20px',
          borderBottom: '1px solid var(--border)',
          gap: '12px',
          flexWrap: 'wrap',
        }}
      >
        <div>
          <h2
            style={{
              margin: 0,
              fontSize: '18px',
              fontWeight: 600,
              color: 'var(--text)',
              lineHeight: 1.3,
            }}
          >
            {title}
          </h2>
          {subtitle && (
            <div
              style={{
                fontSize: '13px',
                color: 'var(--text-secondary)',
                marginTop: '4px',
              }}
            >
              {subtitle}
            </div>
          )}
        </div>
        {actions && (
          <div style={{ display: 'flex', gap: '8px', alignItems: 'center' }}>
            {actions}
          </div>
        )}
      </div>
      <div style={{ padding: '16px 20px' }}>{children}</div>
    </div>
  );
}

export function Section({
  title,
  actions,
  children,
  marginTop,
}: {
  title?: React.ReactNode;
  actions?: React.ReactNode;
  children: React.ReactNode;
  marginTop?: string | number;
}) {
  return (
    <div style={{ marginTop }}>
      {(title || actions) && (
        <div
          style={{
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'space-between',
            marginBottom: '8px',
          }}
        >
          {title && (
            <h3
              style={{
                margin: 0,
                fontSize: '14px',
                fontWeight: 600,
                color: 'var(--text-secondary)',
                letterSpacing: '0.02em',
              }}
            >
              {title}
            </h3>
          )}
          {actions && <div>{actions}</div>}
        </div>
      )}
      {children}
    </div>
  );
}

export function FieldGrid({ children }: { children: React.ReactNode }) {
  return (
    <div
      style={{
        display: 'grid',
        gridTemplateColumns: 'repeat(auto-fit, minmax(180px, 1fr))',
        gap: '12px 20px',
      }}
    >
      {children}
    </div>
  );
}

export function Field({
  label,
  value,
}: {
  label: string;
  value: React.ReactNode;
}) {
  return (
    <div>
      <div
        style={{
          fontSize: '11px',
          textTransform: 'uppercase',
          letterSpacing: '0.06em',
          color: 'var(--text-secondary)',
          marginBottom: '2px',
        }}
      >
        {label}
      </div>
      <div
        style={{ fontSize: '14px', color: 'var(--text)', fontWeight: 500 }}
      >
        {value || (
          <span style={{ color: 'var(--text-secondary)', opacity: 0.6 }}>-</span>
        )}
      </div>
    </div>
  );
}

export function ErrorBanner({ error }: { error?: string | null }) {
  if (!error) return null;
  return (
    <div
      style={{
        background: '#fee2e2',
        color: '#991b1b',
        padding: '8px 12px',
        borderRadius: '6px',
        borderLeft: '3px solid #ef4444',
        fontSize: '13px',
        marginBottom: '12px',
      }}
    >
      {error}
    </div>
  );
}

export function Toolbar({
  children,
  marginBottom = '12px',
}: {
  children: React.ReactNode;
  marginBottom?: string | number;
}) {
  return (
    <div
      style={{
        display: 'flex',
        alignItems: 'center',
        gap: '8px',
        flexWrap: 'wrap',
        marginBottom,
      }}
    >
      {children}
    </div>
  );
}
