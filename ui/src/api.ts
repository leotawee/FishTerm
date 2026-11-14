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
    /** Free-text list section; empty = ungrouped. */
    group: string;
    forwards: PortForward[];
    auth: Auth;
    has_secret: boolean;
}

/** Local forward (`ssh -L`), active while a session is open. */
export interface PortForward {
    local_port: number;
    remote_host: string;
    remote_port: number;
}

export interface HostInput {
    id?: string | null;
    name: string;
    host: string;
    port: number;
    username: string;
    transport: Transport;
    os: Os;
    group?: string;
    forwards?: PortForward[];
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
    /** A forward channel could not be opened on the server. */
    | {type: 'forward-error'; local_port: number}
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
/** Open a terminal link in the default browser (Rust allows http/https/mailto only). */
export const urlOpen = (url: string) => invoke<void>('url_open', {url});

export interface SftpEntry {
    name: string;
    path: string;
    is_dir: boolean;
    size: number;
    modified: number | null;
}

export interface SftpProgress {
    bytes: number;
    total: number;
}

export interface SftpListing {
    /** Server-canonical directory; always use this, never a client guess. */
    cwd: string;
    entries: SftpEntry[];
}

/** `path: ''` lists the remote home directory. */
export const sftpLs = (id: string, path: string) => invoke<SftpListing>('sftp_ls', {id, path});
export const sftpDownload = (id: string, remotePath: string, localPath: string, onProgress: (p: SftpProgress) => void) => {
    const progress = new Channel<SftpProgress>(onProgress);
    return invoke<number>('sftp_download', {id, remotePath, localPath, progress});
};
export const sftpUpload = (id: string, localPath: string, remotePath: string, onProgress: (p: SftpProgress) => void) => {
    const progress = new Channel<SftpProgress>(onProgress);
    return invoke<number>('sftp_upload', {id, localPath, remotePath, progress});
};
/** Download to an app temp copy and open it in macOS Quick Look. */
export const sftpPreview = (id: string, remotePath: string, onProgress: (p: SftpProgress) => void) => {
    const progress = new Channel<SftpProgress>(onProgress);
    return invoke<void>('sftp_preview', {id, remotePath, progress});
};
