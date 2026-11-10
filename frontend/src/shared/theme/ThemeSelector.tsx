import React from 'react';
import { useTheme, THEMES, THEME_ORDER, ThemeId } from './ThemeProvider';
import { useT } from '../i18n';

interface Props {
  /** 상단 네비용 컴팩트 드롭다운. 기본값은 설정 패널용 라디오 카드. */
  compact?: boolean;
}

export default function ThemeSelector({ compact = false }: Props) {
  const { themeId, setTheme } = useTheme();
  const { t } = useT();

  if (compact) {
    return (
      <select
        className="lang-select"
        value={themeId}
        onChange={(e) => setTheme(e.target.value as ThemeId)}
        title={t('theme.select')}
        aria-label={t('theme.label')}
      >
        {THEME_ORDER.map((id) => (
          <option key={id} value={id}>
            {t(THEMES[id].label)}
          </option>
        ))}
      </select>
    );
  }

  return (
    <div
      style={{
        display: 'grid',
        gridTemplateColumns: 'repeat(auto-fit, minmax(220px, 1fr))',
        gap: '12px',
      }}
    >
      {THEME_ORDER.map((id) => {
        const theme = THEMES[id];
        const active = id === themeId;
        return (
          <button
            key={id}
            type="button"
            onClick={() => setTheme(id)}
            style={{
              textAlign: 'left',
              padding: '12px',
              borderRadius: '8px',
              border: active
                ? `2px solid ${theme.primary}`
                : '1px solid var(--border)',
              background: 'var(--surface)',
              cursor: 'pointer',
              color: 'var(--text)',
              boxShadow: active
                ? `0 0 0 3px ${theme.primary}33`
                : 'none',
              transition: 'box-shadow 0.15s, border-color 0.15s',
            }}
          >
            <div
              style={{
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'space-between',
                marginBottom: '8px',
              }}
            >
              <strong style={{ fontSize: '14px' }}>{t(theme.label)}</strong>
              {active && (
                <span
                  style={{
                    background: theme.primary,
                    color: theme.primaryFg,
                    padding: '2px 8px',
                    borderRadius: '999px',
                    fontSize: '11px',
                    fontWeight: 600,
                  }}
                >
                  {t('theme.active')}
                </span>
              )}
            </div>
            <div
              style={{
                fontSize: '12px',
                color: 'var(--text-secondary)',
                marginBottom: '10px',
              }}
            >
              {t(theme.description)}
            </div>
            <div style={{ display: 'flex', gap: '4px' }}>
              {[theme.primary, theme.success, theme.warning, theme.danger].map((c, i) => (
                <span
                  key={i}
                  style={{
                    background: c,
                    width: '24px',
                    height: '18px',
                    borderRadius: '3px',
                    border: '1px solid rgba(0,0,0,0.15)',
                  }}
                />
              ))}
              <span
                style={{
                  background: theme.surface,
                  border: `1px solid ${theme.border}`,
                  width: '24px',
                  height: '18px',
                  borderRadius: '3px',
                }}
              />
              <span
                style={{
                  background: theme.bg,
                  border: `1px solid ${theme.border}`,
                  width: '24px',
                  height: '18px',
                  borderRadius: '3px',
                }}
              />
            </div>
          </button>
        );
      })}
    </div>
  );
}
