import { isTauri } from '@tauri-apps/api/core';

export type RuntimePlatform = 'web' | string;

const tauri = isTauri();
const internal = typeof window === 'undefined' ? undefined : window.__PGPAD_INTERNAL__;
const platform = tauri ? (internal?.platform ?? 'unknown') : 'web';

export const runtime = {
	isTauri: tauri,
	platform,
	isMacOS: tauri && platform === 'macos',
	showWindowControls: tauri && platform !== 'macos'
} as const;
