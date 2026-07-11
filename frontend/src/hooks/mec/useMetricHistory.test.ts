import { renderHook } from '@testing-library/react';
import { useMetricHistory, trendOf } from './useMetricHistory';

describe('trendOf', () => {
  it('is flat without at least two samples', () => {
    expect(trendOf(undefined)).toBe('flat');
    expect(trendOf([])).toBe('flat');
    expect(trendOf([5])).toBe('flat');
  });
  it('reads direction from the last two samples', () => {
    expect(trendOf([1, 2])).toBe('up');
    expect(trendOf([2, 1])).toBe('down');
    expect(trendOf([3, 3])).toBe('flat');
    expect(trendOf([9, 1, 5])).toBe('up');
  });
});

describe('useMetricHistory', () => {
  it('appends one sample per distinct current object', () => {
    const { result, rerender } = renderHook(
      ({ cur }) => useMetricHistory('t-key', cur, false),
      { initialProps: { cur: { cpu: 10 } as Record<string, number> | null } },
    );
    expect(result.current.cpu).toEqual([10]);
    rerender({ cur: { cpu: 20 } });
    expect(result.current.cpu).toEqual([10, 20]);
    rerender({ cur: { cpu: 20 } }); // new object, same value -> still advances
    expect(result.current.cpu).toEqual([10, 20, 20]);
  });

  it('does not write localStorage when persist is false', () => {
    const spy = jest.spyOn(Storage.prototype, 'setItem');
    const { rerender } = renderHook(
      ({ cur }) => useMetricHistory('t-key-2', cur, false),
      { initialProps: { cur: { mem: 1 } as Record<string, number> | null } },
    );
    rerender({ cur: { mem: 2 } });
    expect(spy).not.toHaveBeenCalledWith('t-key-2', expect.anything());
    spy.mockRestore();
  });
});
