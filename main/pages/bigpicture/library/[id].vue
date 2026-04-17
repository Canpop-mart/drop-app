<template>
  <div class="flex flex-col h-full overflow-y-auto" :style="{ backgroundColor: 'var(--bpm-bg)', color: 'var(--bpm-text)' }">
    <!-- Hero banner -->
    <div class="relative shrink-0 h-96">
      <div v-if="!game" class="w-full h-full bg-zinc-800/50 animate-pulse" />
      <template v-else>
        <img
          v-if="game.mBannerObjectId"
          :src="objectUrl(game.mBannerObjectId)"
          class="w-full h-full object-cover"
        />
        <div v-else class="w-full h-full bg-zinc-800/30" />
      </template>
      <div
        v-if="game"
        class="absolute inset-0 bg-gradient-to-r from-zinc-950 via-zinc-950/80 to-zinc-950/20"
      />
      <div
        v-if="game"
        class="absolute inset-0 bg-gradient-to-t from-zinc-950 via-zinc-950/60 to-transparent"
      />

      <!-- Game info overlay -->
      <div v-if="game" class="absolute bottom-0 left-0 right-0 p-8">
        <h1 class="text-5xl font-bold font-display text-zinc-100 mb-2" style="text-shadow: 0 2px 8px rgba(0,0,0,0.8), 0 0 2px rgba(0,0,0,0.6)">
          {{ game?.mName }}
        </h1>
        <p
          v-if="game?.mShortDescription"
          class="text-lg text-zinc-400 max-w-2xl mb-6"
          style="text-shadow: 0 1px 4px rgba(0,0,0,0.8)"
        >
          {{ game.mShortDescription }}
        </p>

        <!-- Action buttons -->
        <div class="flex items-center gap-3">
          <!-- ── Installed: Play button with dropdown ── -->
          <div v-if="status?.type === 'Installed'" class="relative inline-flex">
            <div
              :ref="(el: any) => registerAction(el, { onSelect: launchGame, onContext: togglePlayMenu })"
              class="bp-focus-delegate inline-flex cursor-pointer"
            >
              <span class="bp-focus-ring inline-flex rounded-xl">
                <button
                  class="inline-flex items-center pl-8 pr-4 py-4 text-lg gap-3 font-semibold rounded-l-xl transition-all shadow-lg bg-blue-600 hover:bg-blue-400 text-white shadow-blue-600/20 hover:shadow-blue-500/30 hover:scale-105"
                  @click.stop="launchGame"
                >
                  <PlayIcon class="size-6" />
                  Play
                </button>
                <button
                  class="inline-flex items-center px-3 py-4 font-semibold rounded-r-xl transition-all shadow-lg border-l bg-blue-600 hover:bg-blue-400 text-white border-blue-400/30"
                  @click.stop="togglePlayMenu"
                >
                  <ChevronDownIcon class="size-5" :class="{ 'rotate-180': playMenuOpen }" />
                </button>
              </span>
            </div>
            <!-- Dropdown menu -->
            <Transition name="dropdown-fade">
              <div
                v-if="playMenuOpen"
                class="absolute left-0 top-full mt-2 z-50 min-w-[280px] rounded-xl bg-zinc-900 border border-zinc-700/50 shadow-2xl overflow-hidden"
              >
                <!-- Play locally -->
                <button
                  class="flex items-center gap-3 w-full px-6 py-3.5 text-left text-base transition-colors"
                  :class="playMenuFocus === 0 ? 'bg-blue-600 text-white' : 'text-zinc-300 hover:bg-zinc-800'"
                  @click="selectPlayMenuAction(0)"
                  @mouseenter="playMenuFocus = 0"
                >
                  <PlayIcon class="size-5" />
                  <span class="font-medium">Play</span>
                </button>
                <!-- Stream to other devices -->
                <template v-for="(device, i) in otherDevices" :key="'stream-' + device.id">
                  <button
                    class="flex items-center gap-3 w-full px-6 py-3.5 text-left text-base transition-colors"
                    :class="playMenuFocus === 1 + i ? 'bg-purple-600 text-white' : 'text-zinc-300 hover:bg-zinc-800'"
                    @click="selectPlayMenuAction(1 + i)"
                    @mouseenter="playMenuFocus = 1 + i"
                  >
                    <SignalIcon class="size-5 text-purple-400" />
                    <span class="font-medium">Stream to {{ device.name }}</span>
                    <span class="text-xs opacity-50 ml-auto">{{ device.platform }}</span>
                  </button>
                </template>
                <!-- Install on other devices -->
                <template v-for="(device, i) in otherDevices" :key="'install-' + device.id">
                  <button
                    class="flex items-center gap-3 w-full px-6 py-3.5 text-left text-base transition-colors"
                    :class="playMenuFocus === 1 + otherDevices.length + i ? 'bg-green-600 text-white' : 'text-zinc-300 hover:bg-zinc-800'"
                    @click="selectPlayMenuAction(1 + otherDevices.length + i)"
                    @mouseenter="playMenuFocus = 1 + otherDevices.length + i"
                  >
                    <ArrowDownTrayIcon class="size-5 text-green-400" />
                    <span class="font-medium">Install on {{ device.name }}</span>
                    <span class="text-xs opacity-50 ml-auto">{{ device.platform }}</span>
                  </button>
                </template>
                <!-- Divider + message if no other devices -->
                <div
                  v-if="otherDevices.length === 0"
                  class="px-6 py-3 text-sm text-zinc-500 border-t border-zinc-800/50"
                >
                  No other devices registered
                </div>
              </div>
            </Transition>
            <div v-if="playMenuOpen" class="fixed inset-0 z-40" @click="playMenuOpen = false" />
          </div>

          <!-- ── Running: Stop button ── -->
          <button
            v-else-if="status?.type === 'Running'"
            :ref="(el: any) => registerAction(el, { onSelect: killGame })"
            class="inline-flex items-center px-8 py-4 text-lg gap-3 bg-red-600 hover:bg-red-500 text-white font-semibold rounded-xl transition-colors"
            @click="killGame"
          >
            <StopIcon class="size-6" />
            Stop
          </button>

          <!-- ── Downloading/Queued: Status ── -->
          <button
            v-else-if="status?.type === 'Downloading' || status?.type === 'Queued'"
            class="inline-flex items-center px-8 py-4 text-lg gap-3 font-semibold rounded-xl cursor-not-allowed"
            style="background-color: rgba(59,130,246,0.2); color: rgb(147,197,253)"
            disabled
          >
            <ArrowDownTrayIcon class="size-6 animate-bounce" />
            {{ status?.type === "Downloading" ? "Downloading..." : "Queued" }}
          </button>

          <!-- ── Not installed: Install button with device picker ── -->
          <div v-else class="relative inline-flex">
            <div
              :ref="(el: any) => registerAction(el, { onSelect: downloadGame, onContext: otherDevices.length > 0 ? togglePlayMenu : undefined })"
              class="bp-focus-delegate inline-flex cursor-pointer"
            >
              <span class="bp-focus-ring inline-flex rounded-xl">
                <button
                  class="inline-flex items-center pl-8 py-4 text-lg gap-3 bg-green-600 hover:bg-green-500 text-white font-semibold transition-all shadow-lg"
                  :class="otherDevices.length > 0 ? 'pr-4 rounded-l-xl' : 'pr-8 rounded-xl'"
                  @click.stop="downloadGame"
                >
                  <ArrowDownTrayIcon class="size-6" />
                  Install
                </button>
                <button
                  v-if="otherDevices.length > 0"
                  class="inline-flex items-center px-3 py-4 font-semibold rounded-r-xl transition-all shadow-lg border-l bg-green-600 hover:bg-green-500 text-white border-green-400/30"
                  @click.stop="togglePlayMenu"
                >
                  <ChevronDownIcon class="size-5" :class="{ 'rotate-180': playMenuOpen }" />
                </button>
              </span>
            </div>
            <!-- Dropdown: install on other devices -->
            <Transition name="dropdown-fade">
              <div
                v-if="playMenuOpen && otherDevices.length > 0"
                class="absolute left-0 top-full mt-2 z-50 min-w-[280px] rounded-xl bg-zinc-900 border border-zinc-700/50 shadow-2xl overflow-hidden"
              >
                <button
                  class="flex items-center gap-3 w-full px-6 py-3.5 text-left text-base transition-colors"
                  :class="playMenuFocus === 0 ? 'bg-green-600 text-white' : 'text-zinc-300 hover:bg-zinc-800'"
                  @click="selectInstallMenuAction(0)"
                  @mouseenter="playMenuFocus = 0"
                >
                  <ArrowDownTrayIcon class="size-5" />
                  <span class="font-medium">Install here</span>
                </button>
                <template v-for="(device, i) in otherDevices" :key="'ri-' + device.id">
                  <button
                    class="flex items-center gap-3 w-full px-6 py-3.5 text-left text-base transition-colors"
                    :class="playMenuFocus === 1 + i ? 'bg-green-600 text-white' : 'text-zinc-300 hover:bg-zinc-800'"
                    @click="selectInstallMenuAction(1 + i)"
                    @mouseenter="playMenuFocus = 1 + i"
                  >
                    <ArrowDownTrayIcon class="size-5 text-green-400" />
                    <span class="font-medium">Install on {{ device.name }}</span>
                    <span class="text-xs opacity-50 ml-auto">{{ device.platform }}</span>
                  </button>
                </template>
              </div>
            </Transition>
            <div v-if="playMenuOpen" class="fixed inset-0 z-40" @click="playMenuOpen = false" />
          </div>

          <!-- Add to Library (without installing) — shows for Remote games not yet in library -->
          <button
            v-if="status?.type === 'Remote' && !inLibrary"
            :ref="(el: any) => registerAction(el, { onSelect: addToLibrary })"
            class="inline-flex items-center px-6 py-4 text-lg gap-3 bg-zinc-800/80 hover:bg-zinc-700 text-zinc-300 rounded-xl transition-colors backdrop-blur-sm"
            @click="addToLibrary"
          >
            <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor" class="size-5 text-blue-400">
              <path fill-rule="evenodd" d="M12 3.75a.75.75 0 01.75.75v6.75h6.75a.75.75 0 010 1.5h-6.75v6.75a.75.75 0 01-1.5 0v-6.75H4.5a.75.75 0 010-1.5h6.75V4.5a.75.75 0 01.75-.75z" clip-rule="evenodd" />
            </svg>
            {{ libraryLoading ? "Adding..." : "Add to Library" }}
          </button>
          <span
            v-if="status?.type === 'Remote' && inLibrary"
            class="inline-flex items-center px-4 py-3 text-sm text-zinc-500 gap-2"
          >
            <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor" class="size-4 text-green-500">
              <path fill-rule="evenodd" d="M19.916 4.626a.75.75 0 01.208 1.04l-9 13.5a.75.75 0 01-1.154.114l-6-6a.75.75 0 011.06-1.06l5.353 5.353 8.493-12.739a.75.75 0 011.04-.208z" clip-rule="evenodd" />
            </svg>
            In Library
          </span>

          <!-- Controller, Quality & Widescreen cycle buttons — only for installed emulated games -->
          <template v-if="version && isEmulatedGame && status?.type === 'Installed'">
            <button
              :ref="(el: any) => registerAction(el, { onSelect: cycleController })"
              class="inline-flex items-center gap-1.5 px-4 py-3 text-sm bg-zinc-800/80 hover:bg-zinc-700 text-zinc-300 rounded-xl transition-colors backdrop-blur-sm"
              @click="cycleController"
              :title="`Controller: ${controllerLabel}`"
            >
              <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor" class="size-4 text-blue-400">
                <path d="M17.5 3.5a3.5 3.5 0 00-3.5 3.5 3.5 3.5 0 003.5 3.5A3.5 3.5 0 0021 7a3.5 3.5 0 00-3.5-3.5zm-11 0A3.5 3.5 0 003 7a3.5 3.5 0 003.5 3.5A3.5 3.5 0 0010 7 3.5 3.5 0 006.5 3.5zM12 14c-3.3 0-10 1.7-10 5v2h20v-2c0-3.3-6.7-5-10-5z" />
              </svg>
              <span class="font-medium">{{ controllerLabel }}</span>
            </button>

            <button
              :ref="(el: any) => registerAction(el, { onSelect: cycleQuality })"
              class="inline-flex items-center gap-1.5 px-4 py-3 text-sm bg-zinc-800/80 hover:bg-zinc-700 text-zinc-300 rounded-xl transition-colors backdrop-blur-sm"
              @click="cycleQuality"
              :title="`Quality: ${qualityLabel}`"
            >
              <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor" class="size-4 text-purple-400">
                <path fill-rule="evenodd" d="M9 4.5a.75.75 0 01.721.544l.813 2.846a3.75 3.75 0 002.576 2.576l2.846.813a.75.75 0 010 1.442l-2.846.813a3.75 3.75 0 00-2.576 2.576l-.813 2.846a.75.75 0 01-1.442 0l-.813-2.846a3.75 3.75 0 00-2.576-2.576l-2.846-.813a.75.75 0 010-1.442l2.846-.813A3.75 3.75 0 007.466 7.89l.813-2.846A.75.75 0 019 4.5zM18 1.5a.75.75 0 01.728.568l.258 1.036c.236.94.97 1.674 1.91 1.91l1.036.258a.75.75 0 010 1.456l-1.036.258c-.94.236-1.674.97-1.91 1.91l-.258 1.036a.75.75 0 01-1.456 0l-.258-1.036a2.625 2.625 0 00-1.91-1.91l-1.036-.258a.75.75 0 010-1.456l1.036-.258a2.625 2.625 0 001.91-1.91l.258-1.036A.75.75 0 0118 1.5z" clip-rule="evenodd" />
              </svg>
              <span class="font-medium">{{ qualityLabel }}</span>
            </button>

            <button
              :ref="(el: any) => registerAction(el, { onSelect: toggleWidescreen })"
              class="inline-flex items-center gap-1.5 px-4 py-3 text-sm rounded-xl transition-colors backdrop-blur-sm"
              :class="[
                aspectRatio !== 'Standard'
                  ? 'bg-green-600/80 hover:bg-green-500 text-white'
                  : 'bg-zinc-800/80 hover:bg-zinc-700 text-zinc-300',
              ]"
              @click="toggleWidescreen"
              :title="`Aspect Ratio: ${aspectLabel}`"
            >
              <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="size-4" :class="aspectRatio !== 'Standard' ? 'text-white' : 'text-green-400'">
                <rect x="2" y="5" width="20" height="14" rx="2" />
                <path v-if="aspectRatio !== 'Standard'" d="M7 9l3 3-3 3M13 9h4M13 15h4" />
              </svg>
              <span class="font-medium">{{ aspectLabel }}</span>
            </button>
          </template>

          <!-- Stream status indicator (when streaming is active) -->
          <span
            v-if="isStreaming"
            class="inline-flex items-center gap-2 px-4 py-3 text-sm text-purple-400"
          >
            <span class="size-2 rounded-full bg-purple-400 animate-pulse" />
            {{ pendingRequestSessionId ? 'Waiting for host...' : 'Streaming' }}
          </span>
        </div>
      </div>
    </div>

    <!-- Playtime & Achievement stats bar -->
    <div v-if="game && (gamePlaytime || achievements.length > 0)" class="px-8 pt-4 flex items-center gap-6">
      <!-- Playtime -->
      <div v-if="gamePlaytime" class="flex items-center gap-4">
        <div v-if="gamePlaytime.lastPlayedAt" class="flex items-center gap-1.5">
          <ClockIcon class="size-4 text-zinc-500" />
          <span class="text-sm text-zinc-400">Last played {{ formatTimeAgo(gamePlaytime.lastPlayedAt) }}</span>
        </div>
        <div v-if="gamePlaytime.totalSeconds > 0" class="flex items-center gap-1.5">
          <PlayIcon class="size-4 text-zinc-500" />
          <span class="text-sm text-zinc-400">{{ formatPlaytimeDetailed(gamePlaytime.totalSeconds) }} total</span>
        </div>
      </div>
      <div class="flex-1" />
      <!-- Achievement completion -->
      <div v-if="achievements.length > 0" class="flex items-center gap-2">
        <TrophyIcon class="size-4 text-yellow-500" />
        <span class="text-sm text-zinc-400">{{ unlockedCount }}/{{ achievements.length }}</span>
        <span class="text-xs text-zinc-600">({{ achievementPercent.toFixed(0) }}%)</span>
      </div>
    </div>

    <!-- Content tabs -->
    <div class="px-8 pt-4">
      <div class="relative flex items-center gap-1 border-b border-zinc-800/50">
        <button
          v-for="tab in tabs"
          :key="tab.value"
          :ref="
            (el: any) => {
              registerTabRef(tab.value, el);
              registerTab(el, { onSelect: () => (activeTab = tab.value) });
            }
          "
          class="px-5 py-3 text-sm font-medium transition-colors relative"
          :class="[
            activeTab === tab.value
              ? 'text-blue-400'
              : 'text-zinc-400 hover:text-zinc-200',
          ]"
          @click="activeTab = tab.value"
        >
          {{ tab.label }}
        </button>

        <!-- Animated underline indicator -->
        <div
          class="absolute bottom-0 h-0.5 bg-blue-500 transition-all duration-300 ease-out"
          :style="tabIndicatorStyle"
        />
      </div>
    </div>

    <!-- Tab content -->
    <div class="flex-1 px-8 py-6">
      <!-- Achievements -->
      <div v-if="activeTab === 'achievements'" class="space-y-4">
        <!-- Achievement summary progress + Verify ROM -->
        <div v-if="achievements.length > 0" class="flex items-center gap-3 px-1">
          <div class="flex-1 h-2 bg-zinc-800 rounded-full overflow-hidden">
            <div class="h-full bg-blue-500 rounded-full transition-all" :style="{ width: `${achievementPercent}%` }" />
          </div>
          <span class="text-sm font-medium text-zinc-400 flex-shrink-0">
            {{ unlockedCount }}/{{ achievements.length }}
          </span>
        </div>

        <!-- ROM Hash Status Banners -->
        <div
          v-if="romHashResult?.status === 'Mismatch'"
          class="rounded-lg bg-amber-500/10 p-3 outline outline-1 outline-amber-500/20"
        >
          <p class="text-sm font-medium text-amber-400 mb-1">
            ROM not recognised by RetroAchievements
          </p>
          <p class="text-xs text-zinc-400 mb-2">
            Your ROM hash
            (<code class="text-zinc-300">{{ romHashResult.rom_hash?.slice(0, 12) }}…</code>)
            doesn't match any known hash. Achievements won't track until the ROM is patched or replaced.
          </p>
          <div
            v-if="romHashResult.expected_hashes?.some((h) => h.patchUrl)"
            class="flex flex-wrap gap-2"
          >
            <a
              v-for="h in romHashResult.expected_hashes?.filter((h) => h.patchUrl)"
              :key="h.hash"
              :href="h.patchUrl"
              target="_blank"
              class="inline-flex items-center gap-1 rounded bg-amber-500/20 px-2 py-0.5 text-xs text-amber-300 hover:bg-amber-500/30 transition-colors"
            >
              Patch: {{ h.label || h.hash.slice(0, 8) }}
            </a>
          </div>
        </div>
        <div
          v-else-if="romHashResult?.status === 'Match'"
          class="rounded-lg bg-emerald-500/10 p-2 outline outline-1 outline-emerald-500/20"
        >
          <p class="text-xs text-emerald-400">
            ROM verified — matches RetroAchievements
            <span v-if="romHashResult.matched_label" class="text-zinc-400">
              ({{ romHashResult.matched_label }})
            </span>
          </p>
        </div>
        <div
          v-else-if="romHashResult?.status === 'Error'"
          class="rounded-lg bg-red-500/10 p-2 outline outline-1 outline-red-500/20"
        >
          <p class="text-xs text-red-400">
            Hash check failed: {{ romHashResult.message }}
          </p>
        </div>

        <!-- Achievement items -->
        <div class="space-y-2">
          <div
            v-for="achievement in achievements"
            :key="achievement.id"
            class="flex items-center gap-4 bg-zinc-900/50 rounded-xl p-4"
            :class="{ 'opacity-50': !achievement.unlocked }"
          >
            <img
              v-if="achievement.iconUrl"
              :src="achievement.iconUrl"
              class="size-12 rounded-lg bg-zinc-800"
              referrerpolicy="no-referrer"
              loading="lazy"
              @error="onAchievementIconError"
            />
            <div
              v-if="!achievement.iconUrl"
              class="size-12 rounded-lg bg-zinc-800 flex items-center justify-center"
            >
              <TrophyIcon
                class="size-6"
                :class="achievement.unlocked ? 'text-yellow-400' : 'text-zinc-600'"
              />
            </div>
            <div class="flex-1 min-w-0">
              <p class="text-sm font-medium text-zinc-200">
                {{ achievement.title }}
              </p>
              <p class="text-sm text-zinc-500 truncate">
                {{ achievement.description }}
              </p>
              <!-- Rarity bar -->
              <div v-if="achievement.rarity != null" class="flex items-center gap-2 mt-1.5">
                <div class="flex-1 h-1 bg-zinc-800 rounded-full overflow-hidden">
                  <div
                    class="h-full rounded-full transition-all"
                    :class="rarityColor(achievement.rarity)"
                    :style="{ width: `${Math.max(achievement.rarity, 2)}%` }"
                  />
                </div>
                <span class="text-xs tabular-nums flex-shrink-0" :class="rarityTextColor(achievement.rarity)">
                  {{ achievement.rarity.toFixed(1) }}%
                </span>
              </div>
            </div>
            <TrophyIcon
              v-if="achievement.unlocked"
              class="size-4 text-yellow-400"
            />
          </div>
        </div>

        <p
          v-if="achievements.length === 0"
          class="text-zinc-500 text-center py-8 text-sm"
        >
          No achievements available for this game.
        </p>
      </div>

      <!-- Details (markdown rendered) -->
      <div v-else-if="activeTab === 'details'" class="max-w-3xl">
        <div
          v-if="game?.mDescription"
          class="prose prose-invert prose-zinc max-w-none text-zinc-300 leading-relaxed"
          v-html="renderedDescription"
        />
        <p v-else class="text-zinc-500">No description available.</p>
      </div>

      <!-- Gallery -->
      <div
        v-else-if="activeTab === 'gallery'"
        class="grid grid-cols-2 gap-4"
      >
        <div
          v-for="(imgId, idx) in game?.mImageCarouselObjectIds"
          :key="idx"
          class="aspect-video rounded-lg overflow-hidden bg-zinc-800"
        >
          <img
            :src="objectUrl(imgId)"
            class="w-full h-full object-cover"
            loading="lazy"
          />
        </div>
        <p
          v-if="!game?.mImageCarouselObjectIds?.length"
          class="text-zinc-500 col-span-2 text-center py-8 text-sm"
        >
          No screenshots available.
        </p>
      </div>
      <!-- Saves -->
      <div v-else-if="activeTab === 'saves'" class="space-y-4">
        <!-- Unified save list: merges local + cloud saves -->
        <div v-if="mergedSaves.length > 0" class="space-y-2">
          <div
            v-for="item in mergedSaves"
            :key="item.filename"
            class="flex items-center gap-4 bg-zinc-900/50 rounded-xl p-4"
          >
            <!-- Icon based on file type -->
            <div class="size-10 rounded-lg flex items-center justify-center flex-shrink-0"
              :style="{ backgroundColor: saveTypeColor(item.filename).bg }"
            >
              <svg class="size-5" :style="{ color: saveTypeColor(item.filename).text }" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
                <path v-if="item.filename.endsWith('.srm')" stroke-linecap="round" stroke-linejoin="round" d="M20.25 6.375c0 2.278-3.694 4.125-8.25 4.125S3.75 8.653 3.75 6.375m16.5 0c0-2.278-3.694-4.125-8.25-4.125S3.75 4.097 3.75 6.375m16.5 0v11.25c0 2.278-3.694 4.125-8.25 4.125s-8.25-1.847-8.25-4.125V6.375" />
                <path v-else-if="item.filename.endsWith('.png')" stroke-linecap="round" stroke-linejoin="round" d="M2.25 15.75l5.159-5.159a2.25 2.25 0 013.182 0l5.159 5.159m-1.5-1.5l1.409-1.409a2.25 2.25 0 013.182 0l2.909 2.909M3.75 21h16.5A2.25 2.25 0 0022.5 18.75V5.25A2.25 2.25 0 0020.25 3H3.75A2.25 2.25 0 001.5 5.25v13.5A2.25 2.25 0 003.75 21z" />
                <path v-else stroke-linecap="round" stroke-linejoin="round" d="M3 16.5v2.25A2.25 2.25 0 005.25 21h13.5A2.25 2.25 0 0021 18.75V16.5M16.5 12L12 16.5m0 0L7.5 12m4.5 4.5V3" />
              </svg>
            </div>

            <!-- Info -->
            <div class="flex-1 min-w-0">
              <p class="text-sm font-medium text-zinc-200 truncate">{{ item.filename }}</p>
              <p class="text-xs text-zinc-500">
                {{ saveTypeLabel(item.filename) }}
                <template v-if="item.local">&middot; {{ formatSaveSize(item.local.size) }} &middot; {{ formatTimeAgo(new Date(item.local.modified * 1000).toISOString()) }}</template>
              </p>
              <!-- Sync status badges -->
              <div class="flex gap-2 mt-1">
                <span v-if="item.local" class="inline-flex items-center gap-1 text-[10px] px-1.5 py-0.5 rounded bg-green-900/30 text-green-400">
                  Local
                </span>
                <span v-if="item.cloud" class="inline-flex items-center gap-1 text-[10px] px-1.5 py-0.5 rounded bg-blue-900/30 text-blue-400">
                  Cloud &middot; {{ formatSaveSize(item.cloud.size) }}
                </span>
                <span v-if="!item.local && item.cloud" class="inline-flex items-center gap-1 text-[10px] px-1.5 py-0.5 rounded bg-yellow-900/30 text-yellow-400">
                  Cloud only — download to play
                </span>
              </div>
            </div>

            <!-- Actions -->
            <div class="flex items-center gap-2 flex-shrink-0">
              <button
                v-if="item.local"
                :ref="(el: any) => registerAction(el, { onSelect: () => requestUpload(item.local!) })"
                class="px-3 py-1.5 text-xs rounded-lg transition-colors bg-blue-900/20 text-blue-400 hover:bg-blue-900/30"
                @click="requestUpload(item.local!)"
              >
                {{ cloudSyncStatus[item.filename] === 'uploading' ? 'Syncing...' : (item.cloud ? 'Re-sync' : 'Sync to Cloud') }}
              </button>
              <button
                v-if="item.cloud"
                :ref="(el: any) => registerAction(el, { onSelect: () => requestDownload(item.filename, item.cloud!.saveType) })"
                class="px-3 py-1.5 text-xs rounded-lg transition-colors bg-green-900/20 text-green-400 hover:bg-green-900/30"
                @click="requestDownload(item.filename, item.cloud!.saveType)"
              >
                {{ cloudSyncStatus[item.filename] === 'downloading' ? 'Downloading...' : 'Download' }}
              </button>
              <button
                v-if="item.local"
                :ref="(el: any) => registerAction(el, { onSelect: () => deleteSave(item.local!) })"
                class="px-3 py-1.5 text-xs rounded-lg transition-colors bg-red-900/20 text-red-400 hover:bg-red-900/30"
                @click="deleteSave(item.local!)"
              >
                Delete
              </button>
            </div>
          </div>
        </div>

        <!-- PC Game saves via Ludusavi -->
        <div v-if="pcSaves.length > 0" class="mt-6">
          <div class="flex items-center justify-between mb-3">
            <h4 class="text-sm font-semibold" style="color: var(--bpm-muted)">PC GAME SAVES (via Ludusavi)</h4>
            <div class="flex gap-2">
              <button
                :ref="(el: any) => registerAction(el, { onSelect: backupPcSaves })"
                class="px-3 py-1.5 text-xs rounded-lg transition-colors bg-blue-900/20 text-blue-400 hover:bg-blue-900/30"
                @click="backupPcSaves"
              >
                {{ pcSaveStatus === 'backing-up' ? 'Backing up...' : 'Backup All' }}
              </button>
              <button
                :ref="(el: any) => registerAction(el, { onSelect: restorePcSaves })"
                class="px-3 py-1.5 text-xs rounded-lg transition-colors bg-green-900/20 text-green-400 hover:bg-green-900/30"
                @click="restorePcSaves"
              >
                {{ pcSaveStatus === 'restoring' ? 'Restoring...' : 'Restore' }}
              </button>
            </div>
          </div>
          <!-- Grouped save slots -->
          <div class="space-y-3">
            <div
              v-for="group in pcSaveGroups"
              :key="group.name"
              class="rounded-xl overflow-hidden"
              style="background-color: var(--bpm-surface)"
            >
              <!-- Primary save row — info area + action buttons laid out as a flex row -->
              <div class="flex items-center gap-4 p-4">
                <!-- Save icon + info (focusable for expand toggle) -->
                <div
                  class="flex items-center gap-4 flex-1 min-w-0 cursor-pointer"
                  :ref="(el: any) => registerAction(el, { onSelect: () => { if (group.backups.length > 0) group.expanded = !group.expanded; } })"
                  @click="() => { if (group.backups.length > 0) group.expanded = !group.expanded; }"
                >
                  <div class="size-10 rounded-lg flex items-center justify-center flex-shrink-0"
                    :style="{ backgroundColor: group.type === 'settings' ? 'rgba(156,163,175,0.15)' : 'rgba(34,197,94,0.15)' }"
                  >
                    <svg v-if="group.type === 'settings'" class="size-5 text-zinc-400" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
                      <path stroke-linecap="round" stroke-linejoin="round" d="M9.594 3.94c.09-.542.56-.94 1.11-.94h2.593c.55 0 1.02.398 1.11.94l.213 1.281c.063.374.313.686.645.87.074.04.147.083.22.127.324.196.72.257 1.075.124l1.217-.456a1.125 1.125 0 011.37.49l1.296 2.247a1.125 1.125 0 01-.26 1.431l-1.003.827c-.293.24-.438.613-.431.992a6.759 6.759 0 010 .255c-.007.378.138.75.43.99l1.005.828c.424.35.534.954.26 1.43l-1.298 2.247a1.125 1.125 0 01-1.369.491l-1.217-.456c-.355-.133-.75-.072-1.076.124a6.57 6.57 0 01-.22.128c-.331.183-.581.495-.644.869l-.213 1.28c-.09.543-.56.941-1.11.941h-2.594c-.55 0-1.02-.398-1.11-.94l-.213-1.281c-.062-.374-.312-.686-.644-.87a6.52 6.52 0 01-.22-.127c-.325-.196-.72-.257-1.076-.124l-1.217.456a1.125 1.125 0 01-1.369-.49l-1.297-2.247a1.125 1.125 0 01.26-1.431l1.004-.827c.292-.24.437-.613.43-.992a6.932 6.932 0 010-.255c.007-.378-.138-.75-.43-.99l-1.004-.828a1.125 1.125 0 01-.26-1.43l1.297-2.247a1.125 1.125 0 011.37-.491l1.216.456c.356.133.751.072 1.076-.124.072-.044.146-.087.22-.128.332-.183.582-.495.644-.869l.214-1.281z" />
                      <path stroke-linecap="round" stroke-linejoin="round" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
                    </svg>
                    <svg v-else class="size-5 text-green-400" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
                      <path stroke-linecap="round" stroke-linejoin="round" d="M20.25 6.375c0 2.278-3.694 4.125-8.25 4.125S3.75 8.653 3.75 6.375m16.5 0c0-2.278-3.694-4.125-8.25-4.125S3.75 4.097 3.75 6.375m16.5 0v11.25c0 2.278-3.694 4.125-8.25 4.125s-8.25-1.847-8.25-4.125V6.375" />
                    </svg>
                  </div>
                  <div class="flex-1 min-w-0">
                    <p class="text-sm font-medium text-zinc-200">{{ group.label }}</p>
                    <p class="text-xs text-zinc-500">
                      {{ group.primary ? pcSaveFileName(group.primary.path) : group.name }}
                      <template v-if="group.primary && group.primary.size > 0"> &middot; {{ formatSaveSize(group.primary.size) }}</template>
                      <template v-if="group.backups.length > 0"> &middot; {{ group.backups.length }} backup{{ group.backups.length !== 1 ? 's' : '' }}</template>
                    </p>
                  </div>
                </div>
                <!-- Action buttons — each independently focusable -->
                <div class="flex items-center gap-2 flex-shrink-0">
                  <span v-if="pcCloudStatus[group.name]" class="text-xs px-2 py-0.5 rounded-full"
                    :class="pcCloudStatus[group.name] === 'synced' ? 'bg-green-900/20 text-green-400' : pcCloudStatus[group.name] === 'cloud-only' ? 'bg-blue-900/20 text-blue-400' : 'bg-zinc-800 text-zinc-500'"
                  >
                    {{ pcCloudStatus[group.name] === 'synced' ? 'Synced' : pcCloudStatus[group.name] === 'cloud-only' ? 'Cloud' : pcCloudStatus[group.name] === 'cloud-newer' ? 'Cloud newer' : 'Local newer' }}
                  </span>
                  <button
                    v-if="group.primary && pcSyncStatus[group.name] !== 'uploading'"
                    :ref="(el: any) => registerAction(el, { onSelect: () => uploadPcSave(group) })"
                    class="px-3 py-1.5 text-xs rounded-lg transition-colors bg-blue-900/20 text-blue-400 hover:bg-blue-900/30"
                    @click.stop="uploadPcSave(group)"
                  >
                    Upload
                  </button>
                  <span v-else-if="pcSyncStatus[group.name] === 'uploading'" class="text-xs text-blue-400 animate-pulse">Uploading...</span>
                  <button
                    v-if="hasPcCloudSave(group.name) && pcSyncStatus[group.name] !== 'downloading'"
                    :ref="(el: any) => registerAction(el, { onSelect: () => downloadPcSave(group) })"
                    class="px-3 py-1.5 text-xs rounded-lg transition-colors bg-green-900/20 text-green-400 hover:bg-green-900/30"
                    @click.stop="downloadPcSave(group)"
                  >
                    Download
                  </button>
                  <span v-else-if="pcSyncStatus[group.name] === 'downloading'" class="text-xs text-green-400 animate-pulse">Downloading...</span>
                </div>
              </div>

              <!-- Expandable backups -->
              <div v-if="group.expanded && group.backups.length > 0" class="border-t px-4 pb-3 pt-2 space-y-1" style="border-color: var(--bpm-border)">
                <div
                  v-for="backup in group.backups"
                  :key="backup.path"
                  class="flex items-center gap-3 py-1.5 pl-14"
                >
                  <div class="size-5 rounded flex items-center justify-center flex-shrink-0" style="background-color: rgba(234,179,8,0.1)">
                    <svg class="size-3 text-yellow-500" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
                      <path stroke-linecap="round" stroke-linejoin="round" d="M19.5 14.25v-2.625a3.375 3.375 0 00-3.375-3.375h-1.5A1.125 1.125 0 0113.5 7.125v-1.5a3.375 3.375 0 00-3.375-3.375H8.25m2.25 0H5.625c-.621 0-1.125.504-1.125 1.125v17.25c0 .621.504 1.125 1.125 1.125h12.75c.621 0 1.125-.504 1.125-1.125V11.25a9 9 0 00-9-9z" />
                    </svg>
                  </div>
                  <span class="text-xs text-zinc-400 truncate">{{ pcSaveFileName(backup.path) }}</span>
                  <span v-if="backup.size > 0" class="text-xs text-zinc-600 flex-shrink-0">{{ formatSaveSize(backup.size) }}</span>
                </div>
              </div>
            </div>
          </div>
        </div>

        <div v-if="!ludusaviAvailable && isNativeGame" class="mt-4 p-4 rounded-xl" style="background-color: var(--bpm-surface)">
          <div class="flex items-center justify-between">
            <div>
              <p class="text-sm font-medium" style="color: var(--bpm-text)">PC Save Management</p>
              <p class="text-xs mt-0.5" style="color: var(--bpm-muted)">
                Drop uses Ludusavi to detect and back up PC game saves. Install it to enable save management for this game.
              </p>
            </div>
            <button
              :ref="(el: any) => registerAction(el, { onSelect: doInstallLudusavi })"
              class="px-4 py-2 text-sm font-medium rounded-lg flex-shrink-0 ml-4"
              :style="{ backgroundColor: 'var(--bpm-accent-hex)', color: 'var(--bpm-accent-text)' }"
              :disabled="ludusaviInstalling"
              @click="doInstallLudusavi"
            >
              {{ ludusaviInstalling ? 'Installing...' : 'Install Ludusavi' }}
            </button>
          </div>
        </div>

        <p v-if="mergedSaves.length === 0 && pcSaves.length === 0 && !savesLoading" class="text-zinc-500 text-center py-8 text-sm">
          <template v-if="isNativeGame && !ludusaviAvailable">
            <!-- Ludusavi prompt handles this case above -->
          </template>
          <template v-else-if="isNativeGame && ludusaviAvailable">
            No saves detected by Ludusavi for this game.
          </template>
          <template v-else>
            No save data found for this game. Play the game to create saves.
          </template>
        </p>
        <p v-if="savesLoading" class="text-zinc-500 text-center py-8 text-sm">
          Loading saves...
        </p>

        <!-- Dev: Test Conflict Dialog -->
        <div class="mt-6 pt-4 border-t border-zinc-800/30">
          <button
            :ref="(el: any) => registerAction(el, { onSelect: triggerTestConflict })"
            class="px-3 py-1.5 text-xs rounded-lg transition-colors bg-amber-900/20 text-amber-400 border border-amber-500/30 hover:bg-amber-900/30"
            @click="triggerTestConflict"
          >
            Test Conflict Dialog
          </button>
        </div>
      </div>
    </div>

    <!-- Recommended games -->
    <div v-if="recommendedGames.length > 0" class="px-8 pb-6">
      <h3 class="text-sm font-semibold mb-3" style="color: var(--bpm-muted)">YOU MIGHT ALSO LIKE</h3>
      <div class="flex gap-4 overflow-x-auto pb-2" style="scrollbar-width: thin">
        <div
          v-for="rec in recommendedGames"
          :key="rec.id"
          class="flex-shrink-0 cursor-pointer bp-focus-delegate"
          style="width: 9rem"
          :ref="(el: any) => registerAction(el, { onSelect: () => $router.push(`/bigpicture/library/${rec.id}`) })"
        >
          <div class="bp-focus-ring rounded-lg overflow-hidden transition-transform hover:scale-105" style="aspect-ratio: 3/4">
            <img v-if="rec.mCoverObjectId" :src="objectUrl(rec.mCoverObjectId)" class="w-full h-full object-cover" loading="lazy" />
            <div v-else class="w-full h-full flex items-center justify-center bg-zinc-800 text-zinc-500 text-lg font-bold">{{ rec.mName[0] }}</div>
          </div>
          <p class="text-xs mt-1.5 truncate" style="color: var(--bpm-text)">{{ rec.mName }}</p>
        </div>
      </div>
    </div>

    <!-- Settings toast -->
    <Transition
      enter-active-class="transition-all duration-200"
      leave-active-class="transition-all duration-300"
      enter-from-class="opacity-0 translate-y-4"
      leave-to-class="opacity-0 translate-y-4"
    >
      <div
        v-if="settingsToast"
        class="fixed bottom-8 left-1/2 -translate-x-1/2 z-[200] px-6 py-3 rounded-xl text-sm font-medium shadow-lg backdrop-blur-md"
        style="background-color: rgba(var(--bpm-accent, 59 130 246) / 0.9); color: var(--bpm-accent-text, #fff)"
      >
        {{ settingsToast }}
        <span class="text-xs opacity-70 ml-2">Applied on next launch</span>
      </div>
    </Transition>

    <!-- Launch error dialog -->
    <BigPictureDialog
      :visible="launchError !== null"
      title="Launch Failed"
      :message="launchError || ''"
      confirm-label="Dismiss"
      :show-cancel="false"
      @confirm="dismissLaunchError"
    />

    <!-- Uninstall confirmation dialog -->
    <BigPictureDialog
      :visible="confirmUninstall"
      title="Uninstall Game"
      :message="`Are you sure you want to uninstall ${game?.mName ?? 'this game'}? This will delete all local game files.`"
      confirm-label="Uninstall"
      cancel-label="Cancel"
      :destructive="true"
      @confirm="doUninstall"
      @cancel="confirmUninstall = false"
    />

    <!-- Remove from library confirmation dialog -->
    <BigPictureDialog
      :visible="confirmRemoveFromLibrary"
      title="Remove from Library"
      :message="`Are you sure you want to remove ${game?.mName ?? 'this game'} from your library?`"
      confirm-label="Remove"
      cancel-label="Cancel"
      :destructive="true"
      @confirm="doRemoveFromLibrary"
      @cancel="confirmRemoveFromLibrary = false"
    />

    <!-- Cloud sync confirmation -->
    <BigPictureDialog
      :visible="confirmSyncAction !== null"
      :title="confirmSyncAction?.type === 'upload' ? 'Replace Cloud Save?' : 'Replace Local Save?'"
      :message="confirmSyncAction?.type === 'upload'
        ? `This will replace the cloud version of '${confirmSyncAction?.filename}' with your local copy. A backup of the current cloud version will be saved automatically.`
        : `This will replace your local copy of '${confirmSyncAction?.filename}' with the cloud version. A backup of your current local save will be created automatically.`"
      :confirm-label="confirmSyncAction?.type === 'upload' ? 'Replace Cloud Save' : 'Replace Local Save'"
      cancel-label="Cancel"
      :destructive="false"
      @confirm="confirmSync"
      @cancel="confirmSyncAction = null"
    />

    <!-- On-screen keyboard for creating new shelf -->
    <BigPictureKeyboard
      :visible="showNewShelfKeyboard"
      :model-value="newShelfNameInPicker"
      placeholder="Enter shelf name..."
      @update:model-value="newShelfNameInPicker = $event"
      @close="showNewShelfKeyboard = false"
      @submit="showNewShelfKeyboard = false; createShelfAndAdd()"
    />

    <!-- Shelf picker overlay -->
    <Teleport to="body">
      <Transition
        enter-active-class="transition-opacity duration-200"
        enter-from-class="opacity-0"
        enter-to-class="opacity-100"
        leave-active-class="transition-opacity duration-150"
        leave-from-class="opacity-100"
        leave-to-class="opacity-0"
      >
        <div
          v-if="showShelfPicker"
          class="fixed inset-0 z-50 flex items-center justify-center bg-black/70 backdrop-blur-sm"
          @click.self="showShelfPicker = false"
        >
          <div
            class="rounded-2xl shadow-2xl p-6 w-full max-w-md mx-4"
            style="background-color: var(--bpm-surface); color: var(--bpm-text)"
          >
            <h2 class="text-lg font-semibold font-display mb-4">Add to Shelf</h2>

            <!-- Existing shelves as checkboxes -->
            <div v-if="shelvesData.shelves.value.length > 0" class="space-y-2 mb-4">
              <button
                v-for="(shelf, sIdx) in shelvesData.shelves.value"
                :key="shelf.id"
                class="w-full flex items-center gap-3 px-4 py-3 rounded-xl text-left transition-all text-sm"
                :style="{
                  backgroundColor: shelfFocusIdx === sIdx ? 'var(--bpm-accent-hex)' : 'var(--bpm-bg)',
                  color: shelfFocusIdx === sIdx ? 'var(--bpm-accent-text)' : 'var(--bpm-text)',
                }"
                @click="toggleGameOnShelf(shelf.id)"
              >
                <div
                  class="size-5 rounded border-2 flex items-center justify-center flex-shrink-0 transition-colors"
                  :style="{
                    borderColor: shelf.entries.some(e => e.gameId === gameId) ? 'var(--bpm-accent-hex)' : 'var(--bpm-muted)',
                    backgroundColor: shelf.entries.some(e => e.gameId === gameId) ? 'var(--bpm-accent-hex)' : 'transparent',
                  }"
                >
                  <svg v-if="shelf.entries.some(e => e.gameId === gameId)" class="size-3 text-white" fill="none" stroke="currentColor" stroke-width="3" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" d="M4.5 12.75l6 6 9-13.5" />
                  </svg>
                </div>
                <span>{{ shelf.name }}</span>
                <span class="ml-auto text-xs" style="color: var(--bpm-muted)">{{ shelf.entries.length }}</span>
              </button>
            </div>

            <!-- Create new shelf -->
            <button
              class="w-full flex items-center gap-3 px-4 py-3 rounded-xl text-left text-sm mb-4"
              :style="{
                backgroundColor: shelfFocusIdx === shelvesData.shelves.value.length ? 'var(--bpm-accent-hex)' : 'var(--bpm-bg)',
                color: shelfFocusIdx === shelvesData.shelves.value.length ? 'var(--bpm-accent-text)' : 'var(--bpm-muted)',
              }"
              @click="showNewShelfKeyboard = true"
            >
              <span>+ Create New Shelf</span>
            </button>

            <button
              class="w-full py-2.5 text-sm font-medium rounded-xl transition-colors"
              :style="{
                backgroundColor: shelfFocusIdx === shelvesData.shelves.value.length + 1 ? 'var(--bpm-accent-hex)' : 'var(--bpm-bg)',
                color: shelfFocusIdx === shelvesData.shelves.value.length + 1 ? 'var(--bpm-accent-text)' : 'var(--bpm-muted)',
              }"
              @click="showShelfPicker = false"
            >
              Done
            </button>
          </div>
        </div>
      </Transition>
    </Teleport>

    <!-- Options menu overlay — fully gamepad-navigable -->
    <Teleport to="body">
      <Transition
        enter-active-class="transition-opacity duration-200"
        leave-active-class="transition-opacity duration-200"
        enter-from-class="opacity-0"
        leave-to-class="opacity-0"
      >
        <div
          v-if="showOptions"
          class="fixed inset-0 z-50 flex items-center justify-center bg-black/70 backdrop-blur-sm"
        >
          <div class="bg-zinc-900 border border-zinc-700/50 rounded-2xl shadow-2xl p-6 max-w-md w-full mx-4">
            <h2 class="text-xl font-semibold font-display text-zinc-100 mb-4">Game Options</h2>

            <div class="space-y-1.5">
              <button
                v-for="(item, idx) in optionsMenuItems"
                :key="item.id"
                class="w-full flex items-center justify-between px-4 py-3 rounded-xl text-sm transition-colors"
                :class="optionsFocusIdx === idx
                  ? 'bg-blue-600 text-white shadow-lg shadow-blue-600/20'
                  : 'bg-zinc-800/50 text-zinc-300 hover:bg-zinc-700'"
                @click="item.action()"
              >
                <span class="font-medium">{{ item.label }}</span>
                <span v-if="item.valueLabel" class="text-xs opacity-75">{{ item.valueLabel }}</span>
              </button>
            </div>

            <!-- Hints -->
            <div class="flex gap-6 mt-4 text-xs text-zinc-500 justify-end">
              <BigPictureButtonPrompt button="A" label="Select" size="sm" />
              <BigPictureButtonPrompt button="B" label="Close" size="sm" />
            </div>
          </div>
        </div>
      </Transition>
    </Teleport>

    <!-- Cloud Save Conflict Resolution Dialog -->
    <BpmSaveConflictDialog
      :visible="saveConflictVisible"
      :game-id="gameId"
      :conflicts="saveConflicts"
      @resolved="saveConflictVisible = false"
    />
  </div>
