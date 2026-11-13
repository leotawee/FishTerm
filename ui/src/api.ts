import {Channel, invoke} from '@tauri-apps/api/core';

// Mirrors `Host` in src-tauri/src/store.rs. Secrets are never part of this type.
export type Transport = 'direct' | 'cloudflare_access';
export type Auth = {kind: 'password'} | {kind: 'key'; path: string};
// Mirrors `Os` in src-tauri/src/store.rs.
export type Os =
    | 'generic'
    | 'debian'
    | 'ubuntu'
    | 'raspberry_pi_os'
    | 'redhat'
    | 'centos'
    | 'fedora'
    | 'opensuse'
    | 'suse'
    | 'arch_linux'
    | 'amazon'
    | 'freebsd'
    | 'apple'
    | 'windows'
    | 'linux';

export interface HostView {
    id: string;
    name: string;
    host: string;
    port: number;
    username: string;
    transport: Transport;
    os: Os;
    auth: Auth;
    has_secret: boolean;
}

export interface HostInput {
    id?: string | null;
    name: string;
    host: string;
    port: number;
    username: string;
    transport: Transport;
    os: Os;
    auth: Auth;
}

/** Shape of every rejected command (src-tauri/src/error.rs). */
export interface AppError {
    code: string;
    message: string;
}

export function errorOf(e: unknown): AppError {
    if (e && typeof e === 'object' && 'code' in e && 'message' in e) return e as AppError;
    return {code: 'unknown', message: String(e)};
}

export const hostsList = () => invoke<HostView[]>('hosts_list');
export const hostSave = (input: HostInput) => invoke<HostView>('host_save', {input});
export const hostDelete = (id: string) => invoke<void>('host_delete', {id});
// Write-only: there is deliberately no command that reads a secret back.
export const hostSetSecret = (id: string, secret: string) =>
    invoke<void>('host_set_secret', {id, secret});
export const hostClearSecret = (id: string) => invoke<void>('host_clear_secret', {id});
export const startupWarnings = () => invoke<string[]>('startup_warnings');

/** Footer monitor values; `null` renders as `--` (non-Linux host or first sample). */
export interface SessionStats {
    cpu: number | null;
    mem: number | null;
    disk: number | null;
    rx_kbps: number | null;
    tx_kbps: number | null;
}
export const sessionStats = (id: string) => invoke<SessionStats>('session_stats', {id});

export interface HostKeyPrompt {
    host: string;
    port: number;
    algorithm: string;
    fingerprint: string;
}

export type SessionEvent =
    | {type: 'hostkey'; prompt_id: string; prompt: HostKeyPrompt}
    /** Cloudflare Access progress before the session id exists. */
    | {type: 'cf-login'; status: 'probe' | 'cache' | 'browser' | 'waiting' | 'token' | 'connecting'; attempt: number}
    | {type: 'closed'; error: AppError | null};

/** Raw `ArrayBuffer` messages are terminal output; objects are control events. */
export function sessionOpen(
    hostId: string,
    cols: number,
    rows: number,
    onEvent: (msg: ArrayBuffer | SessionEvent) => void,
) {
    const events = new Channel<ArrayBuffer | SessionEvent>(onEvent);
    return invoke<string>('session_open', {hostId, cols, rows, events});
}
export const sessionWrite = (id: string, data: string) => invoke<void>('session_write', {id, data});
export const sessionResize = (id: string, cols: number, rows: number) =>
    invoke<void>('session_resize', {id, cols, rows});
export const sessionClose = (id: string) => invoke<void>('session_close', {id});
export const hostkeyDecide = (promptId: string, accept: boolean) =>
    invoke<void>('hostkey_decide', {promptId, accept});
export const quitApp = () => invoke<void>('quit_app');
