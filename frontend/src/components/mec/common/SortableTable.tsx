import React, { useMemo, useState } from 'react';
import { useT } from '../../../i18n';

export interface Column<T> {
  key: string;
  header: React.ReactNode;
  /** Raw comparable value for sorting. If omitted, uses render() when string/number. */
  accessor?: (row: T) => string | number | boolean | null | undefined;
  render?: (row: T) => React.ReactNode;
  align?: 'left' | 'right' | 'center';
  width?: string | number;
  sortable?: boolean;
  headerTitle?: string;
  /** Allow this column's cells to wrap onto multiple lines. Cells stay on a
   *  single line by default so short values (timestamps, IDs) never break. */
  wrap?: boolean;
}

interface Props<T> {
  data: T[];
  columns: Column<T>[];
  rowKey: (row: T, idx: number) => string | number;
  onRowClick?: (row: T) => void;
  emptyMessage?: React.ReactNode;
  defaultSortKey?: string;
  defaultSortDir?: 'asc' | 'desc';
  dense?: boolean;
  stickyHeader?: boolean;
  maxHeight?: string | number;
  /** Optional client-side filter input value applied across all string columns. */
  filter?: string;
  /** Row keys to animate in (e.g. newly streamed rows in a live feed). */
  highlightRowKeys?: Set<string | number>;
}

function compare(a: unknown, b: unknown): number {
  const aNil = a === null || a === undefined || a === '';
  const bNil = b === null || b === undefined || b === '';
  if (aNil && bNil) return 0;
  if (aNil) return 1;
  if (bNil) return -1;
  if (typeof a === 'number' && typeof b === 'number') return a - b;
  const as = String(a);
  const bs = String(b);
  const aNum = Number(as);
  const bNum = Number(bs);
  if (!Number.isNaN(aNum) && !Number.isNaN(bNum) && as.trim() && bs.trim()) {
    return aNum - bNum;
  }
  return as.localeCompare(bs, undefined, { numeric: true, sensitivity: 'base' });
}

export default function SortableTable<T>({
  data,
  columns,
  rowKey,
  onRowClick,
  emptyMessage,
  defaultSortKey,
  defaultSortDir = 'asc',
  dense = false,
  stickyHeader = false,
  maxHeight,
  filter,
  highlightRowKeys,
}: Props<T>) {
  const { t } = useT();
  const [sortKey, setSortKey] = useState<string | null>(defaultSortKey ?? null);
  const [sortDir, setSortDir] = useState<'asc' | 'desc'>(defaultSortDir);

  const handleSort = (col: Column<T>) => {
    if (col.sortable === false) return;
    if (sortKey === col.key) {
      setSortDir((d) => (d === 'asc' ? 'desc' : 'asc'));
    } else {
      setSortKey(col.key);
      setSortDir('asc');
    }
  };

  const filtered = useMemo(() => {
    if (!filter) return data;
    const q = filter.toLowerCase();
    return data.filter((row) =>
      columns.some((c) => {
        const v = c.accessor ? c.accessor(row) : '';
        return String(v ?? '').toLowerCase().includes(q);
      }),
    );
  }, [data, columns, filter]);

  const sorted = useMemo(() => {
    if (!sortKey) return filtered;
    const col = columns.find((c) => c.key === sortKey);
    if (!col || !col.accessor) return filtered;
    const arr = [...filtered];
    arr.sort((x, y) => {
      const c = compare(col.accessor!(x), col.accessor!(y));
      return sortDir === 'asc' ? c : -c;
    });
    return arr;
  }, [filtered, columns, sortKey, sortDir]);

  const th = (col: Column<T>) => {
    const isSorted = sortKey === col.key;
    const arrow = isSorted ? (sortDir === 'asc' ? '▲' : '▼') : '';
    return (
      <th
        key={col.key}
        title={col.headerTitle}
        onClick={col.sortable === false ? undefined : () => handleSort(col)}
        style={{
          textAlign: col.align || 'left',
          width: col.width,
          padding: dense ? '6px 10px' : '10px 12px',
          whiteSpace: 'nowrap',
          cursor: col.sortable === false ? 'default' : 'pointer',
          userSelect: 'none',
          background: 'var(--surface-alt)',
          borderBottom: '1px solid var(--border)',
          fontSize: '12px',
          fontWeight: 600,
          color: isSorted ? 'var(--text)' : 'var(--text-secondary)',
          textTransform: 'none',
          letterSpacing: 0,
          position: stickyHeader ? 'sticky' : undefined,
          top: stickyHeader ? 0 : undefined,
          zIndex: stickyHeader ? 1 : undefined,
        }}
      >
        <span style={{ display: 'inline-flex', alignItems: 'center', gap: '4px' }}>
          {col.header}
          {arrow && (
            <span style={{ color: 'var(--text-secondary)', fontSize: '9px' }}>
              {arrow}
            </span>
          )}
        </span>
      </th>
    );
  };

  return (
    <div
      style={{
        border: '1px solid var(--border)',
        borderRadius: '6px',
        overflow: 'auto',
        maxHeight,
        background: 'var(--surface)',
      }}
    >
      <table
        style={{
          width: '100%',
          borderCollapse: 'collapse',
          fontSize: '13px',
          fontVariantNumeric: 'tabular-nums',
          color: 'var(--text)',
        }}
      >
        <thead>
          <tr>{columns.map(th)}</tr>
        </thead>
        <tbody>
          {sorted.length === 0 && (
            <tr>
              <td
                colSpan={columns.length}
                style={{
                  padding: '24px 12px',
                  textAlign: 'center',
                  color: 'var(--text-secondary)',
                  fontStyle: 'italic',
                }}
              >
                {emptyMessage ?? t('mec.table.empty')}
              </td>
            </tr>
          )}
          {sorted.map((row, idx) => {
            const rk = rowKey(row, idx);
            return (
            <tr
              key={rk}
              className={highlightRowKeys?.has(rk) ? 'mec-row-in' : undefined}
              onClick={onRowClick ? () => onRowClick(row) : undefined}
              style={{
                cursor: onRowClick ? 'pointer' : undefined,
                borderBottom: '1px solid var(--border)',
              }}
              onMouseEnter={
                onRowClick
                  ? (e) =>
                      ((e.currentTarget as HTMLElement).style.background =
                        'var(--surface-alt)')
                  : undefined
              }
              onMouseLeave={
                onRowClick
                  ? (e) =>
                      ((e.currentTarget as HTMLElement).style.background =
                        'transparent')
                  : undefined
              }
            >
              {columns.map((col) => (
                <td
                  key={col.key}
                  style={{
                    padding: dense ? '6px 10px' : '10px 12px',
                    textAlign: col.align || 'left',
                    whiteSpace: col.wrap ? 'normal' : 'nowrap',
                    color: 'var(--text)',
                  }}
                >
                  {col.render ? col.render(row) : String(col.accessor?.(row) ?? '')}
                </td>
              ))}
            </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}