</template>

<script setup lang="ts">
import BpmSaveConflictDialog from "~/components/bigpicture/BpmSaveConflictDialog.vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  PlayIcon,
  StopIcon,
  ArrowDownTrayIcon,
  TrophyIcon,
  SignalIcon,
} from "@heroicons/vue/24/solid";
import { ChevronDownIcon } from "@heroicons/vue/20/solid";
import { ClockIcon } from "@heroicons/vue/24/outline";
import BigPictureDialog from "~/components/bigpicture/BigPictureDialog.vue";
import BigPictureButtonPrompt from "~/components/bigpicture/BigPictureButtonPrompt.vue";
import BigPictureKeyboard from "~/components/bigpicture/BigPictureKeyboard.vue";
import {
  useGame,
  type LaunchResult,
  type VersionOption,
} from "~/composables/game";
import { serverUrl } from "~/composables/use-server-fetch";
import type {
  AspectRatio,
  ControllerType,
  QualityPreset,
  Game,
  GameStatus,
  GameVersion,
} from "~/types";

function objectUrl(id: string): string {
  return serverUrl(`api/v1/object/${id}`);
}

import { useBpFocusableGroup } from "~/composables/bp-focusable";
import { useFocusNavigation } from "~/composables/focus-navigation";
import { GamepadButton, useGamepad } from "~/composables/gamepad";
import { useStreaming } from "~/composables/useStreaming";

