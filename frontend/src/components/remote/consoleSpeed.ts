/** How a console trades looks for speed. The server knows the same three names. */
export type ConsoleSpeed = 'fast' | 'balanced' | 'sharp';

export const SPEEDS: ConsoleSpeed[] = ['fast', 'balanced', 'sharp'];
export const DEFAULT_SPEED: ConsoleSpeed = 'balanced';

const KEY = 'nabiman_console_speed';

function readAll(): Record<string, unknown> {
  try {
    const value = JSON.parse(localStorage.getItem(KEY) || '{}');
    return value && typeof value === 'object' && !Array.isArray(value) ? value : {};
  } catch { return {}; }
}

/** The speed chosen for a console in this browser, or the default. */
export function readSpeed(consoleKey: string): ConsoleSpeed {
  const saved = readAll()[consoleKey];
  return SPEEDS.includes(saved as ConsoleSpeed) ? saved as ConsoleSpeed : DEFAULT_SPEED;
}

export function rememberSpeed(consoleKey: string, speed: ConsoleSpeed): void {
  try { localStorage.setItem(KEY, JSON.stringify({ ...readAll(), [consoleKey]: speed })); } catch { /* private mode: this visit only */ }
}

/** noVNC's JPEG quality (0–9, higher is sharper) and compression effort (0–9,
 *  higher is smaller) for each speed. `balanced` is noVNC's own default. */
export const VNC_LEVELS: Record<ConsoleSpeed, { quality: number; compression: number }> = {
  fast: { quality: 3, compression: 6 },
  balanced: { quality: 6, compression: 2 },
  sharp: { quality: 9, compression: 1 },
};
