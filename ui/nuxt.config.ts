import AutoImport from "unplugin-auto-import/vite";
import { NaiveUiResolver } from "unplugin-vue-components/resolvers";
import Components from "unplugin-vue-components/vite";

// https://nuxt.com/docs/api/configuration/nuxt-config
export default defineNuxtConfig({
    devtools: { enabled: true },
    modules: ["nuxtjs-naive-ui"],
    compatibilityDate: "2025-07-15",
    runtimeConfig: {
        public: {
            apiBase: "http://localhost:8080",
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
        ],
    },
    build: {
        transpile: ["naive-ui", "vueuc"],
    },
});