definePageMeta({ layout: "bigpicture" });

console.log("[BPM:GAME] >>> Script setup executing (synchronous) <<<");

const route = useRoute();
const gameId = route.params.id as string;
console.log(`[BPM:GAME] Route param gameId: ${gameId}`);

const game = ref<Game | null>(null);
const statusRef = shallowRef<any>(null);
const status = computed<GameStatus | null>(() => statusRef.value?.value ?? null);
const version = ref<GameVersion | null>(null);
const versionOptions = ref<VersionOption[] | null>(null);
const activeTab = ref("achievements");
// Plain object — NOT reactive. Storing DOM refs in a reactive ref causes
// infinite update loops when set from :ref callbacks during render.
const tabRefs: Record<string, HTMLElement | null> = {};
const tabIndicatorStyle = ref({ left: "0", width: "0" });
const launchError = ref<string | null>(null);
const diagnosticsRan = ref(false);
const isStreaming = ref(false);

// ── Streaming ─────────────────────────────────────────────────────────────
const {
  checkSunshine,
  startSunshine,
  startStreamingSession,
  markSessionReady,
  registerGame,
  stopStreamingSession,
  sendHeartbeat,
  listRemoteSessions,
  getConnectionInfo,
  sendPin,
  requestStream,
  listDevices,
  remoteInstall,
} = useStreaming();

