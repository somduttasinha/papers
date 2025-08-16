<template>
    <n-layout>
        <!-- Top nav bar -->
        <!-- Page content -->
        <n-layout-content style="padding: 24px">
            <NCard size="large">
                <NUpload multiple directory-dnd :custom-request="customRequest">
                    <NUploadDragger>
                        <div style="margin-bottom: 12px">
                            <NIcon size="48" :depth="3">
                                <ArchiveIcon />
                            </NIcon>
                        </div>
                        <NText style="font-size: 16px">
                            Click or drag a file to this area to upload
                        </NText>
                    </NUploadDragger>
                </NUpload>
            </NCard>
        </n-layout-content>
    </n-layout>
</template>

<script setup>
import { NUpload, NUploadDragger } from "naive-ui";
import { NLayoutContent, NCard, NIcon } from "naive-ui";
import { ArchiveOutline as ArchiveIcon } from "@vicons/ionicons5";
const message = useMessage();

async function customRequest({ file, onFinish, onError }) {
    const formData = new FormData();
    formData.append("file", file.file);

    try {
        const res = await fetch("http://localhost:8080/api/docs/upload", {
            method: "POST",
            body: formData,
        });

        if (!res.ok) {
            throw new Error(await res.text());
        }

        onFinish();
        message.success(`${file.name} uploaded successfully`);
    } catch (err) {
        onError();
        message.error(`${file.name} upload failed`);
    }
}
</script>

<style scoped></style>
