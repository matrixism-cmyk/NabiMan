import { act, renderHook } from '@testing-library/react';
import { useDictation } from './useDictation';

class FakeSocket {
  static last: FakeSocket;
  readyState = 0;
  binaryType = '';
  sent: unknown[] = [];
  onopen: (() => void) | null = null;
  onclose: (() => void) | null = null;
  onerror: (() => void) | null = null;
  onmessage: ((e: { data: unknown }) => void) | null = null;
  constructor() { FakeSocket.last = this; setTimeout(() => { this.readyState = 1; this.onopen?.(); }); }
  send(data: unknown) { this.sent.push(data); }
  close() { this.readyState = 3; this.onclose?.(); }
  hear(msg: object) { this.onmessage?.({ data: JSON.stringify(msg) }); }
}

beforeAll(() => {
  Object.assign(window, { WebSocket: Object.assign(FakeSocket, { OPEN: 1, CONNECTING: 0 }) });
  class FakeAudio {
    sampleRate = 48000;
    audioWorklet = { addModule: () => Promise.resolve() };
    createMediaStreamSource() { return { connect: jest.fn() }; }
    close() { return Promise.resolve(); }
  }
  Object.assign(window, {
    AudioContext: FakeAudio,
    AudioWorkletNode: class { port = { onmessage: null }; },
  });
});

// The project's jest setup resets mocks before each test.
beforeEach(() => {
  jest.useFakeTimers();
  Object.assign(navigator, { mediaDevices: { getUserMedia: jest.fn(() => Promise.resolve({ getTracks: () => [{ stop: () => undefined }] })) } });
  URL.createObjectURL = jest.fn(() => 'blob:tap');
  URL.revokeObjectURL = jest.fn();
});
afterEach(() => jest.useRealTimers());

const setup = () => {
  const onText = jest.fn();
  const onKeys = jest.fn();
  const view = renderHook(() => useDictation({ open: () => Promise.resolve(new FakeSocket() as unknown as WebSocket), onText, onKeys }));
  return { ...view, onText, onKeys };
};

const begin = async (result: ReturnType<typeof setup>['result']) => {
  await act(async () => {
    const started = result.current.start();
    // Let the microphone and socket promises settle, then the socket open.
    for (let i = 0; i < 10; i += 1) await Promise.resolve();
    jest.advanceTimersByTime(10);
    await started;
  });
};

test('a phrase is typed once the speaker pauses, and "엔터" presses Enter', async () => {
  const { result, onText, onKeys } = setup();
  await begin(result);
  expect(result.current.state).toBe('listening');
  expect(FakeSocket.last.sent[0]).toBe('{}');
  act(() => FakeSocket.last.hear({ type: 'partial', committed: '', partial: 'ls -al' }));
  expect(onText).not.toHaveBeenCalled();
  expect(result.current.partial).toBe('ls -al');
  // The same text again: a pause.
  act(() => FakeSocket.last.hear({ type: 'partial', committed: '', partial: 'ls -al' }));
  expect(onText).toHaveBeenCalledWith('ls -al');
  act(() => FakeSocket.last.hear({ type: 'partial', committed: '', partial: 'ls -al 엔터.' }));
  act(() => { jest.advanceTimersByTime(1300); });
  expect(onKeys).toHaveBeenCalledWith('\r');
  expect(onText).toHaveBeenCalledTimes(1);
});

test('a correction erases what changed; stopping types the last words', async () => {
  const { result, onText, onKeys } = setup();
  await begin(result);
  act(() => { FakeSocket.last.hear({ type: 'partial', committed: '', partial: '오늘 날씨.' }); jest.advanceTimersByTime(1300); });
  expect(onText).toHaveBeenLastCalledWith('오늘 날씨.');
  // "stop" follows whatever the encoder still holds: a moment later.
  await act(async () => { result.current.stop(); await Promise.resolve(); await Promise.resolve(); });
  expect(FakeSocket.last.sent).toContain(JSON.stringify({ type: 'stop' }));
  act(() => FakeSocket.last.hear({ type: 'final', committed: '오늘 날씨가 좋다', partial: '' }));
  expect(onKeys).toHaveBeenCalledWith('\x7f');
  expect(onText).toHaveBeenLastCalledWith('가 좋다');
  act(() => FakeSocket.last.close());
  expect(result.current.state).toBe('idle');
});

