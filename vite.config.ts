import {defineConfig} from 'vite';
import vue from '@vitejs/plugin-vue';

// WHY: Tauri loads the dev server at a fixed port and the built files from `dist/`.
export default defineConfig({
    root: 'ui',
    plugins: [vue()],
    clearScreen: false,
    server: {port: 1420, strictPort: true},
    build: {
        outDir: '../dist',
        emptyOutDir: true,
        target: 'es2022',
    },
});
