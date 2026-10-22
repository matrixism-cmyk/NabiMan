/**
 * Putting Hangul together for keyboards that leave it to the page.
 *
 * The iPhone's (and iPad's) Korean keyboard builds a syllable by replacing
 * the letter before the cursor: ㅇ, then ㅏ turns it into 아. A terminal's
 * input field never keeps what was typed — every key goes straight to the
 * shell — so the keyboard finds nothing to replace and the letters arrive one
 * by one: "ㅇㅣㄹㅓㅎㄱㅔ" for 이렇게. This does the keyboard's part: each
 * letter that joins the syllable just sent goes out as Backspace and the new
 * syllable, and Backspace takes back the last letter, not the whole syllable.
 */

const CHO = 'ㄱㄲㄴㄷㄸㄹㅁㅂㅃㅅㅆㅇㅈㅉㅊㅋㅌㅍㅎ';
const JUNG = 'ㅏㅐㅑㅒㅓㅔㅕㅖㅗㅘㅙㅚㅛㅜㅝㅞㅟㅠㅡㅢㅣ';
/** Final consonants, after "none" at 0. */
const JONG = ['', ...Array.from('ㄱㄲㄳㄴㄵㄶㄷㄹㄺㄻㄼㄽㄾㄿㅀㅁㅂㅄㅅㅆㅇㅈㅊㅋㅌㅍㅎ')];

const VOWEL_PAIRS: Record<string, string> = {
  'ㅗㅏ': 'ㅘ', 'ㅗㅐ': 'ㅙ', 'ㅗㅣ': 'ㅚ', 'ㅜㅓ': 'ㅝ', 'ㅜㅔ': 'ㅞ', 'ㅜㅣ': 'ㅟ', 'ㅡㅣ': 'ㅢ',
};
const FINAL_PAIRS: Record<string, string> = {
  'ㄱㅅ': 'ㄳ', 'ㄴㅈ': 'ㄵ', 'ㄴㅎ': 'ㄶ', 'ㄹㄱ': 'ㄺ', 'ㄹㅁ': 'ㄻ', 'ㄹㅂ': 'ㄼ',
  'ㄹㅅ': 'ㄽ', 'ㄹㅌ': 'ㄾ', 'ㄹㅍ': 'ㄿ', 'ㄹㅎ': 'ㅀ', 'ㅂㅅ': 'ㅄ',
};
/** A compound final, split back into the two it was made of. */
const FINAL_SPLIT: Record<string, [string, string]> = Object.fromEntries(
  Object.entries(FINAL_PAIRS).map(([pair, final]) => [final, [pair[0], pair[1]]]),
);

const BACKSPACE = '\x7f';

/** One syllable in the making: an initial consonant, a vowel, a final. */
interface Block { cho: string; jung: string; jong: string }

const EMPTY: Block = { cho: '', jung: '', jong: '' };

const isConsonant = (c: string) => c !== '' && (CHO.includes(c) || JONG.includes(c));
const isVowel = (c: string) => JUNG.includes(c);

/** What the block looks like on screen. */
function render(b: Block): string {
  if (b.cho && b.jung) {
    return String.fromCharCode(0xac00 + (CHO.indexOf(b.cho) * 21 + JUNG.indexOf(b.jung)) * 28 + JONG.indexOf(b.jong));
  }
  return b.cho || b.jung;
}

/**
 * Feeds what the terminal would send through; returns what to send instead.
 * Anything but a single Hangul letter (or Backspace while a syllable is being
 * made) passes untouched and ends the syllable.
 */
export class HangulComposer {
  /** The steps of the syllable on screen, so Backspace can go back one. */
  private steps: Block[] = [];

  private get block(): Block {
    return this.steps[this.steps.length - 1] || EMPTY;
  }

  feed(data: string): string {
    if (data === BACKSPACE && this.steps.length) {
      this.steps.pop();
      return this.steps.length ? BACKSPACE + render(this.block) : BACKSPACE;
    }
    if (data.length !== 1 || !(isConsonant(data) || isVowel(data))) {
      this.steps = [];
      return data;
    }
    return isVowel(data) ? this.vowel(data) : this.consonant(data);
  }

  /** Replaces the syllable on screen with `next`. */
  private replace(next: Block, after = ''): string {
    this.steps.push(next);
    return BACKSPACE + render(next) + after;
  }

  /** Starts a new syllable with `next`. */
  private begin(next: Block, before = ''): string {
    this.steps = [next];
    return before + render(next);
  }

  private vowel(v: string): string {
    const b = this.block;
    if (b.cho && !b.jung) return this.replace({ ...b, jung: v });
    if (b.jung && !b.jong && VOWEL_PAIRS[b.jung + v]) return this.replace({ ...b, jung: VOWEL_PAIRS[b.jung + v] });
    if (b.cho && b.jung && b.jong) {
      // The final moves on to start the next syllable: 갑 + ㅏ → 가 바.
      const split = FINAL_SPLIT[b.jong];
      const stays: Block = { ...b, jong: split ? split[0] : '' };
      const moves = split ? split[1] : b.jong;
      if (CHO.includes(moves)) {
        const next: Block = { cho: moves, jung: v, jong: '' };
        this.steps = [{ cho: moves, jung: '', jong: '' }, next];
        return BACKSPACE + render(stays) + render(next);
      }
    }
    return this.begin({ cho: '', jung: v, jong: '' });
  }

  private consonant(c: string): string {
    const b = this.block;
    if (b.cho && b.jung && !b.jong && JONG.includes(c)) return this.replace({ ...b, jong: c });
    if (b.cho && b.jung && b.jong && FINAL_PAIRS[b.jong + c]) return this.replace({ ...b, jong: FINAL_PAIRS[b.jong + c] });
    return CHO.includes(c) ? this.begin({ cho: c, jung: '', jong: '' }) : this.begin({ cho: '', jung: '', jong: '' }, c);
  }
}

/** iPhone and iPad (an iPad says it is a Mac, but has a touch screen). */
export function needsHangulComposer(nav: Pick<Navigator, 'userAgent' | 'maxTouchPoints'> = navigator): boolean {
  return /iP(hone|ad|od)/.test(nav.userAgent) || (/Macintosh/.test(nav.userAgent) && nav.maxTouchPoints > 1);
}
