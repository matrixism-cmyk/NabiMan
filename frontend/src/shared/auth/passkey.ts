// Passkeys (WebAuthn) in the browser: turning the server's JSON into what
// `navigator.credentials` takes, and its answer back into JSON. Binary
// fields travel as base64url without padding, as the WebAuthn JSON forms do.

/** Whether this browser can use passkeys at all. */
export function passkeysSupported(): boolean {
  return typeof window !== 'undefined'
    && typeof window.PublicKeyCredential === 'function'
    && typeof navigator.credentials?.create === 'function'
    && typeof navigator.credentials?.get === 'function';
}

export function toBase64Url(data: ArrayBuffer | Uint8Array): string {
  const bytes = data instanceof Uint8Array ? data : new Uint8Array(data);
  let binary = '';
  for (let i = 0; i < bytes.length; i += 1) binary += String.fromCharCode(bytes[i]);
  return window.btoa(binary).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
}

/** Padded or not, url-safe or the standard alphabet. Throws on anything else. */
export function fromBase64Url(text: string): Uint8Array {
  if (!/^[A-Za-z0-9\-_+/]*={0,2}$/.test(text)) throw new Error('not base64url');
  const plain = text.replace(/=+$/, '').replace(/-/g, '+').replace(/_/g, '/');
  if (plain.length % 4 === 1) throw new Error('not base64url');
  const binary = window.atob(plain + '='.repeat((4 - (plain.length % 4)) % 4));
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i += 1) bytes[i] = binary.charCodeAt(i);
  return bytes;
}

interface DescriptorJSON { type: string; id: string; transports?: string[]; }
/** `publicKey` of a registration, as the server sends it. */
export interface CreationOptionsJSON {
  challenge: string;
  user: { id: string; name: string; displayName: string };
  excludeCredentials?: DescriptorJSON[];
  [other: string]: unknown;
}
/** `publicKey` of a sign-in, as the server sends it. */
export interface RequestOptionsJSON {
  challenge: string;
  allowCredentials?: DescriptorJSON[];
  [other: string]: unknown;
}

const descriptors = (list?: DescriptorJSON[]) => (list || []).map((d) => ({
  ...d, id: fromBase64Url(d.id),
})) as PublicKeyCredentialDescriptor[];

export function creationOptions(json: CreationOptionsJSON): PublicKeyCredentialCreationOptions {
  return {
    ...json,
    challenge: fromBase64Url(json.challenge),
    user: { ...json.user, id: fromBase64Url(json.user.id) },
    excludeCredentials: descriptors(json.excludeCredentials),
  } as unknown as PublicKeyCredentialCreationOptions;
}

export function requestOptions(json: RequestOptionsJSON): PublicKeyCredentialRequestOptions {
  return {
    ...json,
    challenge: fromBase64Url(json.challenge),
    allowCredentials: descriptors(json.allowCredentials),
  } as unknown as PublicKeyCredentialRequestOptions;
}

/** A new passkey, as posted to `/api/auth/passkeys/register/finish`. */
export function registrationJSON(credential: PublicKeyCredential) {
  const response = credential.response as AuthenticatorAttestationResponse;
  return {
    id: credential.id,
    rawId: toBase64Url(credential.rawId),
    type: credential.type,
    response: {
      attestationObject: toBase64Url(response.attestationObject),
      clientDataJSON: toBase64Url(response.clientDataJSON),
    },
  };
}

/** A sign-in answer, as posted to `/api/auth/passkeys/login/finish`. */
export function assertionJSON(credential: PublicKeyCredential) {
  const response = credential.response as AuthenticatorAssertionResponse;
  return {
    id: credential.id,
    rawId: toBase64Url(credential.rawId),
    type: credential.type,
    response: {
      authenticatorData: toBase64Url(response.authenticatorData),
      clientDataJSON: toBase64Url(response.clientDataJSON),
      signature: toBase64Url(response.signature),
      userHandle: response.userHandle ? toBase64Url(response.userHandle) : null,
    },
  };
}

/** Asks the device to make a passkey. Rejects when the person cancels. */
export async function createPasskey(json: CreationOptionsJSON) {
  const made = await navigator.credentials.create({ publicKey: creationOptions(json) });
  if (!made) throw new DOMException('No passkey was made', 'NotAllowedError');
  return registrationJSON(made as PublicKeyCredential);
}

/** Asks the device for one of this site's passkeys. Rejects when the person cancels. */
export async function getPasskey(json: RequestOptionsJSON) {
  const got = await navigator.credentials.get({ publicKey: requestOptions(json) });
  if (!got) throw new DOMException('No passkey was chosen', 'NotAllowedError');
  return assertionJSON(got as PublicKeyCredential);
}

/** The person closed the prompt, or it timed out: not an error to report as one. */
export function passkeyCancelled(cause: unknown): boolean {
  return cause instanceof DOMException && (cause.name === 'NotAllowedError' || cause.name === 'AbortError');
}

/** The device already holds a passkey of this account (`excludeCredentials`). */
export function passkeyAlreadyThere(cause: unknown): boolean {
  return cause instanceof DOMException && cause.name === 'InvalidStateError';
}
