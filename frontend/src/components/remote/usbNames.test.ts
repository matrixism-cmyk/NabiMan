import { usbInfo, usbLabel } from './usbNames';

it('names the board from what Web Serial reports (decimal ids → hex, known chips)', () => {
  expect(usbInfo(`${0x303a}:${0x1001}`)).toEqual({ id: '303A:1001', name: 'Espressif USB-JTAG/Serial (ESP32-C3/S3/C6/H2)' });
  expect(usbLabel(`${0x10c4}:${0xea60}`)).toBe('USB 10C4:EA60 · Silicon Labs CP210x');
  // A product not in the table still gets its vendor.
  expect(usbLabel(`${0x303a}:${0x9999}`)).toBe('USB 303A:9999 · Espressif');
  expect(usbLabel(`${0x1234}:${0x5678}`)).toBe('USB 1234:5678');
  // Not a USB port (Bluetooth), or nothing known.
  expect(usbLabel(':')).toBe('');
  expect(usbInfo('')).toBeNull();
});
