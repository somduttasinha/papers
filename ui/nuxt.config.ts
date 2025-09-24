import tailwindcss from "@tailwindcss/vite";
import AutoImport from "unplugin-auto-import/vite";
import { NaiveUiResolver } from "unplugin-vue-components/resolvers";
import Components from "unplugin-vue-components/vite";

// https://nuxt.com/docs/api/configuration/nuxt-config
export default defineNuxtConfig({
    devtools: { enabled: true },
    css: ["~/assets/css/main.css"],
    modules: ["nuxtjs-naive-ui"],
    compatibilityDate: "2025-07-15",
    runtimeConfig: {
        public: {
            apiBase: process.env.API_BASE,
        },
    },
    vite: {
        plugins: [
            AutoImport({
                imports: [
                    {
                        "naive-ui": [
                            "useDialog",
                            "useMessage",
                            "useNotification",
                            "useLoadingBar",
                        ],
                    },
                ],
            }),
            Components({
                resolvers: [NaiveUiResolver()],
            }),
            tailwindcss(),
        ],
    },
    build: {
        transpile: ["naive-ui", "vueuc"],
    },
});
