<template>
  <article
    class="doc-card"
    tabindex="0"
    @keydown.enter.prevent="emitView"
    @keydown.space.prevent="emitView"
    :aria-label="`Open document ${title || docId}`"
  >
    <div class="thumb">
      <img :src="imageSrc" :alt="titleAlt" loading="lazy" />
    </div>

    <div class="meta">
      <h3 class="title" v-if="title">{{ title }}</h3>
    </div>

    <div class="actions">
      <a
        class="action"
        :href="`${apiBase}/api/docs/preview/${docId}`"
        :target="`_blank`"
        aria-label="View document"
      >
        <Eye16Regular />
      </a>

      <a
        class="action"
        :href="`${apiBase}/api/docs/download/${docId}`"
        :target="`_blank`"
        aria-label="Download document"
      >
        <ArrowDownload16Regular />
      </a>

      <!-- TODO: complete the share functionality -->
      <a class="action">
        <Share16Regular />
      </a>

      <!-- TODO: complete the delete functionality -->
      <a 
        class="action"
        @click.prevent="emitDelete"
        href="#"
        aria-label="Delete document">
        <Delete16Regular />
      </a>
    </div>
  </article>
</template>

<script setup lang="ts">
import { computed } from "vue";
import {
  ArrowDownload16Regular,
  Eye16Regular,
  Share16Regular,
  Delete16Regular,
} from "@vicons/fluent";
const props = defineProps<{
  imageSrc: string;
  docId: string | number;
  title: string;
}>();

const emit = defineEmits<{
  (e: "view", docId: string | number): void;
  (e: "download", docId: string | number): void;
  (e: "delete", docId: string | number): void;
}>();

const {
  public: { apiBase },
} = useRuntimeConfig();

const emitView = () => emit("view", props.docId);
const emitDelete = () => emit("delete", props.docId);

const titleAlt = computed(() =>
  props.title ? `Preview of ${props.title}` : "Document preview",
);
</script>

<style scoped>
.doc-card {
  display: flex;
  width: 200px;
  flex-direction: column;
  gap: 0.4rem;
  border: 1px solid #e5e7eb;
  border-radius: 12px;
  padding: 0.5rem;
  background: #fff;
  box-shadow: 0 1px 2px rgba(0, 0, 0, 0.04);
  transition:
    box-shadow 0.2s,
    transform 0.05s,
    border-color 0.2s;
  cursor: pointer;
  outline: none;
}

.doc-card:focus-visible {
  box-shadow: 0 0 0 3px rgba(59, 130, 246, 0.35);
}

.doc-card:hover {
  box-shadow: 0 6px 18px rgba(0, 0, 0, 0.08);
  border-color: #d1d5db;
}

.doc-card:active {
  transform: translateY(1px);
}

.thumb {
  /*position: relative;*/
  width: 100%;
  aspect-ratio: 3/4;
  /* nice doc feel */
  border-radius: 10px;
  overflow: hidden;
  background: #f3f4f6;
}

.thumb img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
}

.meta {
  display: flex;
  flex-direction: column;
  gap: 0.35rem;
  padding: 0 0.25rem;
}

.title {
  font-size: 1rem;
  font-weight: 500;
  color: #111827;
  margin: 0;
  line-height: 1.2;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.row {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  color: #374151;
  font-size: 0.9rem;
}

.ico {
  width: 20px;
  height: 20px;
  display: inline-flex;
}

svg {
  width: 100%;
  height: 100%;
  fill: currentColor;
}

.actions {
  display: flex;
  justify-content: flex-end;
  gap: 0.5rem;
  margin-top: 0.25rem;
  border-top: 1px solid #f3f4f6;
  padding-top: 0.5rem;
}

.action {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 30px;
  height: 30px;
  border-radius: 10px;
  border: 1px solid #e5e7eb;
  background: #fff;
  color: #111827;
  transition:
    background 0.2s,
    border-color 0.2s,
    transform 0.05s;
  text-decoration: none;
}

.action:hover {
  background: #f9fafb;
  border-color: #d1d5db;
}

.action:active {
  transform: translateY(1px);
}

.action:focus-visible {
  outline: none;
  box-shadow: 0 0 0 3px rgba(59, 130, 246, 0.35);
}

.action svg {
  width: 30px;
  height: 30px;
}
</style>
