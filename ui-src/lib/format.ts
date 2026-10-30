export const localDate = () => {
  const now = new Date();
  return new Date(now.getTime() - now.getTimezoneOffset() * 60_000).toISOString().slice(0, 10);
};
export const dateLabel = (date: string) => new Date(`${date}T12:00:00`).toLocaleDateString('ko-KR', { year: 'numeric', month: 'long', day: 'numeric' });
export const timeLabel = (date?: string | null) => date ? new Date(date).toLocaleString('ko-KR') : '아직 동기화하지 않음';
export const accountLabel = (status: { connected: boolean; account_name?: string | null; account_email?: string | null }) =>
  !status.connected ? '연결되지 않음' : status.account_name && status.account_email ? `${status.account_name} (${status.account_email})` : status.account_email || status.account_name || 'Google 계정 연결됨';
export const fileBase64 = (file: File) => new Promise<string>((resolve, reject) => {
  const reader = new FileReader();
  reader.onload = () => resolve(String(reader.result).split(',')[1]);
  reader.onerror = () => reject(reader.error);
  reader.readAsDataURL(file);
});
export const errorMessage = (error: unknown) => error instanceof Error ? error.message : String(error);
