import {createApp} from 'vue';
import App from './App.vue';
import './style.css';
import '@xterm/xterm/css/xterm.css';
import '../assets/fonts.css';

import FontAwesome from './font-awesome';

createApp(App).component('Icon', FontAwesome).mount('#app');
