<template>
  <div class="documents">
    <div v-if="loading">Loading documents...</div>
    <div v-else-if="error">Error: {{ error }}</div>
    <DocumentCard
      @view="previewDoc"
      @download="downloadDoc"
      @delete="deleteDoc"
      v-else
      v-for="document in documents"
      :key="document.id"
      :image-src="document.thumbnail_url"
      :doc-id="document.id"
      :title="document.title"
    />
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

async function downloadDoc(docId: string | number) {
  if (aborter) {
    aborter.abort();
  }
  aborter = new AbortController();
  const response = await fetch(
    useRuntimeConfig().public.apiBase + `/api/docs/download/${docId}`,
    {
      signal: aborter.signal,
    },
  );
  if (!response.ok) {
    throw new Error(`HTTP error! status: ${response.status}`);
  }

  const blob = await response.blob();
  const url = window.URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = url;
  link.download = "document.pdf";
  document.body.appendChild(link);
  link.click();
  document.body.removeChild(link);
}

async function deleteDoc(docId: string | number) {
  if (!confirm("Are you sure you want to delete this document?")) {
    return;
  }

  try {
    const response = await fetch(
      useRuntimeConfig().public.apiBase + `/api/docs/delete/${docId}`,
      {
        method: "DELETE",
      },
    );

    if (!response.ok) {
      throw new Error(`HTTP error! status: ${response.status}`);
    }

    documents.value = documents.value.filter((doc) => doc.id !== docId);
  } catch (e: any) {
    console.error("Failed to delete document:", e);
    alert("Failed to delete document: " + e.message);
    error.value = e.message;
    return;
  }
}

async function previewDoc(docId: string | number) {
  if (aborter) {
    aborter.abort();
  }
  aborter = new AbortController();
  const response = await fetch(
    useRuntimeConfig().public.apiBase + `/api/docs/preview/${docId}`,
    {
      signal: aborter.signal,
    },
  );
  if (!response.ok) {
    throw new Error(`HTTP error! status: ${response.status}`);
  }
  const presigned_url = await response.text();
  window.open(presigned_url, "_blank");
}

onMounted(async () => {
  try {
    const response = await fetch(
      useRuntimeConfig().public.apiBase + "/api/docs",
    );
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
