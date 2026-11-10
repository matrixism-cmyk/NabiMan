import React, { useState, useMemo } from 'react';
import { onActivateKey } from '../shared/ui/activate';

export type SortDir = 'asc' | 'desc';

export interface SortState {
  key: string;
  dir: SortDir;
}

export function useSortable<T>(data: T[], defaultKey?: string, defaultDir: SortDir = 'asc') {
  const [sort, setSort] = useState<SortState>({ key: defaultKey || '', dir: defaultDir });

  const toggle = (key: string) => {
    setSort(prev => prev.key === key ? { key, dir: prev.dir === 'asc' ? 'desc' : 'asc' } : { key, dir: 'asc' });
  };

  const sorted = useMemo(() => {
    if (!sort.key) return data;
    return [...data].sort((a, b) => {
      const av = (a as any)[sort.key];
      const bv = (b as any)[sort.key];
      if (av == null && bv == null) return 0;
      if (av == null) return 1;
      if (bv == null) return -1;
      let cmp: number;
      if (typeof av === 'number' && typeof bv === 'number') {
        cmp = av - bv;
      } else if (typeof av === 'boolean') {
        cmp = (av === bv) ? 0 : av ? -1 : 1;
      } else {
        cmp = String(av).localeCompare(String(bv), undefined, { numeric: true, sensitivity: 'base' });
      }
      return sort.dir === 'desc' ? -cmp : cmp;
    });
  }, [data, sort]);

  const indicator = (key: string) => sort.key === key ? (sort.dir === 'asc' ? ' ▲' : ' ▼') : '';

  /** A sortable column heading's props: reachable by Tab, sorted by Enter or Space, its order announced. */
  const headerProps = (key: string) => ({
    className: 'sortable',
    tabIndex: 0,
    'aria-sort': (sort.key === key ? (sort.dir === 'asc' ? 'ascending' : 'descending') : 'none') as React.AriaAttributes['aria-sort'],
    onClick: () => toggle(key),
    onKeyDown: onActivateKey(() => toggle(key)),
  });

  return { sorted, sort, toggle, indicator, headerProps };
}