import type { ClientDevice } from "~/composables/useStreaming";

let activeStreamSessionId: string | null = null;
let heartbeatInterval: ReturnType<typeof setInterval> | null = null;

// ── Remote stream discovery (client/receiver side) ────────────────────────
const availableStream = ref<any>(null);
let streamPollInterval: ReturnType<typeof setInterval> | null = null;
// Session ID of a stream we requested (waiting for host to fulfill)
const pendingRequestSessionId = ref<string | null>(null);

async function pollRemoteSessions() {
  try {
    const sessions = await listRemoteSessions();
    // Find an active session for this game (Ready/Starting/Streaming)
    const found = sessions.find(
      (s: any) =>
        s.game?.id === gameId &&
        (s.status === "Ready" || s.status === "Starting" || s.status === "Streaming"),
    ) ?? null;
    availableStream.value = found;

    // If we have a pending request and a session just became Ready, auto-connect
    if (pendingRequestSessionId.value && found && found.status === "Ready") {
      console.log("[BPM:STREAM] Our requested session is now Ready! Auto-connecting...");
      pendingRequestSessionId.value = null;
      isStreaming.value = false;
      await connectToRemoteStream();
    }
  } catch {
    // Silently ignore poll errors
  }
}

async function connectToRemoteStream() {
  if (!availableStream.value) return;
  try {
    const info = await getConnectionInfo(availableStream.value.id);
    console.log("[BPM:STREAM] Connection info:", JSON.stringify(info));
    const host = info.hostLocalIp || info.hostExternalIp;
    if (!host) {
      launchError.value = "No host IP available for streaming";
      return;
    }
    // Launch Moonlight pointed at the host
    const port = info.sunshinePort || 47989;
    console.log(`[BPM:STREAM] Launching Moonlight → ${host}:${port}`);
    await invoke("launch_moonlight", { host, port, pin: info.pairingPin ?? null });
    streamGuard = false;
    // Restore normal poll interval
    if (streamPollInterval) clearInterval(streamPollInterval);
    streamPollInterval = setInterval(pollRemoteSessions, 15_000);
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    console.error("[BPM:STREAM] Failed to connect to stream:", msg);
    launchError.value = `Stream connect failed: ${msg}`;
    streamGuard = false;
  }
}

// ── Device list & action menu ────────────────────────────────────────────
const devices = ref<ClientDevice[]>([]);
const otherDevices = computed(() => devices.value.filter((d) => !d.isSelf));

async function loadDevices() {
  try {
    devices.value = await listDevices();
  } catch {
    devices.value = [];
  }
}

const playMenuOpen = ref(false);
const playMenuFocus = ref(0);
let playMenuLockId = 0;

// Total items in the installed-game dropdown:
// [0] Play, [1..N] Stream to device, [N+1..2N] Install on device
const playMenuItemCount = computed(() => 1 + otherDevices.value.length * 2);

// Total items in the not-installed dropdown:
// [0] Install here, [1..N] Install on device
const installMenuItemCount = computed(() => 1 + otherDevices.value.length);

function openPlayMenu() {
  playMenuOpen.value = true;
  playMenuFocus.value = 0;
  playMenuLockId = focusNav.acquireInputLock();
  wirePlayMenuGamepad();
}

function togglePlayMenu() {
  if (playMenuOpen.value) {
    closePlayMenu();
  } else {
    openPlayMenu();
  }
}

function closePlayMenu() {
  playMenuOpen.value = false;
  unwirePlayMenuGamepad();
  focusNav.releaseInputLock(playMenuLockId);
}

function selectPlayMenuAction(index: number) {
  closePlayMenu();
  if (index === 0) {
    // Play locally
    launchGame();
  } else if (index <= otherDevices.value.length) {
    // Stream to device at index-1
    const device = otherDevices.value[index - 1];
    streamToDevice(device);
  } else {
    // Install on device
    const deviceIdx = index - 1 - otherDevices.value.length;
    const device = otherDevices.value[deviceIdx];
    installOnDevice(device);
  }
}

function selectInstallMenuAction(index: number) {
  closePlayMenu();
  if (index === 0) {
    downloadGame();
  } else {
    const device = otherDevices.value[index - 1];
    installOnDevice(device);
  }
}

async function streamToDevice(device: ClientDevice) {
  console.log(`[BPM:STREAM] Stream to device: ${device.name} (${device.id})`);
  streamGame();
}

async function installOnDevice(device: ClientDevice) {
  console.log(`[BPM:STREAM] Remote install on device: ${device.name} (${device.id})`);
  try {
    await remoteInstall(gameId, device.id);
    launchError.value = null;
    // Show brief confirmation
    console.log(`[BPM:STREAM] Remote install requested on ${device.name}`);
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    launchError.value = `Remote install failed: ${msg}`;
  }
}

const _playMenuUnsubs: (() => void)[] = [];
function wirePlayMenuGamepad() {
  unwirePlayMenuGamepad();
  const maxIdx = status.value?.type === "Installed"
    ? playMenuItemCount.value - 1
    : installMenuItemCount.value - 1;
  _playMenuUnsubs.push(
    gamepad.onButton(GamepadButton.DPadUp, () => {
      if (playMenuOpen.value) playMenuFocus.value = Math.max(0, playMenuFocus.value - 1);
    }),
    gamepad.onButton(GamepadButton.DPadDown, () => {
      if (playMenuOpen.value) playMenuFocus.value = Math.min(maxIdx, playMenuFocus.value + 1);
    }),
    gamepad.onButton(GamepadButton.South, () => {
      if (playMenuOpen.value) {
        if (status.value?.type === "Installed") {
          selectPlayMenuAction(playMenuFocus.value);
        } else {
          selectInstallMenuAction(playMenuFocus.value);
        }
      }
    }),
    gamepad.onButton(GamepadButton.East, () => { if (playMenuOpen.value) closePlayMenu(); }),
  );
}
function unwirePlayMenuGamepad() {
  for (const u of _playMenuUnsubs) u();
  _playMenuUnsubs.length = 0;
}

let streamGuard = false;

/**
 * Request a stream from another device (push-based flow).
 * 1. Creates a "Requested" session on the server
 * 2. Polls faster (every 3s) waiting for a host to pick it up
 * 3. When the session becomes "Ready", auto-launches Moonlight
 */
async function streamGame() {
  console.log("[BPM:STREAM] streamGame() called — requesting remote stream");
  if (launchGuard || streamGuard) return;
  streamGuard = true;
  isStreaming.value = true;
  try {
    // Request a stream from another device
    console.log("[BPM:STREAM] Sending stream request for gameId:", gameId);
    const sessionId = await requestStream(gameId);
    pendingRequestSessionId.value = sessionId;
    console.log("[BPM:STREAM] Stream requested, session:", sessionId);

    // Speed up polling while waiting for the host to accept
    if (streamPollInterval) clearInterval(streamPollInterval);
    streamPollInterval = setInterval(pollRemoteSessions, 3_000);

    // Set a timeout — if no host picks it up within 60 seconds, give up
    setTimeout(() => {
      if (pendingRequestSessionId.value === sessionId) {
        console.warn("[BPM:STREAM] Stream request timed out — no host responded");
        pendingRequestSessionId.value = null;
        isStreaming.value = false;
        streamGuard = false;
        launchError.value = "No host responded to the stream request. Make sure Drop is running on your PC.";
        // Restore normal poll interval
        if (streamPollInterval) clearInterval(streamPollInterval);
        streamPollInterval = setInterval(pollRemoteSessions, 15_000);
      }
    }, 60_000);
  } catch (e) {
    const errMsg = e instanceof Error ? e.message : String(e);
    console.error("[BPM:STREAM] Stream request failed:", errMsg);
    launchError.value = `Stream request failed: ${errMsg}`;
    isStreaming.value = false;
    streamGuard = false;
  }
}

