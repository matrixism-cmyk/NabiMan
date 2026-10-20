/**
 * What the file panel can show in its preview, by file name. Each kind has
 * its own renderer in PreviewDialog; adding a format is adding a row here
 * and, for a new kind, a renderer.
 */
export type PreviewKind = 'image' | 'text' | 'pdf' | 'video' | 'audio' | 'zip' | 'file';

/** Kinds read a range at a time through a stream link instead of whole. */
export const STREAMED: ReadonlySet<PreviewKind> = new Set<PreviewKind>(['video', 'audio', 'zip', 'file']);

export interface PreviewSpec {
  kind: PreviewKind;
  /** The type the bytes are labelled with, so the browser decodes them right. */
  mime: string;
  /** Larger files are not fetched for a preview; they can still be downloaded. */
  maxBytes: number;
  /** For text: the highlight.js language, or `plaintext`. */
  language?: string;
}

const MB = 1024 * 1024;

const IMAGES: Record<string, string> = {
  png: 'image/png', apng: 'image/apng',
  jpg: 'image/jpeg', jpeg: 'image/jpeg', jpe: 'image/jpeg', jfif: 'image/jpeg',
  gif: 'image/gif', webp: 'image/webp', avif: 'image/avif',
  bmp: 'image/bmp', ico: 'image/x-icon', cur: 'image/x-icon',
  // Shown through <img>, where an SVG's scripts never run.
  svg: 'image/svg+xml',
};

const VIDEO = new Set(['mp4', 'm4v', 'webm', 'ogv', 'mov', 'mkv']);
const AUDIO = new Set(['mp3', 'wav', 'flac', 'm4a', 'm4b', 'aac', 'ogg', 'oga', 'opus', 'weba']);
/** ZIP inside: archives, Office and OpenDocument files, Java, Android, e-books, wheels. */
const ZIP = new Set(['zip', 'docx', 'xlsx', 'pptx', 'odt', 'ods', 'odp', 'jar', 'war', 'ear', 'apk', 'aab', 'epub', 'whl', 'xpi', 'nupkg', 'vsix', 'ipa']);

export const extensionOf = (name: string) => {
  const dot = name.lastIndexOf('.');
  return dot > 0 ? name.slice(dot + 1).toLowerCase() : '';
};

/** Text and code by extension, with the language to colour it as. */
const CODE: Record<string, string> = {
  js: 'javascript', mjs: 'javascript', cjs: 'javascript', jsx: 'javascript',
  ts: 'typescript', mts: 'typescript', cts: 'typescript', tsx: 'typescript',
  py: 'python', pyw: 'python', rs: 'rust', go: 'go', java: 'java', kt: 'kotlin', kts: 'kotlin',
  c: 'c', h: 'c', cc: 'cpp', cpp: 'cpp', cxx: 'cpp', hpp: 'cpp', hh: 'cpp', cs: 'csharp',
  php: 'php', rb: 'ruby', swift: 'swift', scala: 'scala', groovy: 'groovy', gradle: 'groovy',
  lua: 'lua', pl: 'perl', pm: 'perl', r: 'r', dart: 'dart',
  sh: 'bash', bash: 'bash', zsh: 'bash', ksh: 'bash', env: 'bash', ps1: 'powershell', psm1: 'powershell',
  sql: 'sql', json: 'json', jsonc: 'json', json5: 'json', ndjson: 'json', ipynb: 'json',
  yaml: 'yaml', yml: 'yaml', toml: 'ini', ini: 'ini', cfg: 'ini', conf: 'ini', cnf: 'ini',
  properties: 'properties', service: 'ini', timer: 'ini', socket: 'ini', desktop: 'ini',
  xml: 'xml', xsd: 'xml', xsl: 'xml', html: 'xml', htm: 'xml', xhtml: 'xml', vue: 'xml', plist: 'xml',
  css: 'css', scss: 'scss', less: 'less', md: 'markdown', markdown: 'markdown',
  diff: 'diff', patch: 'diff', dockerfile: 'dockerfile', mk: 'makefile',
  txt: 'plaintext', text: 'plaintext', log: 'plaintext', csv: 'plaintext', tsv: 'plaintext',
  out: 'plaintext', err: 'plaintext', pid: 'plaintext', lock: 'plaintext', rst: 'plaintext',
  tex: 'plaintext', srt: 'plaintext', vtt: 'plaintext', pem: 'plaintext', crt: 'plaintext', pub: 'plaintext',
};

/** Whole names (lower case) that are text without telling extension. */
const NAMES: Record<string, string> = {
  dockerfile: 'dockerfile', containerfile: 'dockerfile', makefile: 'makefile', gnumakefile: 'makefile',
  jenkinsfile: 'groovy', vagrantfile: 'ruby', gemfile: 'ruby', rakefile: 'ruby', procfile: 'plaintext',
  readme: 'plaintext', license: 'plaintext', licence: 'plaintext', changelog: 'plaintext', authors: 'plaintext',
  notice: 'plaintext', todo: 'plaintext', copying: 'plaintext', version: 'plaintext', crontab: 'bash',
  '.bashrc': 'bash', '.bash_profile': 'bash', '.bash_logout': 'bash', '.profile': 'bash', '.zshrc': 'bash',
  '.zprofile': 'bash', '.env': 'bash', '.gitignore': 'plaintext', '.dockerignore': 'plaintext',
  '.gitattributes': 'plaintext', '.npmrc': 'ini', '.editorconfig': 'ini', '.gitconfig': 'ini',
  '.vimrc': 'plaintext', '.tmux.conf': 'bash', 'authorized_keys': 'plaintext', 'known_hosts': 'plaintext',
  hosts: 'plaintext', fstab: 'plaintext', passwd: 'plaintext', group: 'plaintext', sudoers: 'plaintext',
};

/** Config files whose own syntax highlight.js knows. */
function configLanguage(lower: string): string | null {
  if (lower.endsWith('.conf') && lower.includes('nginx')) return 'nginx';
  if (/^(httpd|apache2?)\.conf$|\.htaccess$/.test(lower)) return 'apache';
  return null;
}

export function previewOf(name: string): PreviewSpec | null {
  const ext = extensionOf(name);
  if (IMAGES[ext]) return { kind: 'image', mime: IMAGES[ext], maxBytes: 50 * MB };
  if (ext === 'pdf') return { kind: 'pdf', mime: 'application/pdf', maxBytes: 100 * MB };
  if (VIDEO.has(ext)) return { kind: 'video', mime: '', maxBytes: Infinity };
  if (AUDIO.has(ext)) return { kind: 'audio', mime: '', maxBytes: Infinity };
  if (ZIP.has(ext)) return { kind: 'zip', mime: '', maxBytes: Infinity };
  const lower = name.toLowerCase();
  // `.env.local`, `Dockerfile.dev`: the leading name still says what it is.
  const stem = lower.startsWith('.') ? lower : lower.split('.')[0];
  const language = configLanguage(lower) || NAMES[lower] || CODE[ext] || NAMES[stem]
    || (lower.startsWith('.env') ? 'bash' : null);
  if (language) return { kind: 'text', mime: 'text/plain', maxBytes: 5 * MB, language };
  // Anything else: what it is, and a look at its first bytes.
  return { kind: 'file', mime: '', maxBytes: Infinity };
}
