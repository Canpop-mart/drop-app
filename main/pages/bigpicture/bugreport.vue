<template>
  <div class="flex-1 overflow-y-auto px-8 py-6">
    <div class="max-w-2xl mx-auto">
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
          :ref="(el: any) => registerContent(el, { onSelect: resetForm })"
          class="mt-4 px-4 py-2 rounded-md text-sm font-medium bg-zinc-800 text-zinc-200 hover:bg-zinc-700"
          @click="resetForm"
        >
          Submit another
        </button>
      </div>

      <form v-else class="space-y-5" @submit.prevent="submitReport">
        <!-- On-screen keyboard for title -->
        <BigPictureKeyboard
          :visible="showTitleKeyboard"
          :model-value="title"
          placeholder="e.g. Game crashes when launching Mario Kart"
          @update:model-value="title = $event"
          @close="showTitleKeyboard = false"
          @submit="showTitleKeyboard = false"
        />

        <!-- On-screen keyboard for description -->
        <BigPictureKeyboard
          :visible="showDescKeyboard"
          :model-value="description"
          placeholder="Steps to reproduce..."
          @update:model-value="description = $event"
          @close="showDescKeyboard = false"
          @submit="showDescKeyboard = false"
        />
        <!-- Title -->
        <div>
          <label class="block text-sm font-medium text-zinc-300 mb-1">
            What happened?
          </label>
          <button
            :ref="(el: any) => registerContent(el, { onSelect: () => (showTitleKeyboard = true) })"
            type="button"
            class="w-full rounded-md border border-zinc-700 bg-zinc-800 px-3 py-2 text-sm text-left outline-none focus:border-blue-500 focus:ring-1 focus:ring-blue-500/50 min-h-[2.5rem]"
            :class="title ? 'text-zinc-100' : 'text-zinc-500'"
            @click="showTitleKeyboard = true"
          >
            {{ title || 'e.g. Game crashes when launching Mario Kart' }}
          </button>
        </div>

        <!-- Description -->
        <div>
          <label class="block text-sm font-medium text-zinc-300 mb-1">
            Steps to reproduce (optional)
          </label>
          <button
            :ref="(el: any) => registerContent(el, { onSelect: () => (showDescKeyboard = true) })"
            type="button"
            class="w-full rounded-md border border-zinc-700 bg-zinc-800 px-3 py-2 text-sm text-left outline-none focus:border-blue-500 focus:ring-1 focus:ring-blue-500/50 min-h-[6rem] whitespace-pre-wrap"
            :class="description ? 'text-zinc-100' : 'text-zinc-500'"
            @click="showDescKeyboard = true"
          >
            {{ description || '1. Open the library\n2. Click Play on a game\n3. Describe what happened' }}
          </button>
        </div>

        <!-- Include logs toggle -->
        <div
          :ref="(el: any) => registerContent(el, { onSelect: () => (includeLogs = !includeLogs) })"
          class="flex items-center gap-3 cursor-pointer"
          @click="includeLogs = !includeLogs"
        >
          <div
            class="h-5 w-5 rounded border flex items-center justify-center transition-colors"
            :class="includeLogs ? 'bg-blue-600 border-blue-600' : 'border-zinc-600 bg-zinc-800'"
          >
            <svg v-if="includeLogs" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="currentColor" class="size-3.5 text-white">
              <path fill-rule="evenodd" d="M16.704 4.153a.75.75 0 0 1 .143 1.052l-8 10.5a.75.75 0 0 1-1.127.075l-4.5-4.5a.75.75 0 0 1 1.06-1.06l3.894 3.893 7.48-9.817a.75.75 0 0 1 1.05-.143Z" clip-rule="evenodd" />
            </svg>
          </div>
          <span class="text-sm text-zinc-300">Include recent client logs (last 200 lines)</span>
        </div>

        <!-- Auto-collected info hint -->
        <div class="rounded-md bg-zinc-800/50 px-3 py-2 text-xs text-zinc-500">
          System info (OS, CPU, RAM, client version) will be attached automatically.
          Secrets like auth tokens are redacted from the log before upload.
        </div>

        <!-- Open log folder (useful when running in SteamDeckDesktop) -->
        <div class="flex justify-start">
          <button
            :ref="(el: any) => registerContent(el, { onSelect: openLogFolder })"
            type="button"
            class="inline-flex items-center gap-2 px-3 py-2 rounded-md text-xs text-zinc-400 hover:text-zinc-200 bg-zinc-800 hover:bg-zinc-700"
            @click="openLogFolder"
          >
            <svg class="size-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z" />
            </svg>
            {{ logFolderOpened ? "Opened!" : "Open log folder" }}
          </button>
        </div>

        <!-- Error display -->
        <div v-if="submitError" class="rounded-md bg-red-600/10 p-3">
          <p class="text-sm text-red-400">{{ submitError }}</p>
        </div>

        <div class="flex justify-end gap-3 pt-2">
          <button
            :ref="(el: any) => registerContent(el, { onSelect: () => navigateTo('/bigpicture/settings') })"
            type="button"
            class="px-4 py-2 rounded-md text-sm text-zinc-300 hover:text-zinc-100"
            @click="navigateTo('/bigpicture/settings')"
          >
            Cancel
          </button>
          <button
            :ref="(el: any) => registerContent(el, { onSelect: submitReport })"
            type="submit"
            :disabled="!title.trim() || submitting"
            class="px-5 py-2 rounded-md text-sm font-medium bg-red-600 hover:bg-red-500 text-white disabled:opacity-50 disabled:cursor-not-allowed inline-flex items-center gap-2"
            @click="submitReport"
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
  </div>
</template>

<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { CheckCircleIcon } from "@heroicons/vue/24/outline";
import BigPictureKeyboard from "~/components/bigpicture/BigPictureKeyboard.vue";
import { serverUrl } from "~/composables/use-server-fetch";
import { useBpFocusableGroup } from "~/composables/bp-focusable";

definePageMeta({ layout: "bigpicture" });

const registerContent = useBpFocusableGroup("content");

const route = useRoute();
const title = ref(typeof route.query.title === "string" ? route.query.title : "");
const description = ref(typeof route.query.body === "string" ? route.query.body : "");
const showTitleKeyboard = ref(false);
const showDescKeyboard = ref(false);
// If caller passed ?attachLog=1 (launch-error handoff), default to attaching.
const includeLogs = ref(route.query.attachLog !== "0");
const submitting = ref(false);
const submitError = ref<string | undefined>();
const submitted = ref(false);
const logFolderOpened = ref(false);

async function openLogFolder() {
  try {
    await invoke("open_log_folder");
    logFolderOpened.value = true;
    setTimeout(() => { logFolderOpened.value = false; }, 2000);
  } catch (e) {
    console.warn("Failed to open log folder:", e);
  }
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
    // short text tail if the file read fails for any reason — the server
    // accepts either `logfile` (multipart) or `logs` (text) today.
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
    diagnostics.surface = "client-bpm";

    // Build form data
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
  } catch (error: unknown) {
    const err = error as { message?: string };
    submitError.value = err.message || "Failed to submit bug report";
  } finally {
    submitting.value = false;
  }
}

function resetForm() {
  title.value = "";
  description.value = "";
  includeLogs.value = true;
  submitError.value = undefined;
  submitted.value = false;
}
</script>
