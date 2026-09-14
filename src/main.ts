import "temporal-polyfill/global";
import "./style.css";

import { QueryClient, VueQueryPlugin } from "@tanstack/vue-query";
import { createPinia } from "pinia";
import { createApp } from "vue";

import App from "./App.vue";

const queryClient = new QueryClient();

createApp(App).use(createPinia()).use(VueQueryPlugin, { queryClient }).mount("#app");
