/**
 * What a serial board is, from the only things Web Serial tells a page about
 * it: the USB vendor and product ids. (The COM port name is never exposed;
 * the browser shows it only in its own port picker.)
 */

/** Common USB-serial chips and boards, by `VVVV:PPPP` (hex), or by vendor alone. */
const KNOWN: Record<string, string> = {
  '303A:1001': 'Espressif USB-JTAG/Serial (ESP32-C3/S3/C6/H2)',
  '303A:0002': 'Espressif ESP32-S2 USB CDC',
  '303A:4001': 'Espressif ESP32-S2/S3 USB CDC (TinyUSB)',
  '303A': 'Espressif',
  '10C4:EA60': 'Silicon Labs CP210x',
  '10C4:EA70': 'Silicon Labs CP2105',
  '10C4': 'Silicon Labs',
  '1A86:7523': 'WCH CH340',
  '1A86:55D4': 'WCH CH9102',
  '1A86:55D3': 'WCH CH343',
  '1A86': 'WCH (QinHeng)',
  '0403:6001': 'FTDI FT232R',
  '0403:6010': 'FTDI FT2232',
  '0403:6014': 'FTDI FT232H',
  '0403:6015': 'FTDI FT231X',
  '0403': 'FTDI',
  '067B:2303': 'Prolific PL2303',
  '2341': 'Arduino',
  '2A03': 'Arduino',
  '2E8A:0005': 'Raspberry Pi Pico (MicroPython)',
  '2E8A:000A': 'Raspberry Pi Pico (SDK CDC)',
  '2E8A': 'Raspberry Pi',
  '239A': 'Adafruit',
  '0483:5740': 'STMicroelectronics Virtual COM',
  '0483:374B': 'ST-LINK/V2-1 VCP',
  '0483': 'STMicroelectronics',
  '1366': 'SEGGER J-Link',
  '0D28:0204': 'Arm DAPLink (micro:bit)',
};

const hex4 = (n: number) => n.toString(16).toUpperCase().padStart(4, '0');

/** `device` is the bridge's port key, `vendor:product` in decimal; empty parts mean unknown. */
export function usbInfo(device: string): { id: string; name: string | null } | null {
  const m = /^(\d+):(\d+)$/.exec(device || '');
  if (!m) return null;
  const vid = hex4(Number(m[1]));
  const pid = hex4(Number(m[2]));
  return { id: `${vid}:${pid}`, name: KNOWN[`${vid}:${pid}`] ?? KNOWN[vid] ?? null };
}

/** One line for people: `USB 303A:1001 · Espressif USB-JTAG/Serial (…)`. */
export function usbLabel(device: string): string {
  const info = usbInfo(device);
  if (!info) return '';
  return info.name ? `USB ${info.id} · ${info.name}` : `USB ${info.id}`;
}