/** Run launch diagnostics and log to console for debug capture */
async function runDiagnostics() {
  if (diagnosticsRan.value) return;
  diagnosticsRan.value = true;
  try {
    const diag = await invoke("diagnose_launch_environment");
    console.log("[BPM:DIAG] === LAUNCH DIAGNOSTICS ===");
    console.log("[BPM:DIAG] UMU installed:", (diag as any).umu_installed, "path:", (diag as any).umu_path);
    console.log("[BPM:DIAG] Proton default:", (diag as any).proton_default, "valid:", (diag as any).proton_default_valid);
    console.log("[BPM:DIAG] Proton autodiscovered:", (diag as any).proton_autodiscovered);
    console.log("[BPM:DIAG] Session:", (diag as any).session_type, "gamescope:", (diag as any).gamescope_detected);
    console.log("[BPM:DIAG] Env:", { display: (diag as any).env_display, wayland: (diag as any).env_wayland, gamescope: (diag as any).env_gamescope, xdg: (diag as any).env_xdg_runtime });
    console.log("[BPM:DIAG] Installed games:", (diag as any).installed_games);
    console.log("[BPM:DIAG] === END DIAGNOSTICS ===");
  } catch (e) {
    console.warn("[BPM:DIAG] Diagnostics not available:", e);
  }
}
const showOptions = ref(false);
const optionsFocusIdx = ref(0);
let optionsLockId = 0;

const focusNav = useFocusNavigation();
const registerAction = useBpFocusableGroup("content");
const registerTab = useBpFocusableGroup("content");

const gamepad = useGamepad();
const _unsubs: (() => void)[] = [];

// ── Markdown rendering (H fix) ──────────────────────────────────────────
const renderedDescription = computed(() => {
  if (!game.value?.mDescription) return "";
  return renderMarkdown(game.value.mDescription);
});

/**
 * Lightweight markdown → HTML renderer for game descriptions.
 * Handles: headings, bold, italic, links, lists, paragraphs.
 */
function renderMarkdown(md: string): string {
  let html = md
    // Escape HTML entities
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    // Headings (## heading)
    .replace(/^### (.+)$/gm, "<h3>$1</h3>")
    .replace(/^## (.+)$/gm, "<h2>$1</h2>")
    .replace(/^# (.+)$/gm, "<h1>$1</h1>")
    // Bold (**text** or __text__)
    .replace(/\*\*(.+?)\*\*/g, "<strong>$1</strong>")
    .replace(/__(.+?)__/g, "<strong>$1</strong>")
    // Italic (*text* or _text_)
    .replace(/\*(.+?)\*/g, "<em>$1</em>")
    .replace(/_(.+?)_/g, "<em>$1</em>")
    // Images ![alt](url) — must come before links to avoid ![...] matching [...]
    .replace(
      /!\[([^\]]*)\]\(([^)]+)\)/g,
      (_m: string, alt: string, url: string) => {
        // Rewrite relative URLs (starting with /) through the server proxy
        const src = url.startsWith("/") ? serverUrl(url.slice(1)) : url;
        return `<img src="${src}" alt="${alt}" class="rounded-lg max-w-full my-2" loading="lazy" />`;
      },
    )
    // Links [text](url)
    .replace(
      /\[([^\]]+)\]\(([^)]+)\)/g,
      '<a href="$2" target="_blank" rel="noopener" class="text-blue-400 hover:underline">$1</a>',
    )
    // Unordered lists (- item or * item)
    .replace(/^[\-\*] (.+)$/gm, "<li>$1</li>")
    // Horizontal rules (--- or ***)
    .replace(/^[\-\*]{3,}$/gm, '<hr class="border-zinc-700 my-4">')
    // Paragraphs (double newlines)
    .replace(/\n\n+/g, "</p><p>")
    // Single newlines → line breaks
    .replace(/\n/g, "<br>");

  // Wrap list items in <ul>
  html = html.replace(
    /(<li>.*?<\/li>)+/gs,
    '<ul class="list-disc pl-6 space-y-1">$&</ul>',
  );

  return `<p>${html}</p>`;
}

// ── Game type detection ─────────────────────────────────────────────────
const isEmulatedGame = computed(() => {
  const ver = version.value;
  if (!ver?.launches) return false;
  // Check if ALL launches use an emulator. If any launch is native (no emulator),
  // this is a native game — the emulator buttons shouldn't show.
  return ver.launches.length > 0 && ver.launches.every((l) => l.emulator != null);
});
const isNativeGame = computed(() => !isEmulatedGame.value);
const isWindowsGame = computed(() => {
  // Check launch configs from the loaded version data first
  const ver = version.value;
  if (ver?.launches?.some((l) => l.platform?.toLowerCase() === "windows")) {
    return true;
  }
  // Fallback to version options (loaded async for install/launch UI)
  return versionOptions.value?.some((v) => v.platform?.toLowerCase() === "windows") ?? false;
});

// ── Controller & Quality presets ─────────────────────────────────────────
const controllerOptions: { label: string; value: ControllerType | null }[] = [
  { label: "Auto", value: null },
  { label: "Xbox (A=South)", value: "Xbox" },
  { label: "Nintendo (A=East)", value: "Nintendo" },
];
const qualityOptions: { label: string; value: QualityPreset | null }[] = [
  { label: "Auto", value: null },
  { label: "Low", value: "Low" },
  { label: "Med", value: "Medium" },
  { label: "High", value: "High" },
  { label: "Ultra", value: "Ultra" },
];

const selectedController = ref<ControllerType | null>(null);
const selectedQuality = ref<QualityPreset | null>(null);
const aspectRatio = ref<AspectRatio>("Standard");
const crtShaderEnabled = ref(false);

const settingsToast = ref("");
let toastTimer: ReturnType<typeof setTimeout> | null = null;

function showSettingsToast(msg: string) {
  settingsToast.value = msg;
  if (toastTimer) clearTimeout(toastTimer);
  toastTimer = setTimeout(() => { settingsToast.value = ""; }, 2000);
}

async function saveUserConfig() {
  const ver = version.value;
  if (!ver) return;
  try {
    const currentConfig = ver.userConfiguration ?? {
      launchTemplate: "{}",
      overrideProtonPath: null,
      enableUpdates: false,
    };
    const config = {
      ...currentConfig,
      controllerType: selectedController.value,
      qualityPreset: selectedQuality.value,
      widescreen: aspectRatio.value,
      crtShader: crtShaderEnabled.value,
    };
    await invoke("update_game_configuration", {
      gameId: gameId,
      options: config,
    });
  } catch (e) {
    console.error("Failed to save config:", e);
    launchError.value = `Failed to save settings: ${e instanceof Error ? e.message : String(e)}`;
  }
}

function setController(value: ControllerType | null) {
  selectedController.value = value;
  saveUserConfig();
  const label = controllerOptions.find((o) => o.value === value)?.label ?? "Auto";
  showSettingsToast(`Controller: ${label}`);
}

function setQuality(value: QualityPreset | null) {
  selectedQuality.value = value;
  saveUserConfig();
  const label = qualityOptions.find((o) => o.value === value)?.label ?? "Auto";
  showSettingsToast(`Quality: ${label}`);
}

function cycleController() {
  const values = controllerOptions.map((o) => o.value);
  const idx = values.indexOf(selectedController.value);
  const next = values[(idx + 1) % values.length];
  setController(next);
}

function cycleQuality() {
  const values = qualityOptions.map((o) => o.value);
  const idx = values.indexOf(selectedQuality.value);
  const next = values[(idx + 1) % values.length];
  setQuality(next);
}

const controllerLabel = computed(() => {
  const match = controllerOptions.find((o) => o.value === selectedController.value);
  return match?.label ?? "Auto";
});

const qualityLabel = computed(() => {
  const match = qualityOptions.find((o) => o.value === selectedQuality.value);
  return match?.label ?? "Auto";
});

const ASPECT_CYCLE: AspectRatio[] = ["Standard", "Wide16_9", "Wide16_10"];
const aspectLabel = computed(() => {
  switch (aspectRatio.value) {
    case "Wide16_9": return "16:9";
    case "Wide16_10": return "16:10";
    default: return "4:3";
  }
});

function toggleWidescreen() {
  const idx = ASPECT_CYCLE.indexOf(aspectRatio.value);
  aspectRatio.value = ASPECT_CYCLE[(idx + 1) % ASPECT_CYCLE.length];
  saveUserConfig();
  showSettingsToast(`Aspect Ratio: ${aspectLabel.value}`);
}

function toggleCrtShader() {
  crtShaderEnabled.value = !crtShaderEnabled.value;
  saveUserConfig();
  showSettingsToast(`CRT Shader: ${crtShaderEnabled.value ? "On" : "Off"}`);
}

async function applyProfileName() {
  showOptions.value = false;
  try {
    const msg = await invoke<string>("configure_game_emulator", { gameId });
    console.log("[EMU]", msg);
  } catch (e) {
    console.error("[EMU] Failed to apply profile:", e);
  }
}

// ── Options menu: gamepad-navigable list ──────────────────────────────────
interface OptionsMenuItem {
  id: string;
  label: string;
  valueLabel?: string;
  action: () => void;
}

const optionsMenuItems = computed<OptionsMenuItem[]>(() => {
  const items: OptionsMenuItem[] = [];

  if (isEmulatedGame.value) {
    items.push({
      id: "controller",
      label: "Controller Layout",
      valueLabel: controllerLabel.value,
      action: cycleController,
    });
    items.push({
      id: "quality",
      label: "Quality Preset",
      valueLabel: qualityLabel.value,
      action: cycleQuality,
    });
    items.push({
      id: "widescreen",
      label: "Aspect Ratio",
      valueLabel: aspectLabel.value,
      action: toggleWidescreen,
    });
    items.push({
      id: "crt-shader",
      label: "CRT Shader",
      valueLabel: crtShaderEnabled.value ? "On" : "Off",
      action: toggleCrtShader,
    });
  }

  if (isNativeGame.value && isWindowsGame.value && status.value?.type === "Installed") {
    items.push({
      id: "profile",
      label: "Set Account Name",
      action: applyProfileName,
    });
  }

  items.push({
    id: "updates",
    label: "Check for Updates",
    action: () => {
      showOptions.value = false;
      checkForUpdates();
    },
  });

  items.push({
    id: "add-to-shelf",
    label: "Add to Shelf",
    action: () => {
      showOptions.value = false;
      showShelfPicker.value = true;
    },
  });

  items.push({
    id: "remove-library",
    label: "Remove from Library",
    action: removeFromLibrary,
  });

  if (status.value?.type === "Installed") {
    items.push({
      id: "uninstall",
      label: "Uninstall",
      action: uninstallGame,
    });
  }

  return items;
});

const _optionsSubs: (() => void)[] = [];

function wireOptionsGamepad() {
  unwireOptionsGamepad();

  _optionsSubs.push(
    gamepad.onButton(GamepadButton.DPadUp, () => {
      if (!showOptions.value) return;
      optionsFocusIdx.value = Math.max(0, optionsFocusIdx.value - 1);
    }),
  );
  _optionsSubs.push(
    gamepad.onButton(GamepadButton.DPadDown, () => {
      if (!showOptions.value) return;
      optionsFocusIdx.value = Math.min(
        optionsMenuItems.value.length - 1,
        optionsFocusIdx.value + 1,
      );
    }),
  );
  _optionsSubs.push(
    gamepad.onButton(GamepadButton.South, () => {
      if (!showOptions.value) return;
      const item = optionsMenuItems.value[optionsFocusIdx.value];
      if (item) item.action();
    }),
  );
  _optionsSubs.push(
    gamepad.onButton(GamepadButton.East, () => {
      if (!showOptions.value) return;
      showOptions.value = false;
    }),
  );
}

function unwireOptionsGamepad() {
  for (const unsub of _optionsSubs) unsub();
  _optionsSubs.length = 0;
}

watch(showOptions, (v) => {
  if (v) {
    optionsFocusIdx.value = 0;
    optionsLockId = focusNav.acquireInputLock();
    wireOptionsGamepad();
  } else {
    unwireOptionsGamepad();
    focusNav.releaseInputLock(optionsLockId);
  }
});

const confirmUninstall = ref(false);

function uninstallGame() {
  showOptions.value = false;
  confirmUninstall.value = true;
}

async function doUninstall() {
  confirmUninstall.value = false;
  try {
    await invoke("uninstall_game", { gameId });

    // The Tauri uninstall runs in a background thread: it deletes local
    // files, sets the game status to "Remote", and emits "update_library".
    // We do NOT remove the game from the server-side collection — the game
    // should remain in the user's library as "not installed" after uninstall.
    // Give the background thread a moment to update the local DB before
    // navigating, otherwise the library page may show stale state.
    await new Promise((resolve) => setTimeout(resolve, 500));

    navigateTo("/bigpicture/library");
  } catch (e) {
    console.error("[BPM:GAME] Uninstall failed:", e);
    launchError.value = `Uninstall failed: ${e instanceof Error ? e.message : String(e)}`;
  }
}

// ── Remove from library ─────────────────────────────────────────────────

const confirmRemoveFromLibrary = ref(false);

function removeFromLibrary() {
  showOptions.value = false;
  confirmRemoveFromLibrary.value = true;
}

async function doRemoveFromLibrary() {
  confirmRemoveFromLibrary.value = false;
  try {
    const resp = await fetch(serverUrl("api/v1/collection/default/entry"), {
      method: "DELETE",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ id: gameId }),
    });
    if (!resp.ok) {
      throw new Error(`Server returned ${resp.status}: ${resp.statusText}`);
    }
    navigateTo("/bigpicture/library");
  } catch (e) {
    console.error("[BPM:GAME] Remove from library failed:", e);
    launchError.value = `Failed to remove: ${e instanceof Error ? e.message : String(e)}`;
  }
}

