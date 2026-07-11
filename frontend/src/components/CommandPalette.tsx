import React, { useEffect, useMemo, useRef, useState } from 'react';
import { useT } from '../i18n';
import { Category, Tab, categories } from '../nav';

interface Props {
  onClose: () => void;
  onSelect: (cat: Category, tab: Tab) => void;
  hiddenTabs: Set<Tab>;
}

interface Entry {
  cat: Category;
  tab: Tab;
  label: string;
  catLabel: string;
}

/**
 * Cmd/Ctrl+K command palette: fuzzy-filter every navigable tab and jump to it.
 * Keyboard-first — ↑/↓ to move, Enter to open, Esc to close.
 */
export default function CommandPalette({ onClose, onSelect, hiddenTabs }: Props) {
  const { t } = useT();
  const [query, setQuery] = useState('');
  const [active, setActive] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLDivElement>(null);

  const all: Entry[] = useMemo(
    () =>
      categories.flatMap((c) =>
        c.tabs
          .filter((tb) => !hiddenTabs.has(tb.key))
          .map((tb) => ({ cat: c.key, tab: tb.key, label: t(tb.labelKey), catLabel: t(c.labelKey) })),
      ),
    [t, hiddenTabs],
  );

  const results = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return all;
    return all.filter(
      (e) => e.label.toLowerCase().includes(q) || e.catLabel.toLowerCase().includes(q),
    );
  }, [all, query]);

  useEffect(() => { inputRef.current?.focus(); }, []);
  useEffect(() => { setActive(0); }, [query]);
  useEffect(() => {
    listRef.current?.querySelector('[data-active="true"]')?.scrollIntoView({ block: 'nearest' });
  }, [active]);

  const choose = (e?: Entry) => { if (e) { onSelect(e.cat, e.tab); onClose(); } };

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'ArrowDown') { e.preventDefault(); setActive((i) => Math.min(i + 1, results.length - 1)); }
    else if (e.key === 'ArrowUp') { e.preventDefault(); setActive((i) => Math.max(i - 1, 0)); }
    else if (e.key === 'Enter') { e.preventDefault(); choose(results[active]); }
    else if (e.key === 'Escape') { e.preventDefault(); onClose(); }
  };

  return (
    <div className="cmdk-overlay" onMouseDown={onClose}>
      <div className="cmdk-panel" onMouseDown={(e) => e.stopPropagation()} onKeyDown={onKeyDown}>
        <input
          ref={inputRef}
          className="cmdk-input"
          placeholder={t('palette.placeholder')}
          value={query}
          onChange={(e) => setQuery(e.target.value)}
        />
        <div className="cmdk-list" ref={listRef}>
          {results.length === 0 && <div className="cmdk-empty">{t('palette.empty')}</div>}
          {results.map((e, i) => (
            <div
              key={`${e.cat}/${e.tab}`}
              data-active={i === active}
              className={`cmdk-item ${i === active ? 'cmdk-item-active' : ''}`}
              onMouseEnter={() => setActive(i)}
              onClick={() => choose(e)}
            >
              <span>{e.label}</span>
              <span className="cmdk-cat">{e.catLabel}</span>
            </div>
          ))}
        </div>
        <div className="cmdk-hint">{t('palette.hint')}</div>
      </div>
    </div>
  );
}
