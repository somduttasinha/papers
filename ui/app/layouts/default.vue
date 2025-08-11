<template>
    <n-layout has-sider style="min-height: 100vh">
        <!-- Sidebar -->
        <n-layout-sider bordered collapse-mode="width" :collapsed-width="64" :width="200" show-trigger
            collapse-trigger="bar">
            <div class="logo">📚</div>
            <n-menu :options="menuOptions" @update:value="handleUpdateValue" />
        </n-layout-sider>

        <!-- Main layout -->
        <n-layout>
            <n-layout-header bordered class="top-nav">
                <div class="nav-content">
                    <div class="nav-left">
                        <h2 class="logo">papers</h2>
                    </div>
                    <div class="nav-centre">
                        <n-input round placeholder="Search..." clearable size="small" style="width: 200px" />
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
    BookOutline as BookIcon,
    HomeOutline as HomeIcon,
    PersonOutline as PersonIcon,
    WineOutline as WineIcon,
} from "@vicons/ionicons5";
import { NIcon, useMessage } from "naive-ui";

function renderIcon(icon: Component) {
    return () => h(NIcon, null, { default: () => h(icon) });
}

const message = useMessage();

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

function handleUpdateValue(key: string, item: MenuOption) {
    message.info(`Selected: ${key}`);
}
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