// ── Shelf picker ────────────────────────────────────────────────────────
const showShelfPicker = ref(false);
const shelvesData = useShelves();
const newShelfNameInPicker = ref("");
const showNewShelfKeyboard = ref(false);
const shelfFocusIdx = ref(0);
const _shelfSubs: (() => void)[] = [];
let shelfLockId = "";

function wireShelfGamepad() {
  unwireShelfGamepad();
  const totalItems = shelvesData.shelves.value.length + 2; // shelves + Create button + Done button
  _shelfSubs.push(
    gamepad.onButton(GamepadButton.DPadUp, () => {
      if (!showShelfPicker.value) return;
      shelfFocusIdx.value = Math.max(0, shelfFocusIdx.value - 1);
    }),
    gamepad.onButton(GamepadButton.DPadDown, () => {
      if (!showShelfPicker.value) return;
      shelfFocusIdx.value = Math.min(totalItems - 1, shelfFocusIdx.value + 1);
    }),
    gamepad.onButton(GamepadButton.South, () => {
      if (!showShelfPicker.value) return;
      const idx = shelfFocusIdx.value;
      const shelfCount = shelvesData.shelves.value.length;
      if (idx < shelfCount) {
        toggleGameOnShelf(shelvesData.shelves.value[idx].id);
      } else if (idx === shelfCount) {
        showNewShelfKeyboard.value = true; // Create New Shelf button
      } else {
        showShelfPicker.value = false; // Done button
      }
    }),
    gamepad.onButton(GamepadButton.East, () => {
      if (!showShelfPicker.value) return;
      showShelfPicker.value = false;
    }),
  );
}

function unwireShelfGamepad() {
  for (const unsub of _shelfSubs) unsub();
  _shelfSubs.length = 0;
}

watch(showShelfPicker, (v) => {
  if (v) {
    shelfFocusIdx.value = 0;
    shelfLockId = focusNav.acquireInputLock();
    wireShelfGamepad();
  } else {
    unwireShelfGamepad();
    focusNav.releaseInputLock(shelfLockId);
  }
});

// Load shelves when game page mounts
onMounted(() => { shelvesData.fetchShelves(); });

async function toggleGameOnShelf(shelfId: string) {
  const shelf = shelvesData.shelves.value.find((s) => s.id === shelfId);
  if (!shelf) return;
  const isOnShelf = shelf.entries.some((e) => e.gameId === gameId);
  if (isOnShelf) {
    await shelvesData.removeFromShelf(shelfId, gameId);
  } else {
    await shelvesData.addToShelf(shelfId, gameId);
  }
}

async function createShelfAndAdd() {
  const name = newShelfNameInPicker.value.trim();
  if (!name) return;
  const shelf = await shelvesData.createShelf(name);
  newShelfNameInPicker.value = "";
  if (shelf) {
    await shelvesData.addToShelf(shelf.id, gameId);
  }
}

/** When an achievement icon fails to load, swap it for the trophy fallback. */
function onAchievementIconError(event: Event) {
  const img = event.target as HTMLImageElement;
  console.warn("[BPM:GAME] Achievement icon failed to load:", img.src);
  // Hide broken image, show a trophy-colored placeholder
  img.style.display = "none";
  // Insert a fallback element after the broken img
  const fallback = document.createElement("div");
  fallback.className = "size-12 rounded-lg bg-zinc-800 flex items-center justify-center";
  fallback.innerHTML = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor" class="size-6 text-zinc-600"><path fill-rule="evenodd" d="M5.166 2.621v.858c-1.035.148-2.059.33-3.071.543a.75.75 0 0 0-.584.859 6.753 6.753 0 0 0 6.138 5.6 6.73 6.73 0 0 0 2.743 1.346A6.707 6.707 0 0 1 9.279 15H8.54c-1.036 0-1.875.84-1.875 1.875V19.5h-.75a2.25 2.25 0 0 0-2.25 2.25c0 .414.336.75.75.75h15.19a.75.75 0 0 0 .75-.75 2.25 2.25 0 0 0-2.25-2.25h-.75v-2.625c0-1.036-.84-1.875-1.875-1.875h-.739a6.707 6.707 0 0 1-1.112-3.173 6.73 6.73 0 0 0 2.743-1.347 6.753 6.753 0 0 0 6.139-5.6.75.75 0 0 0-.585-.858 47.077 47.077 0 0 0-3.07-.543V2.62a.75.75 0 0 0-.658-.744 49.22 49.22 0 0 0-6.093-.377c-2.063 0-4.096.128-6.093.377a.75.75 0 0 0-.657.744Z" clip-rule="evenodd" /></svg>`;
  img.parentNode?.insertBefore(fallback, img.nextSibling);
}

const tabs = [
  { label: "Achievements", value: "achievements" },
  { label: "Details", value: "details" },
  { label: "Gallery", value: "gallery" },
  { label: "Saves", value: "saves" },
];

interface AchievementItem {
  id: string;
  title: string;
  description: string;
  iconUrl?: string;
  unlocked: boolean;
  rarity?: number;
  unlockCount?: number;
}

const achievements: Ref<AchievementItem[]> = ref([]);

// ── Playtime data ─────────────────────────────────────────────────────────
const gamePlaytime = ref<{ totalSeconds: number; lastPlayedAt: string | null } | null>(null);

async function fetchPlaytime() {
  try {
    const url = serverUrl("api/v1/client/playtime/recent");
    const resp = await fetch(url);
    if (!resp.ok) return;
    const data = await resp.json() as Array<{
      gameId: string;
      totalPlaytimeSeconds: number;
      lastPlayedAt: string;
    }>;
    const entry = data.find((d) => d.gameId === gameId);
    if (entry) {
      gamePlaytime.value = {
        totalSeconds: entry.totalPlaytimeSeconds,
        lastPlayedAt: entry.lastPlayedAt,
      };
    }
  } catch {
    // Non-critical — just don't show playtime
  }
}

