import {defineConfig} from 'vite';
import vue from '@vitejs/plugin-vue';
import tailwindcss from 'tailwindcss';
import pkg from './package.json' with {type: 'json'};

// WHY: Tauri loads the dev server at a fixed port and the built files from `dist/`.
export default defineConfig({
    root: 'ui',
    plugins: [vue()],
    clearScreen: false,
    server: {port: 1420, strictPort: true},
    // Tailwind v3 like upstream FishTerm; default theme, dark mode follows the OS.
    css: {postcss: {plugins: [tailwindcss({content: ['ui/index.html', 'ui/src/**/*.{vue,ts}']})]}},
    define: {__APP_VERSION__: JSON.stringify(pkg.version)},
    build: {
        outDir: '../dist',
        emptyOutDir: true,
        target: 'es2022',
    },
});
