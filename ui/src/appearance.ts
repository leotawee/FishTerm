// Appearance preferences: UI theme + terminal color scheme.
// Persisted in the webview's localStorage; no Rust involvement.

export type ThemeMode = 'system' | 'light' | 'dark' | 'catppuccin_mocha' | 'catppuccin_latte';

// xterm ITheme subset: base pair + optional ANSI-16 (Catppuccin ships full palettes
// from the official alacritty schemes, fetched from the catppuccin GitHub repo).
export type TermTheme = {
    background: string;
    foreground: string;
    cursor?: string;
    selectionBackground?: string;
    black?: string;
    red?: string;
    green?: string;
    yellow?: string;
    blue?: string;
    magenta?: string;
    cyan?: string;
    white?: string;
    brightBlack?: string;
    brightRed?: string;
    brightGreen?: string;
    brightYellow?: string;
    brightBlue?: string;
    brightMagenta?: string;
    brightCyan?: string;
    brightWhite?: string;
};

export const TERMINAL_THEMES: Record<string, TermTheme> = {
    slate: {background: '#0f172a', foreground: '#ffffff'},
    black: {background: '#000000', foreground: '#e6e6e6'},
    solarized: {background: '#002b36', foreground: '#839496'},
    dracula: {background: '#282a36', foreground: '#f8f8f2'},
    paper: {background: '#fdf6e3', foreground: '#3b4351'},
    catppuccin_mocha: {
        background: '#1e1e2e',
        foreground: '#cdd6f4',
        cursor: '#f5e0dc',
        selectionBackground: '#585b70',
        black: '#45475a',
        red: '#f38ba8',
        green: '#a6e3a1',
        yellow: '#f9e2af',
        blue: '#89b4fa',
        magenta: '#f5c2e7',
        cyan: '#94e2d5',
        white: '#bac2de',
        brightBlack: '#585b70',
        brightRed: '#f38ba8',
        brightGreen: '#a6e3a1',
        brightYellow: '#f9e2af',
        brightBlue: '#89b4fa',
        brightMagenta: '#f5c2e7',
        brightCyan: '#94e2d5',
        brightWhite: '#a6adc8',
    },
    catppuccin_latte: {
        background: '#eff1f5',
        foreground: '#4c4f69',
        cursor: '#dc8a78',
        selectionBackground: '#d9d3e9',
        black: '#bcc0cc',
        red: '#d20f39',
        green: '#40a02b',
        yellow: '#df8e1d',
        blue: '#1e66f5',
        magenta: '#ea76cb',
        cyan: '#179299',
        white: '#5c5f77',
        brightBlack: '#acb0be',
        brightRed: '#d20f39',
        brightGreen: '#40a02b',
        brightYellow: '#df8e1d',
        brightBlue: '#1e66f5',
        brightMagenta: '#ea76cb',
        brightCyan: '#179299',
        brightWhite: '#6c6f85',
    },
};

// UI palettes (semantic tokens mirrored in style.css :root/.dark defaults).
const PALETTES: Record<Exclude<ThemeMode, 'system'>, Record<string, string>> = {
    light: {
        '--c-bg': '#ffffff',
        '--c-card': '#ffffff',
        '--c-text': '#0f172a',
        '--c-muted': '#64748b',
        '--c-accent': '#4f46e5',
        '--c-accent-hover': '#4338ca',
        '--c-accent-text': '#ffffff',
        '--c-border': '#d1d5db',
        '--c-ghost-hover': '#f1f5f9',
    },
    dark: {
        '--c-bg': '#121212',
        '--c-card': '#2c2c2c',
        '--c-text': '#e5e7eb',
        '--c-muted': '#94a3b8',
        '--c-accent': '#6366f1',
        '--c-accent-hover': '#818cf8',
        '--c-accent-text': '#ffffff',
        '--c-border': '#475569',
        '--c-ghost-hover': '#3f3f46',
    },
    catppuccin_mocha: {
        '--c-bg': '#1e1e2e',
        '--c-card': '#313244',
        '--c-text': '#cdd6f4',
        '--c-muted': '#a5adc8',
        '--c-accent': '#cba6f7',
        '--c-accent-hover': '#b4befe',
        '--c-accent-text': '#1e1e2e',
        '--c-border': '#585b70',
        '--c-ghost-hover': '#45475a',
    },
    catppuccin_latte: {
        '--c-bg': '#eff1f5',
        '--c-card': '#ffffff',
        '--c-text': '#4c4f69',
        '--c-muted': '#6c6f85',
        '--c-accent': '#8839ef',
        '--c-accent-hover': '#7c3aed',
        '--c-accent-text': '#ffffff',
        '--c-border': '#ccd0da',
        '--c-ghost-hover': '#e6e9ef',
    },
};

const TOKEN_KEYS = Object.keys(PALETTES.light);
const media = matchMedia('(prefers-color-scheme: dark)');

export function getThemeMode(): ThemeMode {
    const t = localStorage.getItem('appearance.theme');
    return (['light', 'dark', 'catppuccin_mocha', 'catppuccin_latte'] as const).includes(t as never)
        ? (t as ThemeMode)
        : 'system';
}

export function isDarkTheme(t: ThemeMode, systemDark = media.matches): boolean {
    return t === 'dark' || t === 'catppuccin_mocha' || (t === 'system' && systemDark);
}

export function getTermThemeName(): string {
    return localStorage.getItem('appearance.term') || 'slate';
}

export function getTermTheme(): TermTheme {
    // Concrete literal fallback: noUncheckedIndexedAccess makes lookups T|undefined.
    return TERMINAL_THEMES[getTermThemeName()] ?? {background: '#0f172a', foreground: '#ffffff'};
}

function applyMode() {
    const t = getThemeMode();
    document.documentElement.classList.toggle('dark', isDarkTheme(t));
    const root = document.documentElement;
    for (const key of TOKEN_KEYS) root.style.removeProperty(key);
    if (t !== 'system') {
        for (const [key, value] of Object.entries(PALETTES[t as Exclude<ThemeMode, 'system'>])) {
            root.style.setProperty(key, value);
        }
    }
}

media.addEventListener('change', () => {
    if (getThemeMode() === 'system') applyMode();
});
applyMode();

export function setThemeMode(t: ThemeMode) {
    localStorage.setItem('appearance.theme', t);
    applyMode();
}

export function setTermTheme(name: string) {
    localStorage.setItem('appearance.term', name);
    // Open terminal tabs listen for this and re-theme live.
    dispatchEvent(new Event('appearance-changed'));
}
