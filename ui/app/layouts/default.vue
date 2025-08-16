<template>
    <n-layout has-sider style="min-height: 100vh">
        <!-- Sidebar -->
        <n-layout-sider bordered collapse-mode="width" :collapsed-width="64" :width="200" show-trigger
            collapse-trigger="bar">
            <div class="logo">
                <NuxtLink to="/" class="flex items-center space-x-2">
                    <img src="/papers-logo.svg" alt="Papers logo" />
                </NuxtLink>
            </div>

            <n-menu :options="menuOptions" />
        </n-layout-sider>

        <!-- Main layout -->
        <n-layout>
            <n-layout-header bordered class="top-nav">
                <div class="nav-content">
                    <div class="nav-left">
                        <!-- render this svg: public/papers-logo.svg -->
                    </div>
                    <div class="nav-centre">
                        <NAutoComplete v-model:value="q" :options="options" placeholder="Search..." clearable
                            size="small" round @select="onSelect" />
                    </div>

                    <div class="nav-right">
                        <n-button quaternary icon-placement="right" style="margin-left: 12px">
                            <template #icon>
                                <n-icon><i class="i-ph-user-circle-bold" /></n-icon>
                            </template>
                            User
                        </n-button>
                    </div>
                </div>
            </n-layout-header>

            <n-layout-content content-style="padding: 24px;">
                <slot />
            </n-layout-content>
        </n-layout>
    </n-layout>
</template>

<script lang="ts" setup>
import type { MenuOption } from "naive-ui";
import type { Component } from "vue";
import { h } from "vue";
import { RouterLink } from "vue-router";
import {
    DocumentOutline as DocumentIcon,
    HomeOutline as HomeIcon,
} from "@vicons/ionicons5";

import { useDebounceFn } from "@vueuse/core";

import { NAutoComplete, NIcon } from "naive-ui";

function renderIcon(icon: Component) {
    return () => h(NIcon, null, { default: () => h(icon) });
}

const q = ref("");
const options = ref<string[]>([]);
const {
    public: { apiBase = "" },
} = useRuntimeConfig();

let aborter: AbortController | null = null;

const fetchSuggestions = useDebounceFn(async () => {
    if (!q.value.trim()) {
        options.value = [];
        return;
    }

    aborter?.abort();
    aborter = new AbortController();

    const { data, error } = await useFetch<string[]>(`${apiBase}/api/search`, {
        method: "GET",
        query: {
            query: q.value,
        },
        signal: aborter.signal as any,
    });

    if (!error.value && Array.isArray(data.value)) {
        options.value = data.value;
    } else {
        options.value = [];
    }
}, 200);

watch([q], () => {
    fetchSuggestions();
});

function onSelect(value: string) {
    q.value = value;
    console.log("selected", value);
}

const menuOptions: MenuOption[] = [
    {
        label: () =>
            h(RouterLink, { to: { name: "index" } }, { default: () => "Dashboard" }),
        key: "dashboard",
        icon: renderIcon(HomeIcon),
    },
    {
        key: "divider-1",
        type: "divider",
    },
    {
        label: () =>
            h(
                RouterLink,
                { to: { name: "documents" } },
                { default: () => "Documents" },
            ),
        key: "documents",
        icon: renderIcon(DocumentIcon),
    },
];
</script>

<style scoped>
.logo {
    text-align: center;
    font-size: 2rem;
    padding: 16px;
}

.header {
    background-color: #fff;
    padding: 0 24px;
}

.topbar {
    display: flex;
    align-items: center;
    height: 100%;
}

.top-nav {
    height: 64px;
    background-color: #ffffff;
    display: flex;
    align-items: center;
    padding: 0 24px;
}

.nav-content {
    display: flex;
    justify-content: space-between;
    width: 100%;
    align-items: center;
}

.logo {
    margin: 0;
    font-size: 1.5rem;
    font-weight: 600;
    color: #18a058;
    /* Naive UI primary */
}

.nav-right {
    display: flex;
    align-items: center;
}

.nav-centre {
    display: flex;
    align-items: center;
}
</style>
