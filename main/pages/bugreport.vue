<template>
  <div class="max-w-2xl mx-auto px-6 py-8">
    <div class="mb-6">
      <h1 class="text-2xl font-bold font-display text-zinc-100">
        Report a Bug
      </h1>
      <p class="text-sm text-zinc-400 mt-1">
        Describe what went wrong. System info and recent logs will be attached
        automatically.
      </p>
    </div>

    <div v-if="submitted" class="rounded-xl bg-green-600/10 ring-1 ring-green-500/20 p-6 text-center">
      <CheckCircleIcon class="mx-auto h-10 w-10 text-green-400 mb-3" />
      <p class="text-lg font-semibold text-green-300">Bug report submitted!</p>
      <p class="text-sm text-zinc-400 mt-1">
        An admin will review it shortly.
      </p>
      <button
        class="mt-4 px-4 py-2 rounded-md text-sm font-medium bg-zinc-800 text-zinc-200 hover:bg-zinc-700"
        @click="resetForm"
      >
        Submit another
      </button>
    </div>

    <form v-else class="space-y-5" @submit.prevent="submitReport">
      <!-- Title -->
      <div>
        <label class="block text-sm font-medium text-zinc-300 mb-1">
          What happened? <span class="text-red-400">*</span>
        </label>
        <input
          v-model="title"
          type="text"
          maxlength="200"
          class="w-full rounded-md border border-zinc-700 bg-zinc-800 px-3 py-2 text-sm text-zinc-100 placeholder-zinc-500 outline-none focus:border-blue-500 focus:ring-1 focus:ring-blue-500/50"
          placeholder="e.g. Game crashes when launching Mario Kart"
        />
      </div>

      <!-- Description -->
      <div>
        <label class="block text-sm font-medium text-zinc-300 mb-1">
          Steps to reproduce (optional)
        </label>
        <textarea
          v-model="description"
          maxlength="2000"
          rows="5"
          class="w-full rounded-md border border-zinc-700 bg-zinc-800 px-3 py-2 text-sm text-zinc-100 placeholder-zinc-500 outline-none focus:border-blue-500 focus:ring-1 focus:ring-blue-500/50 resize-none"
          placeholder="1. Open the library&#10;2. Click Play on Mario Kart&#10;3. Game window opens then immediately closes"
        />
      </div>

      <!-- Screenshot -->
      <div>
        <label class="block text-sm font-medium text-zinc-300 mb-1">
          Screenshot (optional)
        </label>
        <label
          for="bug-screenshot"
          class="group cursor-pointer transition relative block w-full rounded-lg border-2 border-dashed border-zinc-700 p-4 text-center hover:border-zinc-600"
        >
          <img
            v-if="screenshotPreview"
            :src="screenshotPreview"
            class="mx-auto max-h-32 rounded-md"
          />
          <PhotoIcon
            v-else
            class="mx-auto h-6 w-6 text-zinc-500 group-hover:text-zinc-400"
          />
          <span class="mt-1 block text-xs text-zinc-500 group-hover:text-zinc-400">
            {{ screenshotFile ? screenshotFile.name : "Click to attach a screenshot" }}
          </span>
        </label>
        <input
          id="bug-screenshot"
          class="hidden"
          type="file"
          accept="image/*"
          @change="onScreenshotSelect"
        />
      </div>

      <!-- Include logs toggle -->
      <div class="flex items-center gap-3">
        <input
          id="include-logs"
          v-model="includeLogs"
          type="checkbox"
          class="h-4 w-4 rounded border-zinc-700 bg-zinc-800 text-blue-500 focus:ring-blue-500/50"
        />
        <label for="include-logs" class="text-sm text-zinc-300">
          Include recent client logs (last 200 lines)
        </label>
      </div>

      <!-- Auto-collected info hint -->
      <div class="rounded-md bg-zinc-800/50 px-3 py-2 text-xs text-zinc-500">
        <InformationCircleIcon class="inline h-3.5 w-3.5 mr-1 -mt-0.5" />
        System info (OS, CPU, RAM, client version) will be attached automatically.
      </div>

      <!-- Error display -->
      <div v-if="submitError" class="rounded-md bg-red-600/10 p-3">
        <p class="text-sm text-red-400">{{ submitError }}</p>
      </div>

      <div class="flex justify-end gap-3 pt-2">
        <button
          type="button"
          class="px-4 py-2 rounded-md text-sm text-zinc-300 hover:text-zinc-100"
          @click="$router.back()"
        >
          Cancel
        </button>
        <button
          type="submit"
          :disabled="!title.trim() || submitting"
          class="px-5 py-2 rounded-md text-sm font-medium bg-red-600 hover:bg-red-500 text-white disabled:opacity-50 disabled:cursor-not-allowed inline-flex items-center gap-2"
        >
          <span
            v-if="submitting"
            class="h-4 w-4 border-2 border-white/30 border-t-white rounded-full animate-spin"
          />
          {{ submitting ? "Submitting..." : "Submit Bug Report" }}
        </button>
      </div>
    </form>
  </div>
