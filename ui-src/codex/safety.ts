// Words that suggest the user may be thinking about hurting themselves. Matching happens in the
// app, not in the model, so a safety card with real contacts is shown even if the model fails,
// is slow, or answers badly. A false alarm costs one extra card; a miss costs far more.
const patterns: RegExp[] = [
  /자살/, /자해/, /죽고\s*싶/, /죽을\s*것\s*같/, /죽어\s*버리/, /목숨을?\s*끊/,
  /(삶|생|인생)을?\s*(끝내|마감|포기)/, /사라지고\s*싶/, /없어지고\s*싶/, /(더\s*이상|이제)\s*살\s*(고\s*싶|기\s*싫|수\s*없|자신)/,
  /살기\s*싫/, /태어나지\s*말/,
  /suicid/i, /kill\s+myself/i, /end\s+my\s+life/i, /self[-\s]?harm/i, /want\s+to\s+die/i,
];

export const detectCrisis = (text: string) => patterns.some(pattern => pattern.test(text));

export const crisisContacts = [
  { name: '자살예방 상담전화', number: '109', note: '24시간' },
  { name: '정신건강 위기상담', number: '1577-0199', note: '24시간' },
  { name: '청소년 상담전화', number: '1388', note: '24시간' },
  { name: '응급상황', number: '119', note: '지금 위험하다면' },
] as const;