test('a blocked microphone is said so', async () => {
  (navigator.mediaDevices.getUserMedia as jest.Mock).mockImplementationOnce(() => Promise.reject(new DOMException('no', 'NotAllowedError')));
  const { result } = setup();
  await act(async () => { await result.current.start(); });
  expect(result.current.error).toBe('denied');
  expect(result.current.state).toBe('idle');
});

const setupPauses = (enterOnPause: boolean) => {
  const onText = jest.fn();
  const onKeys = jest.fn();
  const view = renderHook(() => useDictation({
    open: () => Promise.resolve(new FakeSocket() as unknown as WebSocket), onText, onKeys, endpointMs: 1500, enterOnPause,
  }));
  return { ...view, onText, onKeys };
};

test('with end-of-speech detection, words show until the service says they are done', async () => {
  const { result, onText, onKeys } = setupPauses(false);
  await begin(result);
  expect(FakeSocket.last.sent[0]).toBe('{"endpoint_ms":1500}');
  act(() => FakeSocket.last.hear({ type: 'partial', committed: '', partial: 'git status', utterance: 'git status' }));
  act(() => FakeSocket.last.hear({ type: 'partial', committed: '', partial: 'git status', utterance: 'git status' }));
  act(() => { jest.advanceTimersByTime(3000); });
  expect(onText).not.toHaveBeenCalled();
  expect(result.current.partial).toBe('git status');
  act(() => FakeSocket.last.hear({ type: 'endpoint', text: 'git status', reason: 'silence' }));
  expect(onText).toHaveBeenCalledWith('git status');
  expect(onKeys).not.toHaveBeenCalled();
  expect(result.current.partial).toBe('');
  act(() => FakeSocket.last.hear({ type: 'endpoint', text: '--short 엔터', reason: 'silence' }));
  expect(onText).toHaveBeenLastCalledWith(' --short');
  expect(onKeys).toHaveBeenCalledWith('\r');
  // What came with the stop is in an endpoint first; the final adds nothing.
  act(() => FakeSocket.last.hear({ type: 'final', committed: 'git status --short 엔터', partial: '' }));
  expect(onText).toHaveBeenCalledTimes(2);
});

test('sending on a pause presses Enter after each utterance', async () => {
  const { result, onText, onKeys } = setupPauses(true);
  await begin(result);
  act(() => FakeSocket.last.hear({ type: 'endpoint', text: '사과는 무슨 색이야', reason: 'silence' }));
  expect(onText).toHaveBeenCalledWith('사과는 무슨 색이야');
  expect(onKeys).toHaveBeenCalledWith('\r');
});

test('an older service with no end-of-speech detection keeps the old way', async () => {
  const { result, onText } = setupPauses(false);
  await begin(result);
  act(() => FakeSocket.last.hear({ type: 'partial', committed: '', partial: 'uptime' }));
  act(() => FakeSocket.last.hear({ type: 'partial', committed: '', partial: 'uptime' }));
  expect(onText).toHaveBeenCalledWith('uptime');
});

test('"내 목소리만" is asked for in the opening message, and the service\'s refusal is shown', async () => {
  const view = renderHook(() => useDictation({
    open: () => Promise.resolve(new FakeSocket() as unknown as WebSocket), onText: jest.fn(), onKeys: jest.fn(), endpointMs: 1500, voiceLock: true,
  }));
  await begin(view.result);
  expect(JSON.parse(FakeSocket.last.sent[0] as string)).toEqual({ endpoint_ms: 1500, voice_lock: true });
  act(() => FakeSocket.last.hear({ type: 'error', message: '먼저 계정 화면에서 목소리를 등록해 주세요.' }));
  expect(view.result.current.error).toBe('먼저 계정 화면에서 목소리를 등록해 주세요.');
});


