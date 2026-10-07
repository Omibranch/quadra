// Flags drawn the way the map is: a 6x6 block of squares. Each entry describes the pattern;
// letters are palette keys. Forms:
//   h:abc   horizontal stripes (2 or 3)      v:abc   vertical stripes (2 or 3)
//   +:ab    Nordic cross (field a, cross b)  o:ab    field a with a 2x2 centre of b
//   six rows of six letters separated by "/" for everything else
export const palette = {
  r: '#e0484d', w: 'var(--flag-light)', b: '#3d7fe6', n: '#27408f', k: 'var(--flag-dark)', y: '#f0c648',
  g: '#2fa565', o: '#ee8a3c', c: '#62bdf0', m: '#96303a',
};

const spec = {
  DE: 'h:kry', NL: 'h:rwb', RU: 'h:wbr', AT: 'h:rwr', HU: 'h:rwg', BG: 'h:wgr', EE: 'h:bkw', LT: 'h:ygr',
  LU: 'h:rwc', AM: 'h:rbo', PL: 'h:wr', UA: 'h:by', ID: 'h:rw', MC: 'h:rw', EG: 'h:rwk', YE: 'h:rwk',
  IN: 'h:owg', UZ: 'h:cwg', AZ: 'h:crg', TJ: 'h:rwg', RS: 'h:rbw', SK: 'h:wbr', SI: 'h:wbr', HR: 'h:rwb',
  LV: 'h:mwm', LI: 'h:nr', SG: 'h:rw', AR: 'h:cwc', IR: 'h:gwr', BO: 'h:ryg', GA: 'h:gyb',
  FR: 'v:bwr', IT: 'v:gwr', IE: 'v:gwo', BE: 'v:kyr', RO: 'v:byr', MD: 'v:byr', MX: 'v:gwr', NG: 'v:gwg',
  MT: 'v:wr', PE: 'v:rwr', AD: 'v:byr',
  SE: '+:by', FI: '+:wb', DK: '+:rw', NO: '+:rn', IS: '+:br',
  JP: 'o:wr', CH: 'rrrrrr/rrwwrr/rwwwwr/rwwwwr/rrwwrr/rrrrrr', VN: 'o:ry', HK: 'o:rw', KG: 'o:ry', KZ: 'o:cy',
  AL: 'o:rk', MA: 'o:rg', TR: 'rrrrrr/rwwrrr/rwrrwr/rwrrwr/rwwrrr/rrrrrr', CY: 'o:wo', BD: 'o:gr',
  GB: 'wnrrnw/nnrrnn/rrrrrr/rrrrrr/nnrrnn/wnrrnw',
  US: 'nnnrrr/nnnwww/nnnrrr/wwwwww/rrrrrr/wwwwww',
  CA: 'rwwwwr/rwwwwr/rwrrwr/rwrrwr/rwwwwr/rwwwwr',
  KR: 'wwwwww/wkwwkw/wwrrww/wwbbww/wkwwkw/wwwwww',
  AE: 'rrgggg/rrgggg/rrwwww/rrwwww/rrkkkk/rrkkkk',
  IL: 'wwwwww/bbbbbb/wwbbww/wwbbww/bbbbbb/wwwwww',
  AU: 'rwnnnn/wrnnwn/nnnnnn/nnnwnn/nwnnnw/nnnnnn',
  NZ: 'rwnnnn/wrnnrn/nnnnnn/nnnrnn/nnnnnr/nnnnnn',
  BR: 'gggggg/ggyygg/gybbyg/gybbyg/ggyygg/gggggg',
  CZ: 'bwwwww/bbwwww/bbbwww/bbbrrr/bbrrrr/brrrrr',
  PT: 'ggrrrr/ggrrrr/gyyrrr/gyyrrr/ggrrrr/ggrrrr',
  ES: 'rrrrrr/yyyyyy/yyyyyy/yyyyyy/yyyyyy/rrrrrr',
  GR: 'bwbwww/wwwbbb/bwbwww/bbbbbb/wwwwww/bbbbbb',
  GE: 'wwrrww/wwrrww/rrrrrr/rrrrrr/wwrrww/wwrrww',
  CN: 'ryrrrr/yrrrrr/rrrrrr/rrrrrr/rrrrrr/rrrrrr',
  TW: 'nnnrrr/nwnrrr/nnnrrr/rrrrrr/rrrrrr/rrrrrr',
  TH: 'rrrrrr/wwwwww/nnnnnn/nnnnnn/wwwwww/rrrrrr',
  CL: 'nnwwww/nwwwww/nnwwww/rrrrrr/rrrrrr/rrrrrr',
  CO: 'yyyyyy/yyyyyy/yyyyyy/nnnnnn/rrrrrr/rrrrrr',
  BY: 'wrrrrr/rrrrrr/wrrrrr/rrrrrr/wggggg/rggggg',
  ZA: 'grrrrr/ygwwww/kygggg/kygggg/ygwwww/gbbbbb',
  SA: 'gggggg/gwwwwg/gggggg/gggggg/ggwwgg/gggggg',
  SO: 'cccccc/cccccc/ccwwcc/ccwwcc/cccccc/cccccc',
  EU: 'nnyynn/nynnyn/ynnnny/ynnnny/nynnyn/nnyynn',
};
spec.UK = spec.GB;

function expand(s) {
  const [kind, arg] = s.includes(':') ? s.split(':') : ['x', s];
  if (kind === 'x') return arg.split('/');
  if (kind === 'h') {
    const rows = arg.length === 2 ? [0, 0, 0, 1, 1, 1] : [0, 0, 1, 1, 2, 2];
    return rows.map((i) => arg[i].repeat(6));
  }
  if (kind === 'v') {
    const cols = arg.length === 2 ? [0, 0, 0, 1, 1, 1] : [0, 0, 1, 1, 2, 2];
    return Array(6).fill(cols.map((i) => arg[i]).join(''));
  }
  const [a, b] = arg;
  if (kind === '+') return [0, 1, 2, 3, 4, 5].map((r) => (r === 2 || r === 3 ? b.repeat(6) : a + b + b + a + a + a));
  return [0, 1, 2, 3, 4, 5].map((r) => (r === 2 || r === 3 ? a + a + b + b + a + a : a.repeat(6)));
}

const cache = new Map();

/** Six rows of six palette letters for a country code, or null when there is no drawing. */
export function flagRows(cc) {
  const code = (cc || '').toUpperCase();
  if (!spec[code]) return null;
  if (!cache.has(code)) cache.set(code, expand(spec[code]));
  return cache.get(code);
}

const regionNames = (() => {
  try {
    return new Intl.DisplayNames(['ru'], { type: 'region' });
  } catch {
    return null;
  }
})();

export function countryName(cc) {
  const code = (cc || '').toUpperCase();
  if (!code) return '';
  if (code === 'EU') return 'Европа';
  try {
    return regionNames?.of(code) ?? code;
  } catch {
    return code;
  }
}
