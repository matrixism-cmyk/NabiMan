export type ThemeId = 'carbon' | 'midnight' | 'forest' | 'royal' | 'light' | 'slate';

export interface ThemeTokens {
  bg: string;
  surface: string;
  surfaceAlt: string;
  border: string;
  text: string;
  textSecondary: string;
  primary: string;
  primaryFg: string;
  success: string;
  warning: string;
  danger: string;
  /** Whether panel backgrounds should be near-white (light mode) or dark surfaces. */
  mode: 'dark' | 'light';
}

export interface ThemePreset extends ThemeTokens {
  id: ThemeId;
  label: string;
  description: string;
}

/// 정의된 6종 테마 — Carbon(기본 다크)/Midnight/Forest/Royal/Light/Slate.
/// mode 에 따라 MEC common 컴포넌트가 패널 배경색·텍스트를 조정한다.
export const THEMES: Record<ThemeId, ThemePreset> = {
  carbon: {
    id: 'carbon',
    label: 'Carbon (기본)',
    description: '기존 NabiMan 어두운 테마',
    bg: '#0f1923',
    surface: '#1a2634',
    surfaceAlt: '#243241',
    border: '#2a3a4a',
    text: '#e0e6ed',
    textSecondary: '#8899aa',
    primary: '#3b82f6',
    primaryFg: '#ffffff',
    success: '#2ecc71',
    warning: '#f39c12',
    danger: '#e74c3c',
    mode: 'dark',
  },
  midnight: {
    id: 'midnight',
    label: 'Midnight Blue',
    description: '짙은 네이비 + 시안 포인트',
    bg: '#0a1128',
    surface: '#121a3b',
    surfaceAlt: '#1b2554',
    border: '#2d3d72',
    text: '#e6edf7',
    textSecondary: '#7e8dc4',
    primary: '#38bdf8',
    primaryFg: '#0a1128',
    success: '#34d399',
    warning: '#fbbf24',
    danger: '#f87171',
    mode: 'dark',
  },
  forest: {
    id: 'forest',
    label: 'Forest',
    description: '딥 그린 + 골드 포인트',
    bg: '#0f1a14',
    surface: '#182922',
    surfaceAlt: '#223b32',
    border: '#2f4a3d',
    text: '#e7efe8',
    textSecondary: '#8ba793',
    primary: '#22c55e',
    primaryFg: '#0f1a14',
    success: '#4ade80',
    warning: '#eab308',
    danger: '#ef4444',
    mode: 'dark',
  },
  royal: {
    id: 'royal',
    label: 'Royal Purple',
    description: '가든 퍼플 + 핑크 액센트',
    bg: '#1a0f2e',
    surface: '#2a1a44',
    surfaceAlt: '#3b2a5a',
    border: '#4c3a72',
    text: '#f0e8ff',
    textSecondary: '#a391c0',
    primary: '#a855f7',
    primaryFg: '#1a0f2e',
    success: '#10b981',
    warning: '#f59e0b',
    danger: '#ef4444',
    mode: 'dark',
  },
  light: {
    id: 'light',
    label: 'Light',
    description: '밝은 기본 테마',
    bg: '#f9fafb',
    surface: '#ffffff',
    surfaceAlt: '#f3f4f6',
    border: '#e5e7eb',
    text: '#111827',
    textSecondary: '#6b7280',
    primary: '#2563eb',
    primaryFg: '#ffffff',
    success: '#10b981',
    warning: '#f59e0b',
    danger: '#ef4444',
    mode: 'light',
  },
  slate: {
    id: 'slate',
    label: 'Slate',
    description: '중간 톤 회색 + 블루',
    bg: '#1e293b',
    surface: '#273347',
    surfaceAlt: '#334155',
    border: '#475569',
    text: '#f1f5f9',
    textSecondary: '#94a3b8',
    primary: '#60a5fa',
    primaryFg: '#0f172a',
    success: '#34d399',
    warning: '#fbbf24',
    danger: '#f87171',
    mode: 'dark',
  },
};

export const THEME_ORDER: ThemeId[] = [
  'carbon',
  'light',
  'midnight',
  'forest',
  'royal',
  'slate',
];

export const DEFAULT_THEME: ThemeId = 'carbon';
