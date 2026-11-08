import {invoke} from '@tauri-apps/api/core';

// Mirrors `HostView` in src-tauri/src/store.rs. Secrets are never part of this type.
export type Transport = 'direct' | 'cloudflare_access';
export type Auth = {kind: 'password'} | {kind: 'key'; path: string};

export interface HostView {
    id: string;
    name: string;
    host: string;
    port: number;
    username: string;
    transport: Transport;
    auth: Auth;
    has_secret: boolean;
}

export interface HostInput {
    id?: string;
    name: string;
    host: string;
    port: number;
    username: string;
    transport: Transport;
    auth: Auth;
}

export const hostsList = () => invoke<HostView[]>('hosts_list');
export const hostSave = (input: HostInput) => invoke<HostView>('host_save', {input});
export const hostDelete = (id: string) => invoke<void>('host_delete', {id});
// Write-only: there is deliberately no command that reads a secret back.
export const hostSetSecret = (id: string, secret: string) =>
    invoke<void>('host_set_secret', {id, secret});
export const hostClearSecret = (id: string) => invoke<void>('host_clear_secret', {id});