function formatTimeAgo(dateStr: string): string {
  const diff = Math.floor((Date.now() - new Date(dateStr).getTime()) / 1000);
  if (diff < 60) return "just now";
  if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
  if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`;
  if (diff < 604800) return `${Math.floor(diff / 86400)}d ago`;
  return `${Math.floor(diff / 604800)}w ago`;
}

// ── Save state management ─────────────────────────────────────────────────
interface SaveFile {
  filename: string;
  size: number;
  modified: number;
  save_type: string;
}
const gameSaves = ref<SaveFile[]>([]);
const savesLoading = ref(false);

async function fetchSaves() {
  savesLoading.value = true;
  try {
    const saves: SaveFile[] = await invoke("list_game_saves", { gameId });
    gameSaves.value = saves;
  } catch {
    gameSaves.value = [];
  } finally {
    savesLoading.value = false;
  }
}

async function deleteSave(save: SaveFile) {
  try {
    await invoke("delete_game_save", {
      gameId,
      filename: save.filename,
      saveType: save.save_type,
    });
    gameSaves.value = gameSaves.value.filter((s) => s.filename !== save.filename);
  } catch (e) {
    console.error("[BPM:GAME] Failed to delete save:", e);
    launchError.value = `Failed to delete save: ${e instanceof Error ? e.message : String(e)}`;
  }
}

// ── Ludusavi PC game saves ─────────────────────────────────────────────────
interface LudusaviFile { path: string; size: number; modified: number }
const pcSaves = ref<LudusaviFile[]>([]);
const pcSaveStatus = ref("");
const ludusaviAvailable = ref(false);
const ludusaviInstalling = ref(false);

async function doInstallLudusavi() {
  ludusaviInstalling.value = true;
  try {
    await invoke("install_ludusavi");
    ludusaviAvailable.value = true;
    // Now fetch PC saves since Ludusavi is installed
    await fetchPcSaves();
  } catch (e) {
    console.error("[BPM:GAME] Ludusavi install failed:", e);
    launchError.value = `Ludusavi install failed: ${e instanceof Error ? e.message : String(e)}`;
  } finally {
    ludusaviInstalling.value = false;
  }
}

async function fetchPcSaves() {
  try {
    ludusaviAvailable.value = await invoke("check_ludusavi");
    if (!ludusaviAvailable.value || !game.value) return;

    const result: { files: LudusaviFile[]; game_name: string } = await invoke("list_pc_game_saves", {
      gameId,
      gameName: game.value.mName,
    });
    pcSaves.value = result.files;
  } catch {
    pcSaves.value = [];
  }
}

async function backupPcSaves() {
  if (!game.value) return;
  pcSaveStatus.value = "backing-up";
  try {
    const backupPath: string = await invoke("backup_pc_game_saves", {
      gameId,
      gameName: game.value.mName,
    });
    // Upload the backup to cloud
    // For now, just show success
    launchError.value = null;
    console.log("[BPM:GAME] Ludusavi backup at:", backupPath);
  } catch (e) {
    launchError.value = `Backup failed: ${e instanceof Error ? e.message : String(e)}`;
  } finally {
    pcSaveStatus.value = "";
  }
}

async function restorePcSaves() {
  pcSaveStatus.value = "restoring";
  try {
    // Look for existing backup
    const backupPath = `${await invoke("get_temp_dir")}drop-ludusavi-${gameId}`.replace(/\\/g, "/");
    await invoke("restore_pc_game_saves", { backupPath });
  } catch (e) {
    launchError.value = `Restore failed: ${e instanceof Error ? e.message : String(e)}`;
  } finally {
    pcSaveStatus.value = "";
  }
}

// Group PC saves into save slots with their backups
interface PcSaveGroup {
  name: string;
  label: string;
  type: "save" | "settings" | "other";
  primary: LudusaviFile | null;
  backups: LudusaviFile[];
  expanded: boolean;
}

const pcSaveGroups = ref<PcSaveGroup[]>([]);

watch(pcSaves, (saves) => {
  const filtered = saves.filter((f) => {
    const lower = f.path.toLowerCase();
    if (lower.includes("crashreportclient")) return false;
    if (lower.includes("uecc-windows-")) return false;
    if (lower.endsWith(".log") || lower.endsWith(".tmp")) return false;
    return true;
  });

  // Group: find primary saves and attach their backups
  const groups = new Map<string, PcSaveGroup>();

  // Use case-insensitive keys to avoid duplicates on Windows
  for (const file of filtered) {
    const filename = pcSaveFileName(file.path);
    const lower = filename.toLowerCase();
    const key = lower; // case-insensitive grouping key

    // Determine if this is a backup of another file
    const backupMatch = lower.match(/^(.+?)_backup\d*\.(\w+)$/);
    if (backupMatch) {
      const parentKey = `${backupMatch[1]}.${backupMatch[2]}`;
      const existing = groups.get(parentKey);
      if (existing) {
        existing.backups.push(file);
      } else {
        const displayName = filename.replace(/_backup\d*/, "");
        groups.set(parentKey, {
          name: displayName,
          label: displayName.replace(/_/g, " ").replace(/\.\w+$/, ""),
          type: "save",
          primary: null,
          backups: [file],
          expanded: false,
        });
      }
      continue;
    }

    // Determine type
    const isSettings = lower.endsWith(".ini") || lower.endsWith(".cfg");
    const type = isSettings ? "settings" as const : "save" as const;

    const existing = groups.get(key);
    if (existing) {
      // Keep the version with more data (larger file or first seen)
      if (!existing.primary || file.size > existing.primary.size) {
        existing.primary = file;
      }
      existing.type = type;
      existing.label = isSettings ? "Settings" : filename.replace(/_/g, " ").replace(/\.\w+$/, "");
    } else {
      groups.set(key, {
        name: filename,
        label: isSettings ? "Settings" : filename.replace(/_/g, " ").replace(/\.\w+$/, ""),
        type,
        primary: file,
        backups: [],
        expanded: false,
      });
    }
  }

  // Sort: saves first, then settings
  pcSaveGroups.value = [...groups.values()].sort((a, b) => {
    const typeOrder = (t: string) => t === "save" ? 0 : t === "settings" ? 2 : 1;
    return typeOrder(a.type) - typeOrder(b.type);
  });
}, { immediate: true });

// Filter and format PC saves — hide crash reports, show just filenames
const filteredPcSaves = computed(() => {
  return pcSaves.value
    .filter((f) => {
      const lower = f.path.toLowerCase();
      // Hide crash report files and temp files
      if (lower.includes("crashreportclient")) return false;
      if (lower.includes("uecc-windows-")) return false;
      if (lower.endsWith(".log")) return false;
      if (lower.endsWith(".tmp")) return false;
      return true;
    })
    .sort((a, b) => {
      // Save files first, then config, then others
      const typeOrder = (p: string) => {
        const l = p.toLowerCase();
        if (l.endsWith(".sav") || l.endsWith(".save") || l.includes("savegame")) return 0;
        if (l.endsWith(".ini") || l.endsWith(".cfg")) return 2;
        return 1;
      };
      const diff = typeOrder(a.path) - typeOrder(b.path);
      if (diff !== 0) return diff;
      return b.size - a.size; // Larger files first within same type
    });
});

function pcSaveFileName(fullPath: string): string {
  // Extract just the filename from the full path
  const parts = fullPath.replace(/\\/g, "/").split("/");
  return parts[parts.length - 1] || fullPath;
}

function pcSaveFileType(fullPath: string): string {
  const lower = fullPath.toLowerCase();
  if (lower.endsWith(".sav")) return "Game Save";
  if (lower.includes("backup")) return "Auto Backup";
  if (lower.endsWith(".ini") || lower.endsWith(".cfg")) return "Settings";
  if (lower.endsWith(".json")) return "Data";
  return "Save Data";
}

function pcSaveFileColor(fullPath: string): { bg: string; text: string } {
  const lower = fullPath.toLowerCase();
  if (lower.endsWith(".sav") || lower.endsWith(".save")) {
    if (lower.includes("backup")) return { bg: "rgba(234,179,8,0.15)", text: "#eab308" }; // yellow for backups
    return { bg: "rgba(34,197,94,0.15)", text: "#22c55e" }; // green for saves
  }
  if (lower.endsWith(".ini") || lower.endsWith(".cfg")) return { bg: "rgba(156,163,175,0.15)", text: "#9ca3af" }; // grey for config
  return { bg: "rgba(168,85,247,0.15)", text: "#a855f7" }; // purple for other
}

// ── Cloud saves ───────────────────────────────────────────────────────────
interface CloudSaveEntry {
  id: string;
  filename: string;
  saveType: string;
  size: number;
  clientModifiedAt: string;
  uploadedAt: string;
}
const cloudSaves = ref<CloudSaveEntry[]>([]);
const cloudSyncStatus = ref<Record<string, string>>({});

async function fetchCloudSaves() {
  try {
    const url = serverUrl(`api/v1/client/saves/list?gameId=${gameId}`);
    const resp = await fetch(url);
    if (resp.ok) {
      cloudSaves.value = await resp.json();
    }
  } catch { /* non-critical */ }
}

// Confirmation state for cloud sync
const confirmSyncAction = ref<{ type: "upload" | "download"; save: SaveFile | null; filename: string; saveType: string } | null>(null);

function requestUpload(save: SaveFile) {
  // Check if cloud version already exists
  const hasCloud = cloudSaves.value.some((c) => c.filename === save.filename);
  if (hasCloud) {
    confirmSyncAction.value = { type: "upload", save, filename: save.filename, saveType: save.save_type };
  } else {
    doUpload(save);
  }
}

function requestDownload(filename: string, saveType: string) {
  // Check if local version already exists
  const hasLocal = gameSaves.value.some((s) => s.filename === filename);
  if (hasLocal) {
    confirmSyncAction.value = { type: "download", save: null, filename, saveType };
  } else {
    doDownload(filename, saveType);
  }
}

function confirmSync() {
  if (!confirmSyncAction.value) return;
  const action = confirmSyncAction.value;
  confirmSyncAction.value = null;
  if (action.type === "upload" && action.save) {
    doUpload(action.save);
  } else if (action.type === "download") {
    doDownload(action.filename, action.saveType);
  }
}

async function doUpload(save: SaveFile) {
  cloudSyncStatus.value[save.filename] = "uploading";
  try {
    // Before overwriting, create a backup with .bak suffix in cloud
    const existingCloud = cloudSaves.value.find((c) => c.filename === save.filename);
    if (existingCloud) {
      // Download the existing cloud version as a backup
      const backupUrl = serverUrl(`api/v1/client/saves/download?id=${existingCloud.id}`);
      const backupResp = await fetch(backupUrl);
      if (backupResp.ok) {
        const backupData = await backupResp.json();
        // Upload backup with .bak suffix
        await fetch(serverUrl("api/v1/client/saves/upload"), {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({
            gameId,
            filename: save.filename + ".bak",
            saveType: save.save_type,
            data: backupData.data,
            clientModifiedAt: existingCloud.clientModifiedAt,
          }),
        });
      }
    }

    // Read local file as base64 via Tauri
    const base64Data: string = await invoke("read_save_file", {
      gameId,
      filename: save.filename,
      saveType: save.save_type,
    });

    // Upload to server
    const resp = await fetch(serverUrl("api/v1/client/saves/upload"), {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        gameId,
        filename: save.filename,
        saveType: save.save_type,
        data: base64Data,
        clientModifiedAt: new Date(save.modified * 1000).toISOString(),
      }),
    });
    if (!resp.ok) throw new Error(`Upload failed: ${resp.status}`);

    await fetchCloudSaves();
  } catch (e) {
    console.error("[BPM:GAME] Cloud save upload failed:", e);
    launchError.value = `Upload failed: ${e instanceof Error ? e.message : String(e)}`;
  } finally {
    delete cloudSyncStatus.value[save.filename];
  }
}

async function doDownload(filename: string, saveType: string) {
  const cloudEntry = cloudSaves.value.find((c) => c.filename === filename);
  if (!cloudEntry) return;

  cloudSyncStatus.value[filename] = "downloading";
  try {
    // Before overwriting local, create a backup of the local file
    const localSave = gameSaves.value.find((s) => s.filename === filename);
    if (localSave) {
      try {
        const localData: string = await invoke("read_save_file", {
          gameId,
          filename: localSave.filename,
          saveType: localSave.save_type,
        });
        // Write backup locally with .bak suffix
        await invoke("write_save_file", {
          gameId,
          filename: filename + ".bak",
          saveType,
          data: localData,
        });
      } catch {
        // Backup failed — continue anyway
      }
    }

    // Download from server
    const url = serverUrl(`api/v1/client/saves/download?id=${cloudEntry.id}`);
    const resp = await fetch(url);
    if (!resp.ok) throw new Error(`Download failed: ${resp.status}`);
    const { data } = await resp.json();

    // Write to local file via Tauri
    await invoke("write_save_file", {
      gameId,
      filename,
      saveType,
      data,
    });

    // Refresh local saves list
    await fetchSaves();
  } catch (e) {
    console.error("[BPM:GAME] Cloud save download failed:", e);
    launchError.value = `Download failed: ${e instanceof Error ? e.message : String(e)}`;
  } finally {
    delete cloudSyncStatus.value[filename];
  }
}

// ── Merged save view (local + cloud) ──────────────────────────────────────
interface MergedSave {
  filename: string;
  local: SaveFile | null;
  cloud: CloudSaveEntry | null;
}

const mergedSaves = computed((): MergedSave[] => {
  const map = new Map<string, MergedSave>();

  // Add local saves
  for (const save of gameSaves.value) {
    map.set(save.filename, { filename: save.filename, local: save, cloud: null });
  }

  // Merge cloud saves
  for (const cloud of cloudSaves.value) {
    const existing = map.get(cloud.filename);
    if (existing) {
      existing.cloud = cloud;
    } else {
      map.set(cloud.filename, { filename: cloud.filename, local: null, cloud });
    }
  }

  // Sort: .srm first, then .state, then .png. Within each group, newest first.
  return [...map.values()].sort((a, b) => {
    const extOrder = (f: string) => f.endsWith('.srm') ? 0 : f.endsWith('.state') ? 1 : 2;
    const diff = extOrder(a.filename) - extOrder(b.filename);
    if (diff !== 0) return diff;
    const aTime = a.local?.modified ?? 0;
    const bTime = b.local?.modified ?? 0;
    return bTime - aTime;
  });
});

function saveTypeLabel(filename: string): string {
  if (filename.endsWith('.srm')) return 'Game Progress (Battery Save)';
  if (filename.endsWith('.state.png')) return 'Save State Screenshot';
  if (filename.endsWith('.state')) return 'Save State (Exact Position)';
  if (filename.endsWith('.sav')) return 'Game Save';
  return 'Save File';
}

function saveTypeColor(filename: string): { bg: string; text: string } {
  if (filename.endsWith('.srm')) return { bg: 'rgba(34,197,94,0.15)', text: '#22c55e' };
  if (filename.endsWith('.state.png')) return { bg: 'rgba(168,85,247,0.15)', text: '#a855f7' };
  if (filename.endsWith('.state')) return { bg: 'rgba(59,130,246,0.15)', text: '#3b82f6' };
  return { bg: 'rgba(156,163,175,0.15)', text: '#9ca3af' };
}

function formatSaveSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

// ── Per-save cloud sync for PC saves ─────────────────────────────────────
const pcSyncStatus = ref<Record<string, string>>({});
const pcCloudSaves = ref<Record<string, CloudSaveEntry>>({});
const pcCloudStatus = ref<Record<string, string>>({});

function refreshPcCloudStatus() {
  const map: Record<string, CloudSaveEntry> = {};
  for (const cloud of cloudSaves.value) {
    if (cloud.filename.startsWith("pc:")) {
      const groupName = cloud.filename.slice(3);
      map[groupName.toLowerCase()] = cloud;
    }
  }
  pcCloudSaves.value = map;

  const status: Record<string, string> = {};
  for (const group of pcSaveGroups.value) {
    const key = group.name.toLowerCase();
    const cloud = map[key];
    if (!cloud) continue;
    if (!group.primary) {
      status[group.name] = "cloud-only";
    } else {
      const localModified = group.primary.modified * 1000;
      const cloudModified = new Date(cloud.clientModifiedAt).getTime();
      if (Math.abs(localModified - cloudModified) < 2000) {
        status[group.name] = "synced";
      } else if (cloudModified > localModified) {
        status[group.name] = "cloud-newer";
      } else {
        status[group.name] = "local-newer";
      }
    }
  }
  pcCloudStatus.value = status;
}

watch(cloudSaves, refreshPcCloudStatus, { immediate: true });
watch(pcSaveGroups, refreshPcCloudStatus);

// Helper to check if a cloud save exists for a PC save group (avoids Map.has() reactivity issues in templates)
function hasPcCloudSave(groupName: string): boolean {
  return groupName.toLowerCase() in pcCloudSaves.value;
}

async function uploadPcSave(group: PcSaveGroup) {
  if (!group.primary || !game.value) return;
  pcSyncStatus.value[group.name] = "uploading";
  try {
    const base64Data: string = await invoke("read_pc_save_file", {
      filePath: group.primary.path,
    });

    const cloudFilename = `pc:${group.name}`;
    const resp = await fetch(serverUrl("api/v1/client/saves/upload"), {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        gameId,
        filename: cloudFilename,
        saveType: "save",
        data: base64Data,
        clientModifiedAt: new Date(group.primary.modified * 1000).toISOString(),
      }),
    });
    if (!resp.ok) throw new Error(`Upload failed: ${resp.status}`);

    await fetchCloudSaves();
    console.log("[BPM:GAME] PC save uploaded:", group.name);
  } catch (e) {
    console.error("[BPM:GAME] PC save upload failed:", e);
    launchError.value = `Upload failed: ${e instanceof Error ? e.message : String(e)}`;
  } finally {
    delete pcSyncStatus.value[group.name];
  }
}

async function downloadPcSave(group: PcSaveGroup) {
  const key = group.name.toLowerCase();
  const cloudEntry = pcCloudSaves.value[key];
  if (!cloudEntry) return;

  pcSyncStatus.value[group.name] = "downloading";
  try {
    const url = serverUrl(`api/v1/client/saves/download?id=${cloudEntry.id}`);
    const resp = await fetch(url);
    if (!resp.ok) throw new Error(`Download failed: ${resp.status}`);
    const { data } = await resp.json();

    if (!group.primary) {
      launchError.value = "No local path known for this save — cannot restore.";
      return;
    }

    await invoke("write_pc_save_file", {
      filePath: group.primary.path,
      data,
    });

    await fetchPcSaves();
    console.log("[BPM:GAME] PC save downloaded:", group.name);
  } catch (e) {
    console.error("[BPM:GAME] PC save download failed:", e);
    launchError.value = `Download failed: ${e instanceof Error ? e.message : String(e)}`;
  } finally {
    delete pcSyncStatus.value[group.name];
  }
}

// Load saves when the saves tab is selected
watch(() => activeTab.value, (tab) => {
  if (tab === "saves") {
    if (gameSaves.value.length === 0) fetchSaves();
    fetchCloudSaves();
    if (isNativeGame.value) fetchPcSaves();
  }
});

// ── Recommended games ──────────────────────────────────────────────────
interface RecommendedGame {
  id: string;
  mName: string;
  mCoverObjectId: string | null;
}
const recommendedGames = ref<RecommendedGame[]>([]);

async function fetchRecommendations() {
  try {
    const url = serverUrl("api/v1/store/recommended");
    const resp = await fetch(url);
    if (!resp.ok) return;
    const data = await resp.json();
    // Filter out the current game and take up to 8
    const games = (data.games ?? data ?? []) as RecommendedGame[];
    recommendedGames.value = games
      .filter((g: RecommendedGame) => g.id !== gameId)
      .slice(0, 8);
  } catch {
    // Non-critical
  }
}

function formatPlaytimeDetailed(seconds: number): string {
  if (seconds < 60) return `${seconds}s`;
  if (seconds < 3600) return `${Math.floor(seconds / 60)}m`;
  const hours = Math.floor(seconds / 3600);
  const mins = Math.floor((seconds % 3600) / 60);
  return mins > 0 ? `${hours}h ${mins}m` : `${hours}h`;
}

function registerTabRef(value: string, el: any) {
  if (el) {
    tabRefs[value] = el;
    // Do NOT call updateTabIndicator() here — this runs inside a :ref
    // callback during render. Modifying reactive state (tabIndicatorStyle)
    // during render causes an infinite update loop.
  }
}

function updateTabIndicator() {
  const activeEl = tabRefs[activeTab.value];
  if (activeEl) {
    tabIndicatorStyle.value = {
      left: `${activeEl.offsetLeft}px`,
      width: `${activeEl.offsetWidth}px`,
    };
  }
}

watch(activeTab, () => {
  nextTick(() => updateTabIndicator());
});

// ── Achievement rarity display ──────────────────────────────────────────
function rarityColor(rarity: number): string {
  if (rarity < 5) return "bg-yellow-500";      // Ultra Rare - gold
  if (rarity < 20) return "bg-purple-500";     // Rare
  if (rarity < 50) return "bg-blue-500";       // Uncommon
  return "bg-zinc-500";                         // Common
}

function rarityTextColor(rarity: number): string {
  if (rarity < 5) return "text-yellow-400";
  if (rarity < 20) return "text-purple-400";
  if (rarity < 50) return "text-blue-400";
  return "text-zinc-500";
}

const unlockedCount = computed(() => achievements.value.filter(a => a.unlocked).length);
const achievementPercent = computed(() => achievements.value.length > 0 ? (unlockedCount.value / achievements.value.length) * 100 : 0);

// ── ROM Hash Verification (RetroAchievements) ──────────────────────────
type RomHashResult = {
  status: "Match" | "Mismatch" | "NoHashData" | "Error";
  rom_hash?: string;
  matched_label?: string;
  expected_hashes?: { hash: string; label: string; patchUrl: string }[];
  message?: string;
};

const romHashResult = ref<RomHashResult | null>(null);
const romHashChecking = ref(false);

// Listen for launch-time hash check results
let unlistenHash: (() => void) | null = null;
onMounted(async () => {
  unlistenHash = await listen<RomHashResult>(
    `ra_hash_check/${gameId}`,
    (event) => {
      romHashResult.value = event.payload;
    },
  );
});

// ── Cloud Save Conflict Resolution ──────────────────────────────────────
import type { SaveConflict } from "~/types/save-sync";

const saveConflictVisible = ref(false);
const saveConflicts = ref<SaveConflict[]>([]);

let unlistenConflict: (() => void) | null = null;
onMounted(async () => {
  unlistenConflict = await listen<{ gameId: string; conflicts: SaveConflict[] }>(
    `save_sync_conflict/${gameId}`,
    (event) => {
      saveConflicts.value = event.payload.conflicts;
      saveConflictVisible.value = true;
    },
  );
});

function triggerTestConflict() {
  saveConflicts.value = [
    {
      filename: "test-save-01.srm",
      saveType: "save",
      localHash: "abc123local",
      localSize: 32768,
      localModifiedAt: Math.floor(Date.now() / 1000) - 3600,
      cloudId: "cloud-save-uuid-1",
      cloudHash: "def456cloud",
      cloudSize: 32512,
      cloudModifiedAt: new Date(Date.now() - 7200_000).toISOString(),
      cloudUploadedFrom: "Gaming-PC",
    },
    {
      filename: "test-save-02.state",
      saveType: "state",
      localHash: "ghi789local",
      localSize: 1048576,
      localModifiedAt: Math.floor(Date.now() / 1000) - 600,
      cloudId: "cloud-save-uuid-2",
      cloudHash: "jkl012cloud",
      cloudSize: 1048320,
      cloudModifiedAt: new Date(Date.now() - 1800_000).toISOString(),
      cloudUploadedFrom: "Living-Room-PC",
    },
    {
      filename: "pc/AppData/Saves/slot1.sav",
      saveType: "pc",
      localHash: "mno345local",
      localSize: 524288,
      localModifiedAt: Math.floor(Date.now() / 1000) - 120,
      cloudId: "cloud-save-uuid-3",
      cloudHash: "pqr678cloud",
      cloudSize: 524000,
      cloudModifiedAt: new Date(Date.now() - 300_000).toISOString(),
      cloudUploadedFrom: "Laptop",
    },
  ] as SaveConflict[];
  saveConflictVisible.value = true;
}

// On-demand hash check
async function checkRomHash() {
  romHashChecking.value = true;
  romHashResult.value = null;
  try {
    const result = await invoke<RomHashResult>("check_ra_rom_hash", {
      gameId,
    });
    romHashResult.value = result;
  } catch (e) {
    romHashResult.value = {
      status: "Error",
      message: String(e),
    };
  } finally {
    romHashChecking.value = false;
  }
}

// Helper: race a promise against a timeout
function withTimeout<T>(promise: Promise<T>, ms: number): Promise<T | null> {
  return Promise.race([
    promise,
    new Promise<null>((resolve) => setTimeout(() => resolve(null), ms)),
  ]);
}

onMounted(async () => {
  console.log(`[BPM:GAME] === Page mounted for gameId: ${gameId} ===`);
  console.log(`[BPM:GAME] Route: ${route.fullPath}`);

  // Wire up gamepad immediately — don't wait for data to load
  _unsubs.push(
    gamepad.onButton(GamepadButton.Start, () => {
      showOptions.value = true;
    }),
  );

  // Listen for external launch errors (process crashes / wrong binary format)
  const { listen } = await import("@tauri-apps/api/event");
  const unlistenLaunchTrace = await listen("launch_trace", (event) => {
    const p = event.payload as any;
    console.log(`[BPM:TRACE:${p.step}]`, JSON.stringify(p, null, 2));
    // Surface BIOS warnings to the user so they know why a game crashed
    if (p.step === "7_retroarch_config_result" && p.bios_warnings?.length) {
      launchError.value = p.bios_warnings.join("\n");
    }
  });
  _unsubs.push(() => unlistenLaunchTrace());

  const unlistenLaunchError = await listen("launch_external_error", (event) => {
    if (event.payload === gameId) {
      console.error("[BPM:GAME] External launch error for:", gameId);
      launchError.value = "The game may have failed to launch. Check the game's compatibility — Windows games require Proton/UMU on Linux.";
      runDiagnostics();
    }
  });
  _unsubs.push(() => unlistenLaunchError());

  console.log("[BPM:GAME] Gamepad wired. Starting data fetch...");

  // Fire all fetches in parallel — apply results as each resolves instead
  // of waiting for all (avoids a slow fetch blocking the entire page).

  const achievementsUrl = serverUrl(`api/v1/games/${gameId}/achievements`);
  console.log("[BPM:GAME] Achievements URL:", achievementsUrl);

  // Game data — needed for the page header, status, and config
  // useGame is a local Tauri invoke (usually cached) — 5s is generous
  const gamePromise = withTimeout(useGame(gameId), 5000)
    .then((r) => {
      if (!r) { console.warn("[BPM:GAME] useGame TIMED OUT or null"); return; }
      console.log("[BPM:GAME] useGame resolved:", r.game?.mName ?? "null");
      game.value = r.game;
      statusRef.value = r.status;
      version.value = r.version?.value ?? null;
      console.log("[BPM:GAME] Game loaded:", r.game.mName, "| Status:", r.status?.value);
      if (version.value?.userConfiguration) {
        selectedController.value = version.value.userConfiguration.controllerType ?? null;
        selectedQuality.value = version.value.userConfiguration.qualityPreset ?? null;
        const ws = version.value.userConfiguration.widescreen;
        if (ws === true) aspectRatio.value = "Wide16_9";
        else if (ws === false || ws == null) aspectRatio.value = "Standard";
        else aspectRatio.value = ws as AspectRatio;
        crtShaderEnabled.value = version.value.userConfiguration.crtShader ?? false;
      }
    })
    .catch((e) => console.error("[BPM:GAME] useGame FAILED:", e));

  // Version options — can arrive late without blocking the page
  const versionPromise = invoke<VersionOption[]>("fetch_game_version_options", { gameId })
    .then((r) => {
      console.log("[BPM:GAME] version_options resolved:", r?.length ?? 0, "options");
      if (r) versionOptions.value = r;
    })
    .catch((e) => console.warn("[BPM:GAME] version_options failed:", e));

  // Achievements — server:// proxied fetch, 5s timeout
  const achievementsPromise = withTimeout(
    fetch(achievementsUrl).then((res) => {
      console.log("[BPM:GAME] achievements fetch status:", res.status);
      return res.ok ? res.json() : null;
    }),
    5000,
  )
    .then((r) => {
      if (!r) { console.warn("[BPM:GAME] achievements timed out or null"); return; }
      achievements.value = Array.isArray(r) ? r : (r.achievements ?? []);
      console.log("[BPM:GAME] Achievements loaded:", achievements.value.length);
      if (achievements.value.length > 0) {
        const sample = achievements.value.slice(0, 3);
        for (const a of sample) {
          console.log(`[BPM:GAME] Achievement "${a.title}" iconUrl: ${a.iconUrl || "(empty)"}`);
        }
      }
    })
    .catch((e) => console.warn("[BPM:GAME] achievements fetch FAILED:", e));

  // Wait for the critical data (game + achievements) before setting up focus.
  // version_options, playtime, and recommendations are intentionally NOT awaited.
  fetchPlaytime();
  fetchRecommendations();
  await Promise.all([gamePromise, achievementsPromise]);
  console.log("[BPM:GAME] Critical data loaded, versions pending:", !versionOptions.value);

  console.log("[BPM:GAME] Setting up focus...");
  nextTick(() => updateTabIndicator());
  focusNav.autoFocusContent("content");
  console.log("[BPM:GAME] === Page setup complete ===");
});

function _onResize() {
  updateTabIndicator();
}
onMounted(() => {
  window.addEventListener("resize", _onResize);
  // Load other devices for the dropdown
  loadDevices();
  // Start polling for remote streaming sessions (receiver side)
  pollRemoteSessions();
  streamPollInterval = setInterval(pollRemoteSessions, 15_000);
});

onUnmounted(() => {
  for (const unsub of _unsubs) unsub();
  _unsubs.length = 0;
  unwireOptionsGamepad();
  unlistenHash?.();
  unlistenConflict?.();
  if (showOptions.value) focusNav.releaseInputLock(optionsLockId);
  window.removeEventListener("resize", _onResize);
  // Clean up streaming
  if (streamPollInterval) { clearInterval(streamPollInterval); streamPollInterval = null; }
  if (heartbeatInterval) { clearInterval(heartbeatInterval); heartbeatInterval = null; }
});

// Guard against re-triggering launch immediately after dismissing error dialog.
// When the user presses A to dismiss the error dialog, the same A press can
// propagate to the Play button underneath, causing an immediate re-launch loop.
let launchGuard = false;

function dismissLaunchError() {
  launchError.value = null;
  launchGuard = true;
  setTimeout(() => { launchGuard = false; }, 300);
}

async function launchGame() {
  if (launchGuard) return;
  try {
    const result: LaunchResult = await invoke("launch_game", {
      id: gameId,
      index: 0,
    });
    if (result.result === "InstallRequired") {
      // Auto-download the required dependency (e.g. runtime/tool)
      const [depGameId, depVersionId] = result.data;
      try {
        const installDirs = await invoke<string[]>("fetch_download_dir_stats");
        await invoke("download_game", {
          gameId: depGameId,
          versionId: depVersionId,
          installDir: 0,
          targetPlatform: versionOptions.value?.[0]?.platform ?? "linux",
          enableUpdates: true,
        });
        launchError.value = `A required dependency is being installed. Please try launching again once the download completes.`;
      } catch (depErr) {
        launchError.value = `A required dependency needs to be installed first, but the download failed: ${depErr instanceof Error ? depErr.message : String(depErr)}`;
      }
    } else if (result.result !== "Success") {
      launchError.value = `Failed to launch: ${result.result}`;
    }
  } catch (e) {
    const errMsg = e instanceof Error ? e.message : String(e);
    console.error("[BPM:GAME] Launch error:", errMsg);
    // Auto-run diagnostics on any launch failure for debug logs
    runDiagnostics();
    // Provide user-friendly hints for common errors
    if (errMsg.includes("exec format error") || errMsg.includes("os error 8")) {
      launchError.value = "This game appears to be a Windows executable that can't run natively on Linux. Check that Proton is configured in Settings and the game's platform is set correctly.";
    } else if (errMsg.includes("NoCompat") || errMsg.includes("compatibility layer")) {
      launchError.value = "No Proton compatibility layer found. Set a default Proton path in Settings or add an override for this game.";
    } else if (errMsg.includes("InvalidPlatform")) {
      launchError.value = "This game can't be played on the current platform. It may need a compatibility layer like Proton.";
    } else {
      launchError.value = `Launch error: ${errMsg}`;
    }
  }
}

async function killGame() {
  try {
    await invoke("kill_game", { id: gameId });
    // If we were streaming, stop the server-side session too
    if (heartbeatInterval) { clearInterval(heartbeatInterval); heartbeatInterval = null; }
    if (activeStreamSessionId) {
      try {
        await stopStreamingSession(activeStreamSessionId);
      } catch (e) {
        console.warn("[BPM:STREAM] Failed to stop streaming session:", e);
      }
      activeStreamSessionId = null;
      isStreaming.value = false;
      streamGuard = false;
    }
  } catch (e) {
    console.error("Failed to stop game:", e);
  }
}

/**
 * Download/install the game.
 * Fetches version options to find the best version, then starts the download.
 */
async function downloadGame() {
  try {
    // Need version options to know what to download
    if (!versionOptions.value || versionOptions.value.length === 0) {
      versionOptions.value = await invoke<VersionOption[]>(
        "fetch_game_version_options",
        { gameId },
      );
    }

    if (!versionOptions.value || versionOptions.value.length === 0) {
      launchError.value = "No downloadable versions available for this game.";
      return;
    }

    // Pick the first (latest) version option
    const vo = versionOptions.value[0];

    // Get available install directories
    const installDirs = await invoke<string[]>("fetch_download_dir_stats");
    const installDir = 0; // Default to first directory

    await invoke("download_game", {
      gameId,
      versionId: vo.versionId,
      installDir,
      targetPlatform: vo.platform,
      enableUpdates: true,
    });
  } catch (e) {
    console.error("Failed to start download:", e);
    launchError.value = `Download failed: ${e instanceof Error ? e.message : String(e)}`;
  }
}

// ── Add to Library (without installing) ─────────────────────────────────

const inLibrary = ref(false);
const libraryLoading = ref(false);

// Check if this game is already in the user's library on mount
onMounted(async () => {
  try {
    const url = serverUrl("api/v1/collection/default");
    const res = await fetch(url);
    if (res.ok) {
      const collection = await res.json();
      const entries = collection.entries ?? [];
      inLibrary.value = entries.some((e: any) => e.gameId === gameId);
    }
  } catch (e) {
    console.warn("[BPM:GAME] Failed to check library status:", e);
  }
});

async function addToLibrary() {
  if (libraryLoading.value || inLibrary.value) return;
  libraryLoading.value = true;
  try {
    const url = serverUrl("api/v1/collection/default/entry");
    const res = await fetch(url, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ id: gameId }),
    });
    if (res.ok) {
      inLibrary.value = true;
    } else {
      console.error("[BPM:GAME] Failed to add to library:", res.status);
    }
  } catch (e) {
    console.error("[BPM:GAME] Add to library error:", e);
  } finally {
    libraryLoading.value = false;
  }
}

function openStore() {
  navigateTo(`/store/${gameId}`);
}

// ── Add to Steam ────────────────────────────────────────────────────────

const addedToSteam = ref(false);
const steamLoading = ref(false);

async function addToSteam() {
  if (steamLoading.value || addedToSteam.value) return;
  steamLoading.value = true;
  try {
    const g = game.value;
    const result = await invoke<{ success: boolean; message: string }>(
      "add_game_to_steam",
      {
        gameId,
        gameName: g?.mName ?? "Unknown Game",
        bannerObjectId: g?.mBannerObjectId || null,
        coverObjectId: g?.mCoverObjectId || null,
        iconObjectId: g?.mIconObjectId || null,
      },
    );
    console.log("[BPM:GAME] Add to Steam result:", result);
    if (result.success) {
      addedToSteam.value = true;
    } else {
      launchError.value = result.message;
    }
  } catch (e) {
    console.error("[BPM:GAME] Add to Steam failed:", e);
    launchError.value = `Failed to add to Steam: ${e instanceof Error ? e.message : String(e)}`;
  } finally {
    steamLoading.value = false;
  }
}

async function checkForUpdates() {
  try {
    await invoke("check_for_updates", { gameId });
  } catch (e) {
    console.error("Failed to check for updates:", e);
  }
}
</script>

<style scoped>
.dropdown-fade-enter-active,
.dropdown-fade-leave-active {
  transition: all 0.15s ease;
}
.dropdown-fade-enter-from,
.dropdown-fade-leave-to {
  opacity: 0;
  transform: translateY(-4px) scale(0.98);
}
</style>
