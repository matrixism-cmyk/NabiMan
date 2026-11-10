type Lang = 'ko' | 'en' | 'ja';
type T = Record<string, Record<Lang, string>>;

// What a file is, told from its first bytes in the SFTP preview.
const t: T = {
  'sniff.elf': { ko: 'ELF 실행 파일 / 라이브러리', en: 'ELF executable / library', ja: 'ELF 実行ファイル / ライブラリ' },
  'sniff.exe': { ko: 'Windows 실행 파일 (EXE/DLL)', en: 'Windows executable (EXE/DLL)', ja: 'Windows 実行ファイル (EXE/DLL)' },
  'sniff.macho': { ko: 'macOS 실행 파일 (Mach-O)', en: 'macOS executable (Mach-O)', ja: 'macOS 実行ファイル (Mach-O)' },
  'sniff.gzip': { ko: 'gzip 압축', en: 'gzip archive', ja: 'gzip 圧縮' },
  'sniff.bzip2': { ko: 'bzip2 압축', en: 'bzip2 archive', ja: 'bzip2 圧縮' },
  'sniff.xz': { ko: 'xz 압축', en: 'xz archive', ja: 'xz 圧縮' },
  'sniff.zstd': { ko: 'zstd 압축', en: 'zstd archive', ja: 'zstd 圧縮' },
  'sniff.7z': { ko: '7-Zip 압축', en: '7-Zip archive', ja: '7-Zip 圧縮' },
  'sniff.rar': { ko: 'RAR 압축', en: 'RAR archive', ja: 'RAR 圧縮' },
  'sniff.zip': { ko: 'ZIP 압축', en: 'ZIP archive', ja: 'ZIP 圧縮' },
  'sniff.pdf': { ko: 'PDF 문서', en: 'PDF document', ja: 'PDF 文書' },
  'sniff.png': { ko: 'PNG 그림', en: 'PNG image', ja: 'PNG 画像' },
  'sniff.jpeg': { ko: 'JPEG 그림', en: 'JPEG image', ja: 'JPEG 画像' },
  'sniff.gif': { ko: 'GIF 그림', en: 'GIF image', ja: 'GIF 画像' },
  'sniff.sqlite': { ko: 'SQLite 데이터베이스', en: 'SQLite database', ja: 'SQLite データベース' },
  'sniff.ole': { ko: '옛 Office·한글 문서 (OLE: doc/xls/ppt/hwp)', en: 'Legacy Office / Hangul document (OLE: doc/xls/ppt/hwp)', ja: '旧 Office・ハングル文書 (OLE: doc/xls/ppt/hwp)' },
  'sniff.wasm': { ko: 'WebAssembly', en: 'WebAssembly', ja: 'WebAssembly' },
  'sniff.javaClass': { ko: 'Java 클래스 / macOS 유니버설 바이너리', en: 'Java class / macOS universal binary', ja: 'Java クラス / macOS ユニバーサルバイナリ' },
  'sniff.ogg': { ko: 'Ogg 미디어', en: 'Ogg media', ja: 'Ogg メディア' },
  'sniff.matroska': { ko: 'Matroska/WebM 미디어', en: 'Matroska/WebM media', ja: 'Matroska/WebM メディア' },
  'sniff.mp3': { ko: 'MP3 음성', en: 'MP3 audio', ja: 'MP3 音声' },
  'sniff.flac': { ko: 'FLAC 음성', en: 'FLAC audio', ja: 'FLAC 音声' },
  'sniff.pem': { ko: 'PEM (인증서·키)', en: 'PEM (certificate / key)', ja: 'PEM (証明書・鍵)' },
  'sniff.tar': { ko: 'tar 묶음', en: 'tar archive', ja: 'tar アーカイブ' },
  'sniff.mp4': { ko: 'MP4/MOV/HEIC 미디어', en: 'MP4/MOV/HEIC media', ja: 'MP4/MOV/HEIC メディア' },
  'sniff.riff': { ko: 'RIFF 미디어 (WAV/AVI/WebP)', en: 'RIFF media (WAV/AVI/WebP)', ja: 'RIFF メディア (WAV/AVI/WebP)' },
};

export default t;
