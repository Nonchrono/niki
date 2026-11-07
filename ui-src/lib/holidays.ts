import { useEffect, useState } from 'react';

export interface Holiday {
  date: string;
  localName: string;
  name: string;
  countryCode?: string;
}

export type DayColorType = 'holiday' | 'sunday' | 'saturday' | 'weekday';

// 대한민국 매년 고정 양력 공휴일 (월-일 기준)
const FIXED_ANNUAL_HOLIDAYS: Record<string, { localName: string; name: string }> = {
  '01-01': { localName: '새해', name: "New Year's Day" },
  '03-01': { localName: '3·1절', name: 'Independence Movement Day' },
  '05-05': { localName: '어린이날', name: "Children's Day" },
  '06-06': { localName: '현충일', name: 'Memorial Day' },
  '08-15': { localName: '광복절', name: 'Liberation Day' },
  '10-03': { localName: '개천절', name: 'National Foundation Day' },
  '10-09': { localName: '한글날', name: 'Hangul Day' },
  '12-25': { localName: '크리스마스', name: 'Christmas Day' },
};

// 2024~2030년 대한민국 공휴일 오프라인 기본 데이터 (Nager.Date 공휴일 규정 반영)
const FALLBACK_HOLIDAYS: Record<string, Holiday> = {
  // 2024
  '2024-01-01': { date: '2024-01-01', localName: '새해', name: "New Year's Day" },
  '2024-02-09': { date: '2024-02-09', localName: '설날', name: 'Lunar New Year' },
  '2024-02-10': { date: '2024-02-10', localName: '설날', name: 'Lunar New Year' },
  '2024-02-12': { date: '2024-02-12', localName: '설날', name: 'Lunar New Year' },
  '2024-03-01': { date: '2024-03-01', localName: '3·1절', name: 'Independence Movement Day' },
  '2024-04-10': { date: '2024-04-10', localName: '국회의원선거', name: 'National Assembly Election' },
  '2024-05-05': { date: '2024-05-05', localName: '어린이날', name: "Children's Day" },
  '2024-05-06': { date: '2024-05-06', localName: '어린이날', name: "Children's Day" },
  '2024-05-15': { date: '2024-05-15', localName: '부처님 오신 날', name: "Buddha's Birthday" },
  '2024-06-06': { date: '2024-06-06', localName: '현충일', name: 'Memorial Day' },
  '2024-08-15': { date: '2024-08-15', localName: '광복절', name: 'Liberation Day' },
  '2024-09-16': { date: '2024-09-16', localName: '추석', name: 'Chuseok' },
  '2024-09-17': { date: '2024-09-17', localName: '추석', name: 'Chuseok' },
  '2024-09-18': { date: '2024-09-18', localName: '추석', name: 'Chuseok' },
  '2024-10-03': { date: '2024-10-03', localName: '개천절', name: 'National Foundation Day' },
  '2024-10-09': { date: '2024-10-09', localName: '한글날', name: 'Hangul Day' },
  '2024-12-25': { date: '2024-12-25', localName: '크리스마스', name: 'Christmas Day' },

  // 2025
  '2025-01-01': { date: '2025-01-01', localName: '새해', name: "New Year's Day" },
  '2025-01-28': { date: '2025-01-28', localName: '설날', name: 'Lunar New Year' },
  '2025-01-29': { date: '2025-01-29', localName: '설날', name: 'Lunar New Year' },
  '2025-01-30': { date: '2025-01-30', localName: '설날', name: 'Lunar New Year' },
  '2025-03-01': { date: '2025-03-01', localName: '3·1절', name: 'Independence Movement Day' },
  '2025-03-03': { date: '2025-03-03', localName: '3·1절', name: 'Independence Movement Day' },
  '2025-05-05': { date: '2025-05-05', localName: '어린이날', name: "Children's Day" },
  '2025-05-06': { date: '2025-05-06', localName: '부처님 오신 날', name: "Buddha's Birthday" },
  '2025-06-06': { date: '2025-06-06', localName: '현충일', name: 'Memorial Day' },
  '2025-08-15': { date: '2025-08-15', localName: '광복절', name: 'Liberation Day' },
  '2025-10-03': { date: '2025-10-03', localName: '개천절', name: 'National Foundation Day' },
  '2025-10-05': { date: '2025-10-05', localName: '추석', name: 'Chuseok' },
  '2025-10-06': { date: '2025-10-06', localName: '추석', name: 'Chuseok' },
  '2025-10-07': { date: '2025-10-07', localName: '추석', name: 'Chuseok' },
  '2025-10-08': { date: '2025-10-08', localName: '추석', name: 'Chuseok' },
  '2025-10-09': { date: '2025-10-09', localName: '한글날', name: 'Hangul Day' },
  '2025-12-25': { date: '2025-12-25', localName: '크리스마스', name: 'Christmas Day' },

  // 2026
  '2026-01-01': { date: '2026-01-01', localName: '새해', name: "New Year's Day" },
  '2026-02-16': { date: '2026-02-16', localName: '설날', name: 'Lunar New Year' },
  '2026-02-17': { date: '2026-02-17', localName: '설날', name: 'Lunar New Year' },
  '2026-02-18': { date: '2026-02-18', localName: '설날', name: 'Lunar New Year' },
  '2026-03-01': { date: '2026-03-01', localName: '3·1절', name: 'Independence Movement Day' },
  '2026-03-02': { date: '2026-03-02', localName: '3·1절', name: 'Independence Movement Day' },
  '2026-05-05': { date: '2026-05-05', localName: '어린이날', name: "Children's Day" },
  '2026-05-25': { date: '2026-05-25', localName: '부처님 오신 날', name: "Buddha's Birthday" },
  '2026-06-03': { date: '2026-06-03', localName: '지방 선거일', name: 'Local Election Day' },
  '2026-06-06': { date: '2026-06-06', localName: '현충일', name: 'Memorial Day' },
  '2026-08-15': { date: '2026-08-15', localName: '광복절', name: 'Liberation Day' },
  '2026-08-17': { date: '2026-08-17', localName: '광복절', name: 'Liberation Day' },
  '2026-09-24': { date: '2026-09-24', localName: '추석', name: 'Chuseok' },
  '2026-09-25': { date: '2026-09-25', localName: '추석', name: 'Chuseok' },
  '2026-09-26': { date: '2026-09-26', localName: '추석', name: 'Chuseok' },
  '2026-10-03': { date: '2026-10-03', localName: '개천절', name: 'National Foundation Day' },
  '2026-10-05': { date: '2026-10-05', localName: '개천절', name: 'National Foundation Day' },
  '2026-10-09': { date: '2026-10-09', localName: '한글날', name: 'Hangul Day' },
  '2026-12-25': { date: '2026-12-25', localName: '크리스마스', name: 'Christmas Day' },

  // 2027
  '2027-01-01': { date: '2027-01-01', localName: '새해', name: "New Year's Day" },
  '2027-02-06': { date: '2027-02-06', localName: '설날', name: 'Lunar New Year' },
  '2027-02-07': { date: '2027-02-07', localName: '설날', name: 'Lunar New Year' },
  '2027-02-08': { date: '2027-02-08', localName: '설날', name: 'Lunar New Year' },
  '2027-02-09': { date: '2027-02-09', localName: '설날', name: 'Lunar New Year' },
  '2027-03-01': { date: '2027-03-01', localName: '3·1절', name: 'Independence Movement Day' },
  '2027-05-05': { date: '2027-05-05', localName: '어린이날', name: "Children's Day" },
  '2027-05-13': { date: '2027-05-13', localName: '부처님 오신 날', name: "Buddha's Birthday" },
  '2027-06-06': { date: '2027-06-06', localName: '현충일', name: 'Memorial Day' },
  '2027-08-15': { date: '2027-08-15', localName: '광복절', name: 'Liberation Day' },
  '2027-08-16': { date: '2027-08-16', localName: '광복절', name: 'Liberation Day' },
  '2027-09-14': { date: '2027-09-14', localName: '추석', name: 'Chuseok' },
  '2027-09-15': { date: '2027-09-15', localName: '추석', name: 'Chuseok' },
  '2027-09-16': { date: '2027-09-16', localName: '추석', name: 'Chuseok' },
  '2027-10-03': { date: '2027-10-03', localName: '개천절', name: 'National Foundation Day' },
  '2027-10-04': { date: '2027-10-04', localName: '개천절', name: 'National Foundation Day' },
  '2027-10-09': { date: '2027-10-09', localName: '한글날', name: 'Hangul Day' },
  '2027-10-11': { date: '2027-10-11', localName: '한글날', name: 'Hangul Day' },
  '2027-12-25': { date: '2027-12-25', localName: '크리스마스', name: 'Christmas Day' },

  // 2028
  '2028-01-01': { date: '2028-01-01', localName: '새해', name: "New Year's Day" },
  '2028-01-26': { date: '2028-01-26', localName: '설날', name: 'Lunar New Year' },
  '2028-01-27': { date: '2028-01-27', localName: '설날', name: 'Lunar New Year' },
  '2028-01-28': { date: '2028-01-28', localName: '설날', name: 'Lunar New Year' },
  '2028-03-01': { date: '2028-03-01', localName: '3·1절', name: 'Independence Movement Day' },
  '2028-05-02': { date: '2028-05-02', localName: '부처님 오신 날', name: "Buddha's Birthday" },
  '2028-05-05': { date: '2028-05-05', localName: '어린이날', name: "Children's Day" },
  '2028-06-06': { date: '2028-06-06', localName: '현충일', name: 'Memorial Day' },
  '2028-08-15': { date: '2028-08-15', localName: '광복절', name: 'Liberation Day' },
  '2028-10-02': { date: '2028-10-02', localName: '추석', name: 'Chuseok' },
  '2028-10-03': { date: '2028-10-03', localName: '개천절', name: 'National Foundation Day' },
  '2028-10-04': { date: '2028-10-04', localName: '추석', name: 'Chuseok' },
  '2028-10-05': { date: '2028-10-05', localName: '추석', name: 'Chuseok' },
  '2028-10-09': { date: '2028-10-09', localName: '한글날', name: 'Hangul Day' },
  '2028-12-25': { date: '2028-12-25', localName: '크리스마스', name: 'Christmas Day' },

  // 2029
  '2029-01-01': { date: '2029-01-01', localName: '새해', name: "New Year's Day" },
  '2029-02-12': { date: '2029-02-12', localName: '설날', name: 'Lunar New Year' },
  '2029-02-13': { date: '2029-02-13', localName: '설날', name: 'Lunar New Year' },
  '2029-02-14': { date: '2029-02-14', localName: '설날', name: 'Lunar New Year' },
  '2029-03-01': { date: '2029-03-01', localName: '3·1절', name: 'Independence Movement Day' },
  '2029-05-05': { date: '2029-05-05', localName: '어린이날', name: "Children's Day" },
  '2029-05-07': { date: '2029-05-07', localName: '어린이날', name: "Children's Day" },
  '2029-05-20': { date: '2029-05-20', localName: '부처님 오신 날', name: "Buddha's Birthday" },
  '2029-06-06': { date: '2029-06-06', localName: '현충일', name: 'Memorial Day' },
  '2029-08-15': { date: '2029-08-15', localName: '광복절', name: 'Liberation Day' },
  '2029-09-21': { date: '2029-09-21', localName: '추석', name: 'Chuseok' },
  '2029-09-22': { date: '2029-09-22', localName: '추석', name: 'Chuseok' },
  '2029-09-23': { date: '2029-09-23', localName: '추석', name: 'Chuseok' },
  '2029-09-24': { date: '2029-09-24', localName: '추석', name: 'Chuseok' },
  '2029-10-03': { date: '2029-10-03', localName: '개천절', name: 'National Foundation Day' },
  '2029-10-09': { date: '2029-10-09', localName: '한글날', name: 'Hangul Day' },
  '2029-12-25': { date: '2029-12-25', localName: '크리스마스', name: 'Christmas Day' },

  // 2030
  '2030-01-01': { date: '2030-01-01', localName: '새해', name: "New Year's Day" },
  '2030-02-02': { date: '2030-02-02', localName: '설날', name: 'Lunar New Year' },
  '2030-02-03': { date: '2030-02-03', localName: '설날', name: 'Lunar New Year' },
  '2030-02-04': { date: '2030-02-04', localName: '설날', name: 'Lunar New Year' },
  '2030-03-01': { date: '2030-03-01', localName: '3·1절', name: 'Independence Movement Day' },
  '2030-05-05': { date: '2030-05-05', localName: '어린이날', name: "Children's Day" },
  '2030-05-06': { date: '2030-05-06', localName: '어린이날', name: "Children's Day" },
  '2030-05-09': { date: '2030-05-09', localName: '부처님 오신 날', name: "Buddha's Birthday" },
  '2030-06-06': { date: '2030-06-06', localName: '현충일', name: 'Memorial Day' },
  '2030-08-15': { date: '2030-08-15', localName: '광복절', name: 'Liberation Day' },
  '2030-09-11': { date: '2030-09-11', localName: '추석', name: 'Chuseok' },
  '2030-09-12': { date: '2030-09-12', localName: '추석', name: 'Chuseok' },
  '2030-09-13': { date: '2030-09-13', localName: '추석', name: 'Chuseok' },
  '2030-10-03': { date: '2030-10-03', localName: '개천절', name: 'National Foundation Day' },
  '2030-10-09': { date: '2030-10-09', localName: '한글날', name: 'Hangul Day' },
  '2030-12-25': { date: '2030-12-25', localName: '크리스마스', name: 'Christmas Day' },
};

