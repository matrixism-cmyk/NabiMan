import React, {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
} from 'react';
import { DEFAULT_THEME, THEMES, ThemeId, ThemePreset } from './presets';

interface ThemeContextValue {
  theme: ThemePreset;
  themeId: ThemeId;
  setTheme: (id: ThemeId) => void;
}

const ThemeContext = createContext<ThemeContextValue | null>(null);

const STORAGE_KEY = 'nabiman_theme';

function readStored(): ThemeId {
  try {
    const v = localStorage.getItem(STORAGE_KEY);
    if (v && v in THEMES) return v as ThemeId;
  } catch {
    /* ignore */
  }
  return DEFAULT_THEME;
}

/// CSS custom properties 를 document.documentElement 에 주입하여 App.css /
/// inline 스타일에서 var(--primary) 등으로 참조 가능하게 한다.
function applyTheme(id: ThemeId) {
  const t = THEMES[id];
  const root = document.documentElement;
  root.setAttribute('data-theme', id);
  root.setAttribute('data-theme-mode', t.mode);
  root.style.setProperty('--bg', t.bg);
  root.style.setProperty('--surface', t.surface);
  root.style.setProperty('--surface-alt', t.surfaceAlt);
  root.style.setProperty('--border', t.border);
  root.style.setProperty('--text', t.text);
  root.style.setProperty('--text-secondary', t.textSecondary);
  root.style.setProperty('--primary', t.primary);
  root.style.setProperty('--primary-fg', t.primaryFg);
  root.style.setProperty('--success', t.success);
  root.style.setProperty('--warning', t.warning);
  root.style.setProperty('--danger', t.danger);
  root.style.colorScheme = t.mode;
}

export function ThemeProvider({ children }: { children: React.ReactNode }) {
  const [themeId, setThemeIdState] = useState<ThemeId>(() => readStored());

  useEffect(() => {
    applyTheme(themeId);
  }, [themeId]);

  const setTheme = useCallback((id: ThemeId) => {
    setThemeIdState(id);
    try {
      localStorage.setItem(STORAGE_KEY, id);
    } catch {
      /* ignore */
    }
  }, []);

  const value: ThemeContextValue = useMemo(
    () => ({ theme: THEMES[themeId], themeId, setTheme }),
    [themeId, setTheme],
  );

  return <ThemeContext.Provider value={value}>{children}</ThemeContext.Provider>;
}

export function useTheme(): ThemeContextValue {
  const ctx = useContext(ThemeContext);
  if (!ctx) {
    // ThemeProvider 바깥에서 호출됐을 때는 기본값 반환 (테스트/스토리북 안전).
    return {
      theme: THEMES[DEFAULT_THEME],
      themeId: DEFAULT_THEME,
      setTheme: () => {},
    };
  }
  return ctx;
}

export { THEMES, THEME_ORDER } from './presets';
export type { ThemeId, ThemePreset, ThemeTokens } from './presets';
