<template>
    <div class="documents">
        <div v-if="loading">Loading documents...</div>
        <div v-else-if="error">Error: {{ error }}</div>
        <DocumentCard v-else v-for="document in documents" :key="document.id" :image-src="document.thumbnail_url"
            :doc-id="document.id" :title="document.title" />
    </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from "vue";
import DocumentCard from "../components/DocumentCard.vue";

interface Document {
    id: string;
    thumbnail_url: string;
    contents: string;
    title: string;
}

const documents = ref<Document[]>([]);
const loading = ref(true);
const error = ref("");

let aborter: AbortController | null = null;

onMounted(async () => {
    try {
        const response = await fetch("http://localhost:8080/api/docs");
        if (!response.ok) {
            throw new Error(`HTTP error! status: ${response.status}`);
        }
        const data = await response.json();
        if (Array.isArray(data)) {
            documents.value = data;
        } else {
            console.error("Failed to fetch documents: Response is not an array");
            error.value = "Failed to fetch documents: Response is not an array";
        }
    } catch (e: any) {
        console.error("Failed to fetch documents:", e);
        error.value = e.message;
    } finally {
        loading.value = false;
    }
});
</script>

<style scoped>
.documents {
    display: flex;
    flex-wrap: wrap;
    gap: 1rem;
}
</style>