// 메모리 캐시
const memoryCache = new Map<number, Record<string, Holiday>>();

// 초기 기본 공휴일 맵 생성 (오프라인 fallback + 고정 공휴일)
export function getInitialHolidays(year: number): Record<string, Holiday> {
  const result: Record<string, Holiday> = {};

  // 1) 매년 고정 공휴일 기본 채우기
  for (const [monthDay, info] of Object.entries(FIXED_ANNUAL_HOLIDAYS)) {
    const dateKey = `${year}-${monthDay}`;
    result[dateKey] = { date: dateKey, localName: info.localName, name: info.name };
  }

  // 2) 내장 FALLBACK 데이터에서 해당 연도 채우기
  const yearPrefix = `${year}-`;
  for (const [key, holiday] of Object.entries(FALLBACK_HOLIDAYS)) {
    if (key.startsWith(yearPrefix)) {
      result[key] = holiday;
    }
  }

  // 3) localStorage 캐시가 있으면 병합
  try {
    if (typeof window !== 'undefined' && window.localStorage) {
      const stored = localStorage.getItem(`niki_holidays_${year}`);
      if (stored) {
        const parsed = JSON.parse(stored) as Record<string, Holiday>;
        Object.assign(result, parsed);
      }
    }
  } catch {
    // ignore storage error
  }

  return result;
}

