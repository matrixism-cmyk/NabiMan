/** The server's rule for new passwords (backend/src/password_policy.rs): 12+ characters, at most 72 bytes. */
export const PASSWORD_MIN = 12;
export const PASSWORD_MAX_BYTES = 72;

/** Whether a new password meets the rule; checked again by the server. */
export function passwordOk(pw: string): boolean {
  return Array.from(pw).length >= PASSWORD_MIN && new TextEncoder().encode(pw).length <= PASSWORD_MAX_BYTES;
}