describe('"음성으로 대답해줘"', () => {
  const setupAsk = (enterOnPause: boolean, endpointMs = 1500) => {
    const onText = jest.fn();
    const onKeys = jest.fn();
    const onVoiceAsk = jest.fn();
    const view = renderHook(() => useDictation({
      open: () => Promise.resolve(new FakeSocket() as unknown as WebSocket), onText, onKeys, endpointMs, enterOnPause, onVoiceAsk,
    }));
    return { ...view, onText, onKeys, onVoiceAsk };
  };

  it('is not typed, and its line running asks for the answer read aloud', async () => {
    const { result, onText, onKeys, onVoiceAsk } = setupAsk(true);
    await begin(result);
    act(() => FakeSocket.last.hear({ type: 'endpoint', text: '사과는 무슨 색이야 음성으로 대답해줘', reason: 'silence' }));
    expect(onText).toHaveBeenCalledWith('사과는 무슨 색이야');
    expect(onKeys).toHaveBeenCalledWith('\r');
    expect(onVoiceAsk).toHaveBeenCalledTimes(1);
  });

  it('waits for "엔터" when a pause does not send', async () => {
    const { result, onKeys, onVoiceAsk } = setupAsk(false);
    await begin(result);
    act(() => FakeSocket.last.hear({ type: 'endpoint', text: '디스크 사용량 알려줘 소리로 알려줘', reason: 'silence' }));
    expect(onVoiceAsk).not.toHaveBeenCalled();
    act(() => FakeSocket.last.hear({ type: 'endpoint', text: '엔터', reason: 'silence' }));
    expect(onKeys).toHaveBeenCalledWith('\r');
    expect(onVoiceAsk).toHaveBeenCalledTimes(1);
  });

  it('said on its own just after Enter, is for the line that ran', async () => {
    const { result, onText, onVoiceAsk } = setupAsk(true);
    await begin(result);
    act(() => FakeSocket.last.hear({ type: 'endpoint', text: '오늘 할 일 정리해줘', reason: 'silence' }));
    act(() => FakeSocket.last.hear({ type: 'endpoint', text: '음성으로 대답해 주세요', reason: 'silence' }));
    expect(onText).toHaveBeenCalledTimes(1);
    expect(onVoiceAsk).toHaveBeenCalledTimes(1);
  });

  it('with the page guessing pauses, the request is erased as it is recognised, and Enter asks', async () => {
    const { result, onText, onKeys, onVoiceAsk } = setupAsk(false, 0);
    await begin(result);
    act(() => FakeSocket.last.hear({ type: 'partial', committed: '', partial: '날씨 알려줘 음성으로 대답해줘' }));
    act(() => FakeSocket.last.hear({ type: 'partial', committed: '', partial: '날씨 알려줘 음성으로 대답해줘' }));
    expect(onText).toHaveBeenCalledWith('날씨 알려줘');
    act(() => FakeSocket.last.hear({ type: 'partial', committed: '', partial: '날씨 알려줘 음성으로 대답해줘 엔터' }));
    act(() => { jest.advanceTimersByTime(1300); });
    expect(onKeys).toHaveBeenCalledWith('\r');
    expect(onVoiceAsk).toHaveBeenCalledTimes(1);
  });

  it('is typed like any words where nothing can read the answer', async () => {
    const { result, onText } = setupPauses(true);
    await begin(result);
    act(() => FakeSocket.last.hear({ type: 'endpoint', text: '음성으로 대답해줘', reason: 'silence' }));
    expect(onText).toHaveBeenCalledWith('음성으로 대답해줘');
  });
});