</template>

<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import {
  PhotoIcon,
  InformationCircleIcon,
  CheckCircleIcon,
} from "@heroicons/vue/24/outline";
import { serverUrl } from "~/composables/use-server-fetch";

const title = ref("");
const description = ref("");
const screenshotFile = ref<File | null>(null);
const screenshotPreview = ref<string | null>(null);
const includeLogs = ref(true);
const submitting = ref(false);
const submitError = ref<string | undefined>();
const submitted = ref(false);

function onScreenshotSelect(e: Event) {
  const files = (e.target as HTMLInputElement)?.files;
  if (!files?.length) return;
  screenshotFile.value = files[0];

  const reader = new FileReader();
  reader.onload = () => {
    screenshotPreview.value = reader.result as string;
  };
  reader.readAsDataURL(files[0]);
}

async function submitReport() {
  if (!title.value.trim()) return;
  submitting.value = true;
  submitError.value = undefined;

  try {
    // Collect diagnostics from Tauri
    const diagnostics = await invoke<Record<string, string>>(
      "collect_bug_report_diagnostics",
    );

    // Collect the full log file (redacted) if opted in. Fall back to a
    // short text tail if the file read fails.
    let logFile: { filename: string; content: number[]; truncated: boolean } | undefined;
    let logsText: string | undefined;
    if (includeLogs.value) {
      try {
        logFile = await invoke("collect_bug_report_log_file");
      } catch (e) {
        console.warn("Failed to collect log file, falling back to text tail:", e);
        try {
          logsText = await invoke<string>("collect_bug_report_logs", { maxLines: 400 });
        } catch (inner) {
          console.warn("Failed to collect log tail:", inner);
        }
      }
    }

    // Add surface indicator
    diagnostics.surface = "client";

    // Build form data for multipart upload (handles screenshots)
    const form = new FormData();
    form.append("title", title.value.trim());
    form.append("description", description.value.trim());
    form.append("systemInfo", JSON.stringify(diagnostics));
    if (logFile) {
      const blob = new Blob([new Uint8Array(logFile.content)], { type: "text/plain" });
      form.append("logfile", blob, logFile.filename);
      if (logFile.truncated) {
        form.append("logfileTruncated", "1");
      }
    } else if (logsText) {
      form.append("logs", logsText);
    }
    if (screenshotFile.value) {
      form.append("screenshot", screenshotFile.value);
    }

    // Submit via server:// protocol
    const url = serverUrl("api/v1/bugreports/create");
    const response = await fetch(url, {
      method: "POST",
      body: form,
    });

    if (!response.ok) {
      const err = await response.text().catch(() => "Unknown error");
      throw new Error(err);
    }

    submitted.value = true;
  } catch (error: any) {
    submitError.value = error.message || "Failed to submit bug report";
  } finally {
    submitting.value = false;
  }
}

function resetForm() {
  title.value = "";
  description.value = "";
  screenshotFile.value = null;
  screenshotPreview.value = null;
  includeLogs.value = true;
  submitError.value = undefined;
  submitted.value = false;
}
</script>
