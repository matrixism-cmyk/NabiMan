/** Standalone Jest, replacing the one react-scripts used to configure. */
module.exports = {
  testEnvironment: 'jsdom',
  // jsdom has no fetch; react-scripts used to polyfill it for us.
  setupFiles: ['whatwg-fetch'],
  setupFilesAfterEnv: ['<rootDir>/src/setupTests.ts'],
  testMatch: ['<rootDir>/src/**/*.{test,spec}.{js,jsx,ts,tsx}'],
  transform: { '^.+\\.(js|jsx|ts|tsx)$': 'babel-jest' },
  moduleNameMapper: {
    '\\.(css|less|sass|scss)$': 'identity-obj-proxy',
    '\\.(png|jpe?g|gif|svg|webp|avif|woff2?|ttf|eot)$': '<rootDir>/jest/fileMock.js',
  },
  // CRA reset mock implementations between tests; the suites rely on it.
  resetMocks: true,
};
