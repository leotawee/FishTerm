import type {Os} from './api';

// Font Awesome fallback for OS values without a bundled logo (raspberry_pi_os, opensuse, amazon, generic).
const FA_ICONS: Record<Os, string> = {
    generic: 'fas fa-terminal',
    debian: 'fab fa-debian',
    ubuntu: 'fab fa-ubuntu',
    raspberry_pi_os: 'fab fa-raspberry-pi',
    redhat: 'fab fa-redhat',
    centos: 'fab fa-centos',
    fedora: 'fab fa-fedora',
    opensuse: 'fab fa-opensuse',
    suse: 'fab fa-suse',
    arch_linux: 'fab fa-arch-linux',
    amazon: 'fab fa-amazon',
    freebsd: 'fab fa-freebsd',
    apple: 'fab fa-apple',
    windows: 'fab fa-windows',
    linux: 'fab fa-linux',
};

// Files are named `<Os>.png`; logos extracted from UTM.app (see ui/assets/os/).
const images = import.meta.glob('../assets/os/*.png', {
    eager: true,
    query: '?url',
    import: 'default',
}) as Record<string, string>;

export function osIcon(os: Os): string {
    return images[`../assets/os/${os}.png`] ?? FA_ICONS[os];
}

// Vite inlines small assets as data: URIs, otherwise serves them as absolute paths.
export const isImageIcon = (icon?: string) =>
    !!icon && (icon.startsWith('/') || icon.startsWith('data:'));
