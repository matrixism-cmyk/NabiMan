import {
  assertionJSON, creationOptions, fromBase64Url, passkeyAlreadyThere, passkeyCancelled, passkeysSupported,
  registrationJSON, requestOptions, toBase64Url,
} from './passkey';

const bytes = (...values: number[]) => new Uint8Array(values);

describe('base64url', () => {
  it('writes without padding and with the url-safe alphabet', () => {
    expect(toBase64Url(bytes())).toBe('');
    expect(toBase64Url(bytes(0xfb, 0xff))).toBe('-_8');
    expect(toBase64Url(bytes(0xfb, 0xff, 0xfe))).toBe('-__-');
    expect(toBase64Url(bytes(1, 2, 3, 4).buffer)).toBe('AQIDBA');
  });

  it('reads what it wrote, for every length and byte value', () => {
    for (let length = 0; length < 70; length += 1) {
      const data = new Uint8Array(length).map((_, i) => (i * 37 + length * 11) % 256);
      const text = toBase64Url(data);
      expect(text).not.toMatch(/[+/=]/);
      expect(Array.from(fromBase64Url(text))).toEqual(Array.from(data));
    }
    expect(Array.from(fromBase64Url(toBase64Url(new Uint8Array(256).map((_, i) => i))))).toEqual(Array.from({ length: 256 }, (_, i) => i));
  });

  it('also reads padded and standard-alphabet text, and refuses anything else', () => {
    expect(Array.from(fromBase64Url('+/8='))).toEqual([0xfb, 0xff]);
    expect(Array.from(fromBase64Url('AQIDBA=='))).toEqual([1, 2, 3, 4]);
    for (const bad of ['A', 'AQ ID', 'AQ*D', 'AQ=ID', '한글']) expect(() => fromBase64Url(bad)).toThrow();
  });
});

describe('options for navigator.credentials', () => {
  it('turns a registration into binary challenge, user id and excluded credentials, keeping the rest', () => {
    const options = creationOptions({
      challenge: 'AQID', user: { id: 'BAUG', name: 'kim', displayName: 'kim' },
      excludeCredentials: [{ type: 'public-key', id: 'BwgJ' }],
      rp: { id: 'man.example.com', name: 'NabiMan' }, attestation: 'none',
      authenticatorSelection: { residentKey: 'required', requireResidentKey: true, userVerification: 'required' },
    });
    expect(Array.from(options.challenge as Uint8Array)).toEqual([1, 2, 3]);
    expect(Array.from(options.user.id as Uint8Array)).toEqual([4, 5, 6]);
    expect(Array.from(options.excludeCredentials![0].id as Uint8Array)).toEqual([7, 8, 9]);
    expect(options.rp).toEqual({ id: 'man.example.com', name: 'NabiMan' });
    expect(options.authenticatorSelection).toEqual({ residentKey: 'required', requireResidentKey: true, userVerification: 'required' });
  });

  it('turns a sign-in into a binary challenge with no credentials named', () => {
    const options = requestOptions({ challenge: 'AQID', rpId: 'man.example.com', userVerification: 'required', allowCredentials: [] });
    expect(Array.from(options.challenge as Uint8Array)).toEqual([1, 2, 3]);
    expect(options.allowCredentials).toEqual([]);
    expect(options.rpId).toBe('man.example.com');
    expect(options.userVerification).toBe('required');
    expect(requestOptions({ challenge: 'AQID' }).allowCredentials).toEqual([]);
  });
});

describe('answers for the server', () => {
  it('writes a new passkey as base64url JSON', () => {
    const credential = { id: 'AQI', rawId: bytes(1, 2).buffer, type: 'public-key',
      response: { attestationObject: bytes(3).buffer, clientDataJSON: bytes(4, 5).buffer } } as unknown as PublicKeyCredential;
    expect(registrationJSON(credential)).toEqual({ id: 'AQI', rawId: 'AQI', type: 'public-key', response: { attestationObject: 'Aw', clientDataJSON: 'BAU' } });
  });

  it('writes a sign-in answer with the user handle the device reported', () => {
    const response = { authenticatorData: bytes(1).buffer, clientDataJSON: bytes(2).buffer, signature: bytes(3).buffer, userHandle: bytes(9, 9).buffer };
    const credential = { id: 'AQI', rawId: bytes(1, 2).buffer, type: 'public-key', response } as unknown as PublicKeyCredential;
    expect(assertionJSON(credential).response).toEqual({ authenticatorData: 'AQ', clientDataJSON: 'Ag', signature: 'Aw', userHandle: 'CQk' });
    const without = { ...credential, response: { ...response, userHandle: null } } as unknown as PublicKeyCredential;
    expect(assertionJSON(without).response.userHandle).toBeNull();
  });
});

describe('what the browser can do and said', () => {
  const original = Object.getOwnPropertyDescriptor(window, 'PublicKeyCredential');
  const credentials = Object.getOwnPropertyDescriptor(navigator, 'credentials');
  afterEach(() => {
    if (original) Object.defineProperty(window, 'PublicKeyCredential', original); else delete (window as { PublicKeyCredential?: unknown }).PublicKeyCredential;
    if (credentials) Object.defineProperty(navigator, 'credentials', credentials); else delete (navigator as { credentials?: unknown }).credentials;
  });

  it('needs both PublicKeyCredential and navigator.credentials', () => {
    expect(passkeysSupported()).toBe(false);
    Object.defineProperty(window, 'PublicKeyCredential', { configurable: true, value: function PublicKeyCredential() { /* stand-in */ } });
    expect(passkeysSupported()).toBe(false);
    Object.defineProperty(navigator, 'credentials', { configurable: true, value: { create: jest.fn(), get: jest.fn() } });
    expect(passkeysSupported()).toBe(true);
  });

  it('tells a cancelled prompt and an already registered device from real errors', () => {
    expect(passkeyCancelled(new DOMException('x', 'NotAllowedError'))).toBe(true);
    expect(passkeyCancelled(new DOMException('x', 'AbortError'))).toBe(true);
    expect(passkeyCancelled(new DOMException('x', 'SecurityError'))).toBe(false);
    expect(passkeyCancelled(new Error('NotAllowedError'))).toBe(false);
    expect(passkeyAlreadyThere(new DOMException('x', 'InvalidStateError'))).toBe(true);
    expect(passkeyAlreadyThere(new DOMException('x', 'NotAllowedError'))).toBe(false);
  });
});
