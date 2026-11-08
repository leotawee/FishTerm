import {onUnmounted, unref} from 'vue';

// One captured `pointerdown` covers mouse, touch and pen, and beats inside-click handlers.
export default function useClickOutside() {
    function onClickOutside(target: any, callback: Function) {
        const listener = (event: PointerEvent) => {
            const el = unref(target)?.$el ?? unref(target);
            if (!el) return;
            if (el === event.target || event.composedPath().includes(el)) return;
            callback(event);
        };
        document.addEventListener('pointerdown', listener, true);
        onUnmounted(() => document.removeEventListener('pointerdown', listener, true));
    }

    return {
        onClickOutside,
    };
}