// 무료 공휴일 조회 API (Nager.Date API)를 통해 해당 연도의 공휴일 로드
export async function fetchHolidays(year: number): Promise<Record<string, Holiday>> {
  if (memoryCache.has(year)) {
    return memoryCache.get(year)!;
  }

  const initial = getInitialHolidays(year);
  memoryCache.set(year, initial);

  try {
    const controller = new AbortController();
    const timeoutId = setTimeout(() => controller.abort(), 4000);
    const response = await fetch(`https://date.nager.at/api/v3/PublicHolidays/${year}/KR`, {
      signal: controller.signal,
    });
    clearTimeout(timeoutId);

    if (response.ok) {
      const data = (await response.json()) as Holiday[];
      const updated: Record<string, Holiday> = { ...initial };
      for (const item of data) {
        if (item.date && item.localName) {
          // 노동절 등 일반 법정 공휴일 외 항목 정리 또는 유지 (기본 유지)
          updated[item.date] = {
            date: item.date,
            localName: item.localName,
            name: item.name,
            countryCode: item.countryCode,
          };
        }
      }
      memoryCache.set(year, updated);

      try {
        if (typeof window !== 'undefined' && window.localStorage) {
          localStorage.setItem(`niki_holidays_${year}`, JSON.stringify(updated));
        }
      } catch {
        // ignore
      }

      return updated;
    }
  } catch {
    // 네트워크 실패나 타임아웃 시 기본/캐시 데이터 유지
  }

  return initial;
}

