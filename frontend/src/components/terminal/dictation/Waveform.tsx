import React, { useEffect, useRef } from 'react';
import { LevelMeter, METER_BARS } from './meter';

const H = 16;
const STEP = 3;
const MIN = 2;

/**
 * The microphone's loudness as bars running right to left, in the text
 * colour. Drawn by setting the bars' heights each frame, not by React.
 */
export default function Waveform({ meter }: { meter: LevelMeter }) {
  const ref = useRef<SVGSVGElement>(null);
  useEffect(() => {
    let frame = 0;
    let seen = -1;
    const draw = () => {
      frame = requestAnimationFrame(draw);
      const svg = ref.current;
      if (!svg || meter.count === seen) return;
      seen = meter.count;
      for (let i = 0; i < METER_BARS; i += 1) {
        const h = MIN + meter.bars[i] * (H - MIN);
        const bar = svg.children[i];
        bar.setAttribute('y', String((H - h) / 2));
        bar.setAttribute('height', String(h));
      }
    };
    frame = requestAnimationFrame(draw);
    return () => cancelAnimationFrame(frame);
  }, [meter]);
  return (
    <svg ref={ref} className="dictation-wave" viewBox={`0 0 ${METER_BARS * STEP - 1} ${H}`} aria-hidden="true">
      {Array.from({ length: METER_BARS }, (_, i) => (
        <rect key={i} x={i * STEP} y={(H - MIN) / 2} width="2" height={MIN} rx="1" fill="currentColor" />
      ))}
    </svg>
  );
}
