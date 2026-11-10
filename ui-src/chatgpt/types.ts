export interface ChatGPTAccount { client_id: string; email: string; connected: boolean; plan_enabled: boolean; welcome_seen: boolean }
export interface ChatGPTStatus { accounts: ChatGPTAccount[]; active: string | null; model: string; pending_registration: boolean }
export interface DiarySummary { text: string; model: string; input_tokens: number | null; output_tokens: number | null }
export const activeAccount = (status: ChatGPTStatus | null) => status?.accounts.find(account => account.client_id === status.active);