// 특정 날짜 문자열(YYYY-MM-DD)의 공휴일 정보 조회
export function getHoliday(dateStr: string, holidays?: Record<string, Holiday>): Holiday | undefined {
  if (holidays && holidays[dateStr]) {
    return holidays[dateStr];
  }

  const year = parseInt(dateStr.slice(0, 4), 10);
  const cached = memoryCache.get(year);
  if (cached && cached[dateStr]) {
    return cached[dateStr];
  }

  if (FALLBACK_HOLIDAYS[dateStr]) {
    return FALLBACK_HOLIDAYS[dateStr];
  }

  const monthDay = dateStr.slice(5);
  if (FIXED_ANNUAL_HOLIDAYS[monthDay]) {
    const fixed = FIXED_ANNUAL_HOLIDAYS[monthDay];
    return { date: dateStr, localName: fixed.localName, name: fixed.name };
  }

  return undefined;
}

/**
 * 달력 날짜 색상 분류 함수:
 * - 공휴일: 'holiday' (빨간색)
 * - 토요일과 공휴일이 겹치는 경우: 공휴일이 우선되어 'holiday' (빨간색)
 * - 일요일: 'sunday' (빨간색)
 * - 토요일: 'saturday' (파란색)
 * - 평일: 'weekday' (기본 색상)
 */
export function getDayColorType(date: Date, holiday?: Holiday): DayColorType {
  // 공휴일인 경우: 토요일과 겹치더라도 무조건 빨간색('holiday')으로 표시
  if (holiday) {
    return 'holiday';
  }
  const day = date.getDay();
  if (day === 0) {
    return 'sunday'; // 일요일: 빨간색
  }
  if (day === 6) {
    return 'saturday'; // 토요일: 파란색
  }
  return 'weekday';
}

/**
 * React 훅: 특정 연도의 공휴일 맵을 반환하고 백그라운드에서 API를 통해 최신 공휴일을 조회
 */
export function useHolidays(year: number): Record<string, Holiday> {
  const [holidays, setHolidays] = useState<Record<string, Holiday>>(() => getInitialHolidays(year));

  useEffect(() => {
    let cancelled = false;
    setHolidays(getInitialHolidays(year));

    fetchHolidays(year).then((data) => {
      if (!cancelled) {
        setHolidays({ ...data });
      }
    });

    return () => {
      cancelled = true;
    };
  }, [year]);

  return holidays;
}

