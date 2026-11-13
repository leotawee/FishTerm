// Appearance preferences: UI theme + terminal color scheme.
// Persisted in the webview's localStorage; no Rust involvement.

export type ThemeMode =
    | 'system'
    | 'light'
    | 'dark'
    | 'catppuccin_mocha'
    | 'catppuccin_latte'
    | 'dracula'
    | 'nord'
    | 'tokyo_night'
    | 'gruvbox_dark'
    | 'one_dark'
    | 'rose_pine';

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
    paper: {background: '#fdf6e3', foreground: '#3b4351'},
    solarized_dark: {
        background: '#002b36',
        foreground: '#839496',
        cursor: '#93a1a1',
        selectionBackground: '#073642',
        black: '#073642',
        red: '#dc322f',
        green: '#859900',
        yellow: '#b58900',
        blue: '#268bd2',
        magenta: '#d33682',
        cyan: '#2aa198',
        white: '#eee8d5',
        brightBlack: '#657b83',
        brightRed: '#dc322f',
        brightGreen: '#859900',
        brightYellow: '#b58900',
        brightBlue: '#268bd2',
        brightMagenta: '#d33682',
        brightCyan: '#2aa198',
        brightWhite: '#fdf6e3',
    },
    solarized_light: {
        background: '#fdf6e3',
        foreground: '#586e75',
        cursor: '#657b83',
        selectionBackground: '#eee8d5',
        black: '#073642',
        red: '#dc322f',
        green: '#859900',
        yellow: '#b58900',
        blue: '#268bd2',
        magenta: '#d33682',
        cyan: '#2aa198',
        white: '#eee8d5',
        brightBlack: '#93a1a1',
        brightRed: '#cb4b16',
        brightGreen: '#586e75',
        brightYellow: '#657b83',
        brightBlue: '#839496',
        brightMagenta: '#6c71c4',
        brightCyan: '#93a1a1',
        brightWhite: '#fdf6e3',
    },
    dracula: {
        background: '#282a36',
        foreground: '#f8f8f2',
        cursor: '#f8f8f2',
        selectionBackground: '#44475a',
        black: '#21222c',
        red: '#ff5555',
        green: '#50fa7b',
        yellow: '#f1fa8c',
        blue: '#bd93f9',
        magenta: '#ff79c6',
        cyan: '#8be9fd',
        white: '#f8f8f2',
        brightBlack: '#6272a4',
        brightRed: '#ff6e6e',
        brightGreen: '#69ff94',
        brightYellow: '#ffffa5',
        brightBlue: '#d6acff',
        brightMagenta: '#ff92df',
        brightCyan: '#a4ffff',
        brightWhite: '#ffffff',
    },
    nord: {
        background: '#2e3440',
        foreground: '#d8dee9',
        cursor: '#d8dee9',
        selectionBackground: '#434c5e',
        black: '#3b4252',
        red: '#bf616a',
        green: '#a3be8c',
        yellow: '#ebcb8b',
        blue: '#81a1c1',
        magenta: '#b48ead',
        cyan: '#88c0d0',
        white: '#e5e9f0',
        brightBlack: '#4c566a',
        brightRed: '#bf616a',
        brightGreen: '#a3be8c',
        brightYellow: '#ebcb8b',
        brightBlue: '#81a1c1',
        brightMagenta: '#b48ead',
        brightCyan: '#8fbcbb',
        brightWhite: '#eceff4',
    },
    one_dark: {
        background: '#282c34',
        foreground: '#abb2bf',
        cursor: '#528bff',
        selectionBackground: '#3e4451',
        black: '#282c34',
        red: '#e06c75',
        green: '#98c379',
        yellow: '#e5c07b',
        blue: '#61afef',
        magenta: '#c678dd',
        cyan: '#56b6c2',
        white: '#abb2bf',
        brightBlack: '#5c6370',
        brightRed: '#e06c75',
        brightGreen: '#98c379',
        brightYellow: '#d19a66',
        brightBlue: '#61afef',
        brightMagenta: '#c678dd',
        brightCyan: '#56b6c2',
        brightWhite: '#ffffff',
    },
    tokyo_night: {
        background: '#1a1b26',
        foreground: '#c0caf5',
        cursor: '#c0caf5',
        selectionBackground: '#33467c',
        black: '#15161e',
        red: '#f7768e',
        green: '#9ece6a',
        yellow: '#e0af68',
        blue: '#7aa2f7',
        magenta: '#bb9af7',
        cyan: '#7dcfff',
        white: '#a9b1d6',
        brightBlack: '#414868',
        brightRed: '#ff7a93',
        brightGreen: '#b9f27c',
        brightYellow: '#ff9e64',
        brightBlue: '#7da6ff',
        brightMagenta: '#bb9af7',
        brightCyan: '#0db9d7',
        brightWhite: '#c0caf5',
    },
    tokyo_night_light: {
        background: '#d5d6db',
        foreground: '#3760bf',
        cursor: '#3760bf',
        selectionBackground: '#b6bfe4',
        black: '#e9e9ec',
        red: '#f52a65',
        green: '#587539',
        yellow: '#8c6c3e',
        blue: '#2e7de9',
        magenta: '#9854bb',
        cyan: '#007197',
        white: '#6172b0',
        brightBlack: '#a1a6c5',
        brightRed: '#f52a65',
        brightGreen: '#587539',
        brightYellow: '#8c6c3e',
        brightBlue: '#2e7de9',
        brightMagenta: '#9854bb',
        brightCyan: '#007197',
        brightWhite: '#3760bf',
    },
    gruvbox_dark: {
        background: '#282828',
        foreground: '#ebdbb2',
        cursor: '#ebdbb2',
        selectionBackground: '#504945',
        black: '#282828',
        red: '#cc241d',
        green: '#98971a',
        yellow: '#d79921',
        blue: '#458588',
        magenta: '#b16286',
        cyan: '#689d6a',
        white: '#a89984',
        brightBlack: '#928374',
        brightRed: '#fb4934',
        brightGreen: '#b8bb26',
        brightYellow: '#fabd2f',
        brightBlue: '#83a598',
        brightMagenta: '#d3869b',
        brightCyan: '#8ec07c',
        brightWhite: '#ebdbb2',
    },
    gruvbox_light: {
        background: '#fbf1c7',
        foreground: '#3c3836',
        cursor: '#3c3836',
        selectionBackground: '#d5c4a1',
        black: '#fbf1c7',
        red: '#cc241d',
        green: '#98971a',
        yellow: '#d79921',
        blue: '#458588',
        magenta: '#b16286',
        cyan: '#689d6a',
        white: '#7c6f64',
        brightBlack: '#928374',
        brightRed: '#9d0006',
        brightGreen: '#79740e',
        brightYellow: '#b57614',
        brightBlue: '#076678',
        brightMagenta: '#8f3f71',
        brightCyan: '#427b58',
        brightWhite: '#3c3836',
    },
    rose_pine: {
        background: '#191724',
        foreground: '#e0def4',
        cursor: '#56526e',
        selectionBackground: '#403d52',
        black: '#26233a',
        red: '#eb6f92',
        green: '#31748f',
        yellow: '#f6c177',
        blue: '#9ccfd8',
        magenta: '#c4a7e7',
        cyan: '#ebbcba',
        white: '#e0def4',
        brightBlack: '#6e6a86',
        brightRed: '#eb6f92',
        brightGreen: '#31748f',
        brightYellow: '#f6c177',
        brightBlue: '#9ccfd8',
        brightMagenta: '#c4a7e7',
        brightCyan: '#ebbcba',
        brightWhite: '#e0def4',
    },
    everforest_dark: {
        background: '#2d353b',
        foreground: '#d3c6aa',
        cursor: '#d3c6aa',
        selectionBackground: '#475258',
        black: '#475258',
        red: '#e67e80',
        green: '#a7c080',
        yellow: '#dbbc7f',
        blue: '#7fbbb3',
        magenta: '#d699b6',
        cyan: '#83c092',
        white: '#d3c6aa',
        brightBlack: '#859289',
        brightRed: '#e67e80',
        brightGreen: '#a7c080',
        brightYellow: '#dbbc7f',
        brightBlue: '#7fbbb3',
        brightMagenta: '#d699b6',
        brightCyan: '#83c092',
        brightWhite: '#fdd7c3',
    },
    monokai_pro: {
        background: '#2d2a2e',
        foreground: '#fcfcfa',
        cursor: '#fcfcfa',
        selectionBackground: '#5b595c',
        black: '#403e41',
        red: '#ff6188',
        green: '#a9dc76',
        yellow: '#ffd866',
        blue: '#fc9867',
        magenta: '#ab9df2',
        cyan: '#78dce8',
        white: '#fcfcfa',
        brightBlack: '#727072',
        brightRed: '#ff6188',
        brightGreen: '#a9dc76',
        brightYellow: '#ffd866',
        brightBlue: '#fc9867',
        brightMagenta: '#ab9df2',
        brightCyan: '#78dce8',
        brightWhite: '#fcfcfa',
    },
    snazzy: {
        background: '#1e1f29',
        foreground: '#ffffff',
        cursor: '#ffffff',
        selectionBackground: '#3e4451',
        black: '#282a36',
        red: '#ff5c57',
        green: '#5af78e',
        yellow: '#f3f99d',
        blue: '#57c7ff',
        magenta: '#ff6ac1',
        cyan: '#9aedfe',
        white: '#f1f1f0',
        brightBlack: '#686a87',
        brightRed: '#ff5c57',
        brightGreen: '#5af78e',
        brightYellow: '#f3f99d',
        brightBlue: '#57c7ff',
        brightMagenta: '#ff6ac1',
        brightCyan: '#9aedfe',
        brightWhite: '#f1f1f0',
    },
    tomorrow_night: {
        background: '#1d1f21',
        foreground: '#c5c8c6',
        cursor: '#c5c8c6',
        selectionBackground: '#373b41',
        black: '#1d1f21',
        red: '#cc6666',
        green: '#b5bd68',
        yellow: '#f0c674',
        blue: '#81a2be',
        magenta: '#b294bb',
        cyan: '#8abeb7',
        white: '#c5c8c6',
        brightBlack: '#969896',
        brightRed: '#cc6666',
        brightGreen: '#b5bd68',
        brightYellow: '#f0c674',
        brightBlue: '#81a2be',
        brightMagenta: '#b294bb',
        brightCyan: '#8abeb7',
        brightWhite: '#ffffff',
    },
    night_owl: {
        background: '#011627',
        foreground: '#d6deeb',
        cursor: '#80a4c2',
        selectionBackground: '#1d3b53',
        black: '#011627',
        red: '#ef5350',
        green: '#22da6e',
        yellow: '#addb67',
        blue: '#82aaff',
        magenta: '#c792ea',
        cyan: '#21c7a8',
        white: '#d6deeb',
        brightBlack: '#5f7a97',
        brightRed: '#ef5350',
        brightGreen: '#22da6e',
        brightYellow: '#ffeb95',
        brightBlue: '#82aaff',
        brightMagenta: '#c792ea',
        brightCyan: '#7fdbca',
        brightWhite: '#ffffff',
    },
    github_dark: {
        background: '#0d1117',
        foreground: '#c9d1d9',
        cursor: '#c9d1d9',
        selectionBackground: '#163356',
        black: '#484f58',
        red: '#ff7b72',
        green: '#3fb950',
        yellow: '#d29922',
        blue: '#58a6ff',
        magenta: '#bc8cff',
        cyan: '#39c5cf',
        white: '#b1bac4',
        brightBlack: '#6e7681',
        brightRed: '#ff7b72',
        brightGreen: '#3fb950',
        brightYellow: '#d29922',
        brightBlue: '#58a6ff',
        brightMagenta: '#bc8cff',
        brightCyan: '#39c5cf',
        brightWhite: '#ffffff',
    },
    github_light: {
        background: '#ffffff',
        foreground: '#24292f',
        cursor: '#24292f',
        selectionBackground: '#b4d5fe',
        black: '#24292e',
        red: '#d73a49',
        green: '#28a745',
        yellow: '#dbab09',
        blue: '#0366d6',
        magenta: '#5a32a3',
        cyan: '#0598bc',
        white: '#6a737d',
        brightBlack: '#959da5',
        brightRed: '#cb2431',
        brightGreen: '#22863a',
        brightYellow: '#b08800',
        brightBlue: '#005cc5',
        brightMagenta: '#5a32a3',
        brightCyan: '#3192aa',
        brightWhite: '#d1d5da',
    },
    catppuccin_frappe: {
        background: '#303446',
        foreground: '#c6d0f5',
        cursor: '#f2d5cf',
        selectionBackground: '#626880',
        black: '#51576d',
        red: '#e78284',
        green: '#a6d189',
        yellow: '#e5c890',
        blue: '#8caaee',
        magenta: '#f4b8e4',
        cyan: '#81c8be',
        white: '#b5bfe2',
        brightBlack: '#626880',
        brightRed: '#e78284',
        brightGreen: '#a6d189',
        brightYellow: '#e5c890',
        brightBlue: '#8caaee',
        brightMagenta: '#f4b8e4',
        brightCyan: '#81c8be',
        brightWhite: '#a5adce',
    },
    catppuccin_macchiato: {
        background: '#24273a',
        foreground: '#cad3f5',
        cursor: '#f4dbd6',
        selectionBackground: '#5b6078',
        black: '#494d64',
        red: '#ed8796',
        green: '#a6da95',
        yellow: '#eed49f',
        blue: '#8aadf4',
        magenta: '#f5bde6',
        cyan: '#8bd5ca',
        white: '#b8c0e0',
        brightBlack: '#5b6078',
        brightRed: '#ed8796',
        brightGreen: '#a6da95',
        brightYellow: '#eed49f',
        brightBlue: '#8aadf4',
        brightMagenta: '#f5bde6',
        brightCyan: '#8bd5ca',
        brightWhite: '#a5adcb',
    },
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
    dracula: {
        '--c-bg': '#282a36',
        '--c-card': '#343746',
        '--c-text': '#f8f8f2',
        '--c-muted': '#9da5c4',
        '--c-accent': '#bd93f9',
        '--c-accent-hover': '#ff79c6',
        '--c-accent-text': '#282a36',
        '--c-border': '#44475a',
        '--c-ghost-hover': '#44475a',
    },
    nord: {
        '--c-bg': '#2e3440',
        '--c-card': '#3b4252',
        '--c-text': '#eceff4',
        '--c-muted': '#9ca7bd',
        '--c-accent': '#88c0d0',
        '--c-accent-hover': '#81a1c1',
        '--c-accent-text': '#2e3440',
        '--c-border': '#4c566a',
        '--c-ghost-hover': '#434c5e',
    },
    tokyo_night: {
        '--c-bg': '#1a1b26',
        '--c-card': '#24283b',
        '--c-text': '#c0caf5',
        '--c-muted': '#9aa5ce',
        '--c-accent': '#7aa2f7',
        '--c-accent-hover': '#2ac3de',
        '--c-accent-text': '#1a1b26',
        '--c-border': '#3b4261',
        '--c-ghost-hover': '#292e42',
    },
    gruvbox_dark: {
        '--c-bg': '#282828',
        '--c-card': '#3c3836',
        '--c-text': '#ebdbb2',
        '--c-muted': '#bdae93',
        '--c-accent': '#fabd2f',
        '--c-accent-hover': '#fe8019',
        '--c-accent-text': '#282828',
        '--c-border': '#504945',
        '--c-ghost-hover': '#3c3836',
    },
    one_dark: {
        '--c-bg': '#282c34',
        '--c-card': '#31353f',
        '--c-text': '#abb2bf',
        '--c-muted': '#848e98',
        '--c-accent': '#61afef',
        '--c-accent-hover': '#98c379',
        '--c-accent-text': '#282c34',
        '--c-border': '#3e4451',
        '--c-ghost-hover': '#3a3f4b',
    },
    rose_pine: {
        '--c-bg': '#191724',
        '--c-card': '#26233a',
        '--c-text': '#e0def4',
        '--c-muted': '#908caa',
        '--c-accent': '#c4a7e7',
        '--c-accent-hover': '#f6c177',
        '--c-accent-text': '#191724',
        '--c-border': '#403d52',
        '--c-ghost-hover': '#26233a',
    },
};

const TOKEN_KEYS = Object.keys(PALETTES.light);
const media = matchMedia('(prefers-color-scheme: dark)');

const NON_SYSTEM_MODES = Object.keys(PALETTES) as Exclude<ThemeMode, 'system'>[];

export function getThemeMode(): ThemeMode {
    const t = localStorage.getItem('appearance.theme');
    return t !== null && (NON_SYSTEM_MODES as string[]).includes(t) ? (t as ThemeMode) : 'system';
}

const DARK_MODES: ReadonlySet<string> = new Set(
    NON_SYSTEM_MODES.filter((m) => {
        // Judge darkness by actual bg luminance instead of hardcoding names.
        const bg = PALETTES[m]['--c-bg'] ?? '#ffffff';
        const r = parseInt(bg.slice(1, 3), 16), g = parseInt(bg.slice(3, 5), 16), b = parseInt(bg.slice(5, 7), 16);
        return (0.2126 * r + 0.7152 * g + 0.0722 * b) < 128;
    }),
);

export function isDarkTheme(t: ThemeMode, systemDark = media.matches): boolean {
    return t === 'dark' || DARK_MODES.has(t) || (t === 'system' && systemDark);
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
