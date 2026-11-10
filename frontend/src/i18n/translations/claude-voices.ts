type Lang = 'ko' | 'en' | 'ja';
type T = Record<string, Record<Lang, string>>;

// What each of the speech service's voices sounds like, by the voice id the server sends.
const t: T = {
  'claude.speaker.sohee': { ko: '따뜻하고 감정이 풍부한 한국어 여성', en: 'Warm, expressive Korean female', ja: '温かく感情豊かな韓国語の女性' },
  'claude.speaker.vivian': { ko: '밝고 또렷한 젊은 여성 (중국어)', en: 'Bright, clear young female (Chinese)', ja: '明るくはっきりした若い女性（中国語）' },
  'claude.speaker.serena': { ko: '부드럽고 차분한 젊은 여성 (중국어)', en: 'Soft, calm young female (Chinese)', ja: '柔らかく落ち着いた若い女性（中国語）' },
  'claude.speaker.ono_anna': { ko: '경쾌한 젊은 여성 (일본어)', en: 'Lively young female (Japanese)', ja: '軽快な若い女性（日本語）' },
  'claude.speaker.aiden': { ko: '밝은 미국 남성 (영어)', en: 'Bright American male (English)', ja: '明るいアメリカ人男性（英語）' },
  'claude.speaker.ryan': { ko: '리듬감 있는 남성 (영어)', en: 'Rhythmic male (English)', ja: 'リズム感のある男性（英語）' },
  'claude.speaker.uncle_fu': { ko: '낮고 묵직한 중년 남성 (중국어)', en: 'Deep, weighty middle-aged male (Chinese)', ja: '低く重厚な中年男性（中国語）' },
  'claude.speaker.dylan': { ko: '베이징 젊은 남성 (중국어)', en: 'Young Beijing male (Chinese)', ja: '北京の若い男性（中国語）' },
  'claude.speaker.eric': { ko: '청두 남성, 살짝 허스키 (중국어)', en: 'Chengdu male, slightly husky (Chinese)', ja: '成都の男性、少しハスキー（中国語）' },
};

export default t;
