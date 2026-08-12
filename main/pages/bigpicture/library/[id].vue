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
          class="text-lg text-zinc-400 max-w-4xl mb-6"
          style="text-shadow: 0 1px 4px rgba(0,0,0,0.8)"
        >
          {{ game.mShortDescription }}
        </p>

        <!-- Action buttons -->
        <div class="flex items-center gap-3">
          <!-- ── Installed: Play button with dropdown ── -->
          <div v-if="status?.type === 'Installed' && status.install_type.type === 'Installed'" class="relative inline-flex">
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
                <!-- One flat list: row 0 plays here, the rest are other
                     devices. The index a row renders at is the index its
                     action is looked up by, so there is no arithmetic to get
                     wrong when a device joins or drops off the list. A device
                     that is not reachable stays visible but disabled, so it is
                     clear where it went. -->
                <template v-for="(item, i) in playMenuItems" :key="item.key">
                  <button
                    class="flex items-center gap-3 w-full px-6 py-3.5 text-left text-base transition-colors"
                    :class="playMenuRowClass(item, i)"
                    :disabled="item.disabled"
                    @click="selectPlayMenuAction(i)"
                    @mouseenter="!item.disabled && (playMenuFocus = i)"
                  >
                    <PlayIcon v-if="item.kind === 'play-local'" class="size-5" />
                    <SignalIcon v-else-if="item.kind === 'stream'" class="size-5 text-purple-400" />
                    <ArrowDownTrayIcon v-else class="size-5 text-green-400" />
                    <span class="font-medium">{{ item.label }}</span>
                    <span v-if="item.detail" class="text-xs opacity-50 ml-auto">{{ item.detail }}</span>
                  </button>
                </template>
                <!-- Divider + message if no other devices -->
                <div
                  v-if="playMenuItems.length === 1"
                  class="px-6 py-3 text-sm text-zinc-500 border-t border-zinc-800/50"
                >
                  No other devices registered
                </div>
              </div>
            </Transition>
            <!-- closePlayMenu(), not `playMenuOpen = false`: setting the flag
                 raw skips unwiring the menu's gamepad handlers and releasing
                 the input lock, which silences every non-bypass subscriber in
                 the app for the rest of the session. -->
            <div v-if="playMenuOpen" class="fixed inset-0 z-40" @click="closePlayMenu" />
          </div>

          <!-- ── Installed + PartiallyInstalled: Resume button ── -->
          <button
            v-else-if="status?.type === 'Installed' && status.install_type.type === 'PartiallyInstalled'"
            :ref="(el: any) => registerAction(el, { onSelect: resumePartialDownload })"
            class="inline-flex items-center px-8 py-4 text-lg gap-3 bg-blue-600 hover:bg-blue-500 text-white font-semibold rounded-xl transition-colors shadow-lg"
            @click="resumePartialDownload"
          >
            <ArrowDownTrayIcon class="size-6" />
            Resume
          </button>

          <!-- ── Installed + SetupRequired: Setup button ── -->
          <button
            v-else-if="status?.type === 'Installed' && status.install_type.type === 'SetupRequired'"
            :ref="(el: any) => registerAction(el, { onSelect: launchGame })"
            class="inline-flex items-center px-8 py-4 text-lg gap-3 bg-yellow-600 hover:bg-yellow-500 text-white font-semibold rounded-xl transition-colors shadow-lg"
            @click="launchGame"
          >
            <WrenchIcon class="size-6" />
            Setup
          </button>

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

          <!-- Launch status line — visible during launch and for the first
               few moments after the game is Running (until user dismisses). -->
          <div
            v-if="launchStatus"
            class="inline-flex items-center gap-2 px-4 py-2 rounded-lg bg-blue-950/60 border border-blue-500/30 text-blue-200 text-sm font-medium"
          >
            <span class="h-3 w-3 rounded-full border-2 border-blue-300/40 border-t-blue-300 animate-spin" />
            {{ launchStatus }}
          </div>

          <!-- ── Downloading/Queued: Status ── -->
          <button
            v-if="status?.type === 'Downloading' || status?.type === 'Queued'"
            class="inline-flex items-center px-8 py-4 text-lg gap-3 font-semibold rounded-xl cursor-not-allowed"
            style="background-color: rgba(59,130,246,0.2); color: rgb(147,197,253)"
            disabled
          >
            <ArrowDownTrayIcon class="size-6 animate-bounce" />
            {{ status?.type === "Downloading" ? "Downloading..." : "Queued" }}
          </button>

          <!-- ── Not installed here, but installed on another device:
               offer to play it there and stream it back. This is the
               "play a PC game on the Deck without installing" path. -->
          <div
            v-if="showRemotePlayButton && onlineStreamTargets.length > 0"
            :ref="(el: any) => registerAction(el, { onSelect: () => streaming.streamFromDevice(onlineStreamTargets[0]?.device) })"
            class="bp-focus-delegate inline-flex cursor-pointer"
          >
            <span class="bp-focus-ring inline-flex rounded-xl">
              <button
                class="inline-flex items-center px-8 py-4 text-lg gap-3 bg-purple-600 hover:bg-purple-500 text-white font-semibold rounded-xl transition-all shadow-lg shadow-purple-600/20 hover:scale-105"
                @click.stop="streaming.streamFromDevice(onlineStreamTargets[0]?.device)"
              >
                <SignalIcon class="size-6" />
                Play on {{ onlineStreamTargets[0].device.name }}
              </button>
            </span>
          </div>
          <!-- The game lives on a device that is switched off. Saying so beats
               both a button that times out and a button that vanished. -->
          <div
            v-else-if="showRemotePlayButton && streamTargets.length > 0"
            class="inline-flex items-center px-6 py-4 text-base gap-3 rounded-xl bg-zinc-800/60 text-zinc-400 border border-zinc-700/50"
          >
            <SignalIcon class="size-5 text-zinc-500" />
            {{ streamTargets[0].device.name }} is offline
          </div>

          <!-- ── Not installed: Install button with device picker.
               Explicit condition (not v-else) because launchStatus
               breaks the chain above — we must NOT render Install
               when the game is already Installed / Running / in
               flight, or we end up with two buttons side by side. -->
          <div
            v-if="
              status &&
              status.type !== 'Installed' &&
              status.type !== 'Running' &&
              status.type !== 'Downloading' &&
              status.type !== 'Queued'
            "
            class="relative inline-flex"
          >
            <div
              :ref="(el: any) => registerAction(el, { onSelect: downloadGame, onContext: hasRemoteInstallTargets ? togglePlayMenu : undefined })"
              class="bp-focus-delegate inline-flex cursor-pointer"
            >
              <span class="bp-focus-ring inline-flex rounded-xl">
                <button
                  class="inline-flex items-center pl-8 py-4 text-lg gap-3 bg-green-600 hover:bg-green-500 text-white font-semibold transition-all shadow-lg"
                  :class="hasRemoteInstallTargets ? 'pr-4 rounded-l-xl' : 'pr-8 rounded-xl'"
                  @click.stop="downloadGame"
                >
                  <ArrowDownTrayIcon class="size-6" />
                  Install
                </button>
                <button
                  v-if="hasRemoteInstallTargets"
                  class="inline-flex items-center px-3 py-4 font-semibold rounded-r-xl transition-all shadow-lg border-l bg-green-600 hover:bg-green-500 text-white border-green-400/30"
                  @click.stop="togglePlayMenu"
                >
                  <ChevronDownIcon class="size-5" :class="{ 'rotate-180': playMenuOpen }" />
                </button>
              </span>
            </div>
            <!-- Dropdown: install on other devices that don't have it. Same
                 flat-list rule as the play menu — row index is action index. -->
            <Transition name="dropdown-fade">
              <div
                v-if="playMenuOpen && hasRemoteInstallTargets"
                class="absolute left-0 top-full mt-2 z-50 min-w-[280px] rounded-xl bg-zinc-900 border border-zinc-700/50 shadow-2xl overflow-hidden"
              >
                <template v-for="(item, i) in installMenuItems" :key="item.key">
                  <button
                    class="flex items-center gap-3 w-full px-6 py-3.5 text-left text-base transition-colors"
                    :class="installMenuRowClass(item, i)"
                    :disabled="item.disabled"
                    @click="selectInstallMenuAction(i)"
                    @mouseenter="!item.disabled && (playMenuFocus = i)"
                  >
                    <ArrowDownTrayIcon
                      class="size-5"
                      :class="item.kind === 'install-local' ? '' : 'text-green-400'"
                    />
                    <span class="font-medium">{{ item.label }}</span>
                    <span v-if="item.detail" class="text-xs opacity-50 ml-auto">{{ item.detail }}</span>
                  </button>
                </template>
              </div>
            </Transition>
            <!-- closePlayMenu(), not `playMenuOpen = false`: setting the flag
                 raw skips unwiring the menu's gamepad handlers and releasing
                 the input lock, which silences every non-bypass subscriber in
                 the app for the rest of the session. -->
            <div v-if="playMenuOpen" class="fixed inset-0 z-40" @click="closePlayMenu" />
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

            <button
              :ref="(el: any) => registerAction(el, { onSelect: toggleCrtShader })"
              class="inline-flex items-center gap-1.5 px-4 py-3 text-sm rounded-xl transition-colors backdrop-blur-sm"
              :class="[
                crtShaderEnabled
                  ? 'bg-amber-600/80 hover:bg-amber-500 text-white'
                  : 'bg-zinc-800/80 hover:bg-zinc-700 text-zinc-300',
              ]"
              @click="toggleCrtShader"
              :title="`CRT Shader: ${crtShaderEnabled ? 'On' : 'Off'}`"
            >
              <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="size-4" :class="crtShaderEnabled ? 'text-white' : 'text-amber-400'">
                <rect x="3" y="4" width="18" height="13" rx="1.5" />
                <path d="M8 21h8M12 17v4" />
                <path d="M6 8h12M6 11h12M6 14h12" stroke-width="1" opacity="0.5" />
              </svg>
              <span class="font-medium">CRT</span>
            </button>

            <button
              :ref="(el: any) => registerAction(el, { onSelect: openRaCheatsheet })"
              class="inline-flex items-center gap-1.5 px-4 py-3 text-sm bg-zinc-800/80 hover:bg-zinc-700 text-zinc-300 rounded-xl transition-colors backdrop-blur-sm"
              @click="openRaCheatsheet"
              title="Controller shortcuts for RetroArch"
            >
              <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="size-4 text-emerald-400">
                <rect x="2" y="8" width="20" height="10" rx="3" />
                <path d="M7 13h2M15 13h2M12 11v4" />
              </svg>
              <span class="font-medium">Controls</span>
            </button>
          </template>

          <!-- Game Options. Start opens the same modal, but Start is a
               controller-only accelerator advertised by one small glyph in
               the context bar, and there was no on-screen target at all —
               Configure, Uninstall, Remove from Library and Add to Shelf all
               lived behind it. -->
          <button
            :ref="(el: any) => registerAction(el, { onSelect: () => (showOptions = true) })"
            class="inline-flex items-center gap-1.5 px-4 py-3 text-sm bg-zinc-800/80 hover:bg-zinc-700 text-zinc-300 rounded-xl transition-colors backdrop-blur-sm"
            title="Game options"
            @click="showOptions = true"
          >
            <Cog6ToothIcon class="size-4 text-zinc-400" />
            <span class="font-medium">Options</span>
          </button>

          <!-- Stream status / stop button (when streaming is active) -->
          <button
            v-if="isStreaming"
            :ref="(el: any) => registerAction(el, { onSelect: stopStreaming })"
            class="inline-flex items-center gap-2 px-4 py-3 text-sm rounded-lg transition-colors"
            :class="streamingPhase === 'streaming' ? 'text-red-400 hover:text-red-300 hover:bg-red-900/30' : 'text-purple-400 hover:text-purple-300 hover:bg-purple-900/30'"
            @click="stopStreaming"
          >
            <span class="size-2 rounded-full animate-pulse" :class="streamingPhase === 'streaming' ? 'bg-red-400' : 'bg-purple-400'" />
            {{ streamingPhaseLabel || 'Streaming' }}
          </button>
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

    <!-- Friends tile — Server users who've played this game. Sits between
         the stats row and the tab strip, mirroring the desktop layout. -->
    <div v-if="game" class="px-8 pt-3">
      <GameFriendsTile
        :game-id="gameId"
        :players="gamePlayers"
        :register-action="registerAction"
      />
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
      <!-- Community — achievements list + leaderboard, matching desktop's
           Community tab (which stacks the achievement grid above the
           leaderboard / first-to-unlock signal). -->
      <div v-if="activeTab === 'community'" class="space-y-4">
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
            <!-- No onSelect: focus-nav falls back to el.click() on the
                 anchor, which is exactly what a mouse does. -->
            <a
              v-for="h in romHashResult.expected_hashes?.filter((h) => h.patchUrl)"
              :key="h.hash"
              :ref="(el: any) => registerAction(el)"
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
              :class="gameFirstsMap[achievement.id] ? 'ring-2 ring-yellow-500/70' : ''"
              referrerpolicy="no-referrer"
              loading="lazy"
              @error="onAchievementIconError"
            />
            <div
              v-if="!achievement.iconUrl"
              class="size-12 rounded-lg bg-zinc-800 flex items-center justify-center"
              :class="gameFirstsMap[achievement.id] ? 'ring-2 ring-yellow-500/70' : ''"
            >
              <TrophyIcon
                class="size-6"
                :class="achievement.unlocked ? 'text-yellow-400' : 'text-zinc-600'"
              />
            </div>
            <div class="flex-1 min-w-0">
              <div class="flex items-center gap-1 min-w-0">
                <p class="text-sm font-medium text-zinc-200 truncate">
                  {{ achievement.title }}
                </p>
                <!-- Server-first marker — small gold trophy after the title. -->
                <GameAchievementFirstBadge
                  v-if="gameFirstsMap[achievement.id]"
                  :first="gameFirstsMap[achievement.id]"
                  class="shrink-0"
                />
              </div>
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

        <!-- Leaderboard / activity / first-to-unlock — the shared component
             desktop shows alongside achievements in its Community tab. -->
        <div class="pt-5 mt-1 border-t border-zinc-800/50">
          <GameCommunityTab
            :game-id="gameId"
            :players="gamePlayers"
            :firsts="gameFirsts"
          />
        </div>
      </div>

      <!-- About — description + gallery, matching desktop's About tab. Two
           columns on a wide screen: the old single `max-w-3xl` column left the
           description at roughly a third of a 1080p Big Picture shell. The
           description caps in CHARACTERS, not viewport width, so it stays
           readable rather than tracking the monitor. -->
      <div
        v-else-if="activeTab === 'about'"
        :class="
          hasGallery
            ? 'grid gap-8 xl:grid-cols-[minmax(0,1fr)_480px]'
            : 'space-y-8'
        "
      >
        <div class="min-w-0">
          <div
            v-if="game?.mDescription"
            ref="descriptionEl"
            class="bpm-description prose prose-lg prose-invert prose-zinc max-w-[75ch] text-zinc-300 leading-relaxed"
            v-html="renderedDescription"
            @click="openDescriptionImage"
          />
          <p v-else class="text-zinc-500">No description available.</p>
        </div>

        <div v-if="hasGallery" class="min-w-0">
          <h3 class="text-sm font-semibold mb-3" style="color: var(--bpm-muted)">
            GALLERY
          </h3>
          <GameDetailGallery
            :image-ids="game?.mImageCarouselObjectIds ?? []"
            :game-name="game?.mName ?? ''"
            :register-action="registerAction"
          />
        </div>
      </div>

      <!-- Mods — the same install / uninstall actions as the desktop Mods tab,
           as a card grid. Hidden entirely for games with no mods. -->
      <BpmModsTab
        v-else-if="activeTab === 'mods'"
        :mods="mods"
        :game-name="game?.mName ?? ''"
        :register-action="registerAction"
      />

      <!-- Cloud Saves — cloud list (restore/delete) + local save management
           (upload/download), matching desktop's single Cloud Saves tab. -->
      <div v-else-if="activeTab === 'cloudsaves'" class="space-y-6">
        <BpmCloudSavesPanel
          :game-id="gameId"
          :game-name="game?.mName ?? ''"
          :register-action="registerAction"
        />
        <BpmGameSavesTab
          :saves="saves"
          :is-native-game="isNativeGame"
          :register-action="registerAction"
          :format-time-ago="formatTimeAgo"
        />
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
          :ref="(el: any) => registerAction(el, { onSelect: () => goToRecommendation(rec.id) })"
        >
          <div class="bp-focus-ring rounded-lg overflow-hidden transition-transform hover:scale-105" style="aspect-ratio: 3/4">
            <img v-if="rec.mCoverObjectId" :src="objectUrl(rec.mCoverObjectId)" class="w-full h-full object-cover" loading="lazy" />
            <div v-else class="w-full h-full flex items-center justify-center bg-zinc-800 text-zinc-500 text-lg font-bold">{{ rec.mName[0] }}</div>
          </div>
          <p class="text-xs mt-1.5 truncate" style="color: var(--bpm-text)">{{ rec.mName }}</p>
        </div>
      </div>
    </div>

    <!-- Description images open in the same viewer as the gallery. -->
    <ImageLightbox
      :open="descriptionLightboxOpen"
      :srcs="descriptionImageSrcs"
      :start-index="descriptionImageIndex"
      :alt-prefix="`${game?.mName ?? 'Game'} image`"
      gamepad
      @close="descriptionLightboxOpen = false"
    />

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

    <!-- Neutral info toast (e.g. "remote install requested") -->
    <Transition
      enter-active-class="transition-all duration-200"
      leave-active-class="transition-all duration-300"
      enter-from-class="opacity-0 translate-y-4"
      leave-to-class="opacity-0 translate-y-4"
    >
      <div
        v-if="infoToast"
        class="fixed bottom-8 left-1/2 -translate-x-1/2 z-[200] max-w-lg px-6 py-3 rounded-xl text-sm font-medium shadow-lg backdrop-blur-md bg-zinc-900/90 text-zinc-100 border border-zinc-700/60"
      >
        {{ infoToast }}
      </div>
    </Transition>

    <!-- Launch error dialog -->
    <BigPictureDialog
      :visible="launchError !== null"
      :title="launchErrorTitle"
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

    <!-- Save action confirmation (cloud sync + local delete) -->
    <BigPictureDialog
      :visible="saves.confirmSyncAction.value !== null"
      :title="saveConfirmCopy.title"
      :message="saveConfirmCopy.message"
      :confirm-label="saveConfirmCopy.confirmLabel"
      cancel-label="Cancel"
      :destructive="saveConfirmCopy.destructive"
      @confirm="saves.confirmSync"
      @cancel="saves.confirmSyncAction.value = null"
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

    <!-- RetroArch controller cheatsheet -->
    <BpmRetroArchCheatsheet
      :open="raCheatsheetOpen"
      @close="closeRaCheatsheet"
    />
  </div>
</template>

<script setup lang="ts">
import { devLog } from "~/composables/dev-mode";
import BpmRetroArchCheatsheet from "~/components/bigpicture/BpmRetroArchCheatsheet.vue";
import { invoke } from "@tauri-apps/api/core";
import { platform } from "@tauri-apps/plugin-os";
import { useListen } from "~/composables/useListen";
import {
  PlayIcon,
  StopIcon,
  ArrowDownTrayIcon,
  TrophyIcon,
  SignalIcon,
} from "@heroicons/vue/24/solid";
import { ChevronDownIcon } from "@heroicons/vue/20/solid";
import { ClockIcon, Cog6ToothIcon, WrenchIcon } from "@heroicons/vue/24/outline";
import BigPictureDialog from "~/components/bigpicture/BigPictureDialog.vue";
import BigPictureButtonPrompt from "~/components/bigpicture/BigPictureButtonPrompt.vue";
import BigPictureKeyboard from "~/components/bigpicture/BigPictureKeyboard.vue";
import {
  useGame,
  type LaunchResult,
  type VersionOption,
} from "~/composables/game";
import { serverUrl } from "~/composables/use-server-fetch";
import {
  describeLaunchFailure,
  isBenignLaunchError,
} from "~/composables/launch-failure";
import { objectImageUrl } from "~/composables/use-object";
import { renderMarkdown } from "~/composables/render-markdown";
import type { Game, GameStatus, GameVersion } from "~/types";

function objectUrl(id: string): string {
  return objectImageUrl(id);
}

import { useBpFocusableGroup } from "~/composables/bp-focusable";
import { useFocusNavigation } from "~/composables/focus-navigation";
import { GamepadButton, useGamepad } from "~/composables/gamepad";
import { useStreaming } from "~/composables/useStreaming";
import { useDeckMode } from "~/composables/deck-mode";

definePageMeta({ layout: "bigpicture" });

devLog("state", "[BPM:GAME] >>> Script setup executing (synchronous) <<<");

const route = useRoute();
const gameId = route.params.id as string;
devLog("state", `[BPM:GAME] Route param gameId: ${gameId}`);

const game = ref<Game | null>(null);
const statusRef = shallowRef<any>(null);
const status = computed<GameStatus | null>(() => statusRef.value?.value ?? null);
const version = ref<GameVersion | null>(null);
const versionOptions = ref<VersionOption[] | null>(null);
const activeTab = ref("about");
// Plain object — NOT reactive. Storing DOM refs in a reactive ref causes
// infinite update loops when set from :ref callbacks during render.
const tabRefs: Record<string, HTMLElement | null> = {};
const tabIndicatorStyle = ref({ left: "0", width: "0" });
const launchError = ref<string | null>(null);
// Headline for the launch-error dialog. Every failure used to arrive under the
// same "Launch Failed", which buried the one thing the user could act on.
const launchErrorTitle = ref("Launch Failed");
const diagnosticsRan = ref(false);

// ── Streaming ─────────────────────────────────────────────────────────────
// Receiver-side streaming + cross-device discovery lives in
// `use-bpm-game-streaming.ts` — decomposed out of this page. The composable
// owns every interval and tears them down in `dispose()`.
import { useBpmGameStreaming } from "~/composables/bigpicture/use-bpm-game-streaming";
import {
  nextEnabledIndex,
  type PlayMenuItem,
} from "~/composables/bigpicture/stream-targets";
import type { ClientDevice } from "~/composables/useStreaming";

const streaming = useBpmGameStreaming(
  gameId,
  version,
  (msg) => { launchError.value = msg; },
  (msg) => showInfoToast(msg),
);
const {
  isStreaming,
  streamingPhase,
  streamingPhaseLabel,
  streamTargets,
  installTargets,
  onlineStreamTargets,
  playMenuItems,
  installMenuItems,
  stopStreaming,
} = streaming;

const playMenuOpen = ref(false);
const playMenuFocus = ref(0);
let playMenuLockId = 0;

/** The remote-play slot only exists while the game is not in play here. */
const showRemotePlayButton = computed(() => {
  const type = status.value?.type;
  return (
    !!type &&
    type !== "Installed" &&
    type !== "Running" &&
    type !== "Downloading" &&
    type !== "Queued"
  );
});
const hasRemoteInstallTargets = computed(() => installTargets.value.length > 0);

/** Rows the menu currently renders — the one source both halves index into. */
const activeMenuItems = computed(() =>
  status.value?.type === "Installed" ? playMenuItems.value : installMenuItems.value,
);

/**
 * Would the template draw a dropdown at all, `playMenuOpen` aside?
 *
 * This mirrors the two dropdown `v-if`s and is the one place that condition is
 * written. Opening the menu refetches the device list and takes the global
 * input lock before that fetch lands, so a fetch that fails or comes back with
 * no remote targets can remove every row while the lock is still held: no menu
 * on screen, and D-pad and A dead for the whole page. The same divergence
 * happens if the game's status changes while the menu is open, since both
 * dropdowns are gated on it too.
 */
const playMenuRenderable = computed(() => {
  if (status.value?.type === "Installed") {
    return status.value.install_type.type === "Installed";
  }
  return showRemotePlayButton.value && hasRemoteInstallTargets.value;
});

/**
 * Whether the shoulder button has a menu to open. A one-row menu is nothing to
 * open, and without the render check the button can take the input lock for a
 * dropdown no branch of the template is drawing (a game mid-download, or one
 * that only needs Setup).
 */
const playMenuAvailable = computed(
  () => activeMenuItems.value.length > 1 && playMenuRenderable.value,
);

// Close for real (unwire + release the lock) the moment the menu stops being
// drawn. Nothing else in the page watches `playMenuOpen`, so without this the
// only way out is a B press the user has no reason to try.
watch(playMenuRenderable, (renderable) => {
  if (!renderable && playMenuOpen.value) closePlayMenu();
});

// The same refetch can shrink the list without emptying it. A focus index left
// over from the longer list makes A land on a row that is not there, so bring
// it back onto a real one.
watch(activeMenuItems, (items) => {
  if (!playMenuOpen.value) return;
  if (playMenuFocus.value >= items.length) {
    playMenuFocus.value = nextEnabledIndex(items, playMenuFocus.value, -1);
  }
});

function menuRowClass(item: PlayMenuItem, index: number, activeClass: string) {
  if (item.disabled) return "text-zinc-500 cursor-not-allowed";
  return playMenuFocus.value === index
    ? activeClass
    : "text-zinc-300 hover:bg-zinc-800";
}
const ROW_ACTIVE_CLASS: Record<PlayMenuItem["kind"], string> = {
  "play-local": "bg-blue-600 text-white",
  "install-local": "bg-green-600 text-white",
  stream: "bg-purple-600 text-white",
  "install-remote": "bg-green-600 text-white",
};
function playMenuRowClass(item: PlayMenuItem, index: number) {
  return menuRowClass(item, index, ROW_ACTIVE_CLASS[item.kind]);
}
function installMenuRowClass(item: PlayMenuItem, index: number) {
  return menuRowClass(item, index, "bg-green-600 text-white");
}

function openPlayMenu() {
  // A focus registration keeps the first `onContext` closure it was given for
  // the element's whole life, so the Install button can still call this after
  // its remote targets have gone. Taking the lock for a dropdown no branch of
  // the template draws is what leaves the page with dead input.
  if (!playMenuRenderable.value) return;
  playMenuOpen.value = true;
  playMenuFocus.value = 0;
  // Re-read the device list on open: whether a device is reachable is the
  // difference between a working entry and a dead one, and the list was
  // fetched when the page mounted.
  void streaming.loadDevices();
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

/**
 * Run the row at `index`. The row carries its own action, so a device joining
 * or leaving the list between render and press cannot shift one entry's press
 * onto another entry's device.
 */
function runMenuItem(item: PlayMenuItem | undefined) {
  if (!item || item.disabled) return;
  switch (item.kind) {
    case "play-local":
      launchGame();
      break;
    case "install-local":
      downloadGame();
      break;
    case "stream":
      streaming.streamFromDevice(item.device as ClientDevice);
      break;
    case "install-remote":
      streaming.installOnDevice(item.device as ClientDevice);
      break;
  }
}

function selectPlayMenuAction(index: number) {
  const item = playMenuItems.value[index];
  if (!item || item.disabled) return;
  closePlayMenu();
  runMenuItem(item);
}

function selectInstallMenuAction(index: number) {
  const item = installMenuItems.value[index];
  if (!item || item.disabled) return;
  closePlayMenu();
  runMenuItem(item);
}

const _playMenuUnsubs: (() => void)[] = [];
function wirePlayMenuGamepad() {
  unwirePlayMenuGamepad();
  // Bounds are read at press time, not wire time: the device list can change
  // while the menu is open, and a length captured here would go stale.
  const bypass = { bypassInputLock: true };
  const move = (direction: 1 | -1) => {
    if (!playMenuOpen.value) return;
    playMenuFocus.value = nextEnabledIndex(
      activeMenuItems.value,
      playMenuFocus.value,
      direction,
    );
  };
  _playMenuUnsubs.push(
    gamepad.onButton(GamepadButton.DPadUp, () => move(-1), bypass),
    gamepad.onButton(GamepadButton.DPadDown, () => move(1), bypass),
    gamepad.onButton(GamepadButton.South, () => {
      if (playMenuOpen.value) {
        if (status.value?.type === "Installed") {
          selectPlayMenuAction(playMenuFocus.value);
        } else {
          selectInstallMenuAction(playMenuFocus.value);
        }
      }
    }, bypass),
    gamepad.onButton(GamepadButton.East, () => { if (playMenuOpen.value) closePlayMenu(); }, bypass),
  );
}
function unwirePlayMenuGamepad() {
  for (const u of _playMenuUnsubs) u();
  _playMenuUnsubs.length = 0;
}

/** Run launch diagnostics and log to console for debug capture */
async function runDiagnostics() {
  if (diagnosticsRan.value) return;
  diagnosticsRan.value = true;
  try {
    const diag = await invoke("diagnose_launch_environment");
    devLog("launch", "[BPM:DIAG] === LAUNCH DIAGNOSTICS ===");
    devLog("launch", "[BPM:DIAG] UMU installed:", (diag as any).umu_installed, "path:", (diag as any).umu_path);
    devLog("launch", "[BPM:DIAG] Proton default:", (diag as any).proton_default, "valid:", (diag as any).proton_default_valid);
    devLog("launch", "[BPM:DIAG] Proton autodiscovered:", (diag as any).proton_autodiscovered);
    devLog("launch", "[BPM:DIAG] Session:", (diag as any).session_type, "gamescope:", (diag as any).gamescope_detected);
    devLog("launch", "[BPM:DIAG] Env:", { display: (diag as any).env_display, wayland: (diag as any).env_wayland, gamescope: (diag as any).env_gamescope, xdg: (diag as any).env_xdg_runtime });
    devLog("launch", "[BPM:DIAG] Installed games:", (diag as any).installed_games);
    devLog("launch", "[BPM:DIAG] === END DIAGNOSTICS ===");
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

// ── Markdown rendering ──────────────────────────────────────────────────
// `renderMarkdown` is the shared helper in `composables/render-markdown.ts`.
const renderedDescription = computed(() =>
  game.value?.mDescription ? renderMarkdown(game.value.mDescription) : "",
);

const hasGallery = computed(
  () => (game.value?.mImageCarouselObjectIds?.length ?? 0) > 0,
);

// ── Description images ──────────────────────────────────────────────────
// Markdown images land in a `v-html` sink as inert <img> tags, and they can't
// be marked up on the way in: `sanitize.ts` runs DOMPurify with
// ALLOW_DATA_ATTR:false, so a data-* hook would be stripped. Delegate off the
// wrapper instead and read the rendered <img> list back out of the DOM.
const descriptionEl = ref<HTMLElement | null>(null);
const descriptionImageSrcs = ref<string[]>([]);
const descriptionImageIndex = ref(0);
const descriptionLightboxOpen = ref(false);

function openDescriptionImage(event: MouseEvent) {
  const target = event.target as HTMLElement | null;
  if (!target || target.tagName !== "IMG") return;
  const wrapper = descriptionEl.value;
  if (!wrapper) return;
  const images = Array.from(wrapper.querySelectorAll("img"));
  const index = images.indexOf(target as HTMLImageElement);
  if (index < 0) return;
  descriptionImageSrcs.value = images.map((img) => img.currentSrc || img.src);
  descriptionImageIndex.value = index;
  descriptionLightboxOpen.value = true;
}

// ── Game type detection ─────────────────────────────────────────────────
const isEmulatedGame = computed(() => {
  const ver = version.value;
  if (!ver?.launches) return false;
  // ALL launches must have a REAL emulator reference (a truthy `gameId`). A PC
  // game's launch can carry an empty/placeholder emulator object that a bare
  // `!= null` check wrongly treats as emulated.
  return (
    ver.launches.length > 0 && ver.launches.every((l) => !!l.emulator?.gameId)
  );
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

// ── Settings + info toasts ───────────────────────────────────────────────
const settingsToast = ref("");
let toastTimer: ReturnType<typeof setTimeout> | null = null;

function showSettingsToast(msg: string) {
  settingsToast.value = msg;
  if (toastTimer) clearTimeout(toastTimer);
  toastTimer = setTimeout(() => { settingsToast.value = ""; }, 2000);
}

// Generic information toast — separate from settingsToast because that one
// tacks on "Applied on next launch" which only makes sense for launch-config
// changes. Used for remote-install acknowledgements and other neutral
// confirmations that aren't errors but shouldn't block with a dialog.
const infoToast = ref("");
let infoToastTimer: ReturnType<typeof setTimeout> | null = null;

function showInfoToast(msg: string) {
  infoToast.value = msg;
  if (infoToastTimer) clearTimeout(infoToastTimer);
  infoToastTimer = setTimeout(() => { infoToast.value = ""; }, 5000);
}

// ── Per-game emulator/launch presets ─────────────────────────────────────
// Controller/quality/aspect/CRT presets + persistence live in
// `use-bpm-game-config.ts` — decomposed out of this page. `applyProfileName`
// here closes the options menu before delegating to the composable.
import { useBpmGameConfig } from "~/composables/bigpicture/use-bpm-game-config";

const gameConfig = useBpmGameConfig(
  gameId,
  version,
  showSettingsToast,
  (msg) => { launchError.value = msg; },
);
const {
  selectedController,
  selectedQuality,
  aspectRatio,
  crtShaderEnabled,
  fullscreen,
  controllerLabel,
  qualityLabel,
  aspectLabel,
  mangohudLabel,
  protonLabel,
  protonOptions,
  cycleController,
  cycleQuality,
  toggleWidescreen,
  toggleCrtShader,
  toggleFullscreen,
  cycleMangohud,
  cycleProton,
} = gameConfig;

// Proton override is only meaningful for Windows games launched on a Linux
// host — on Windows / macOS the `fetch_proton_paths` Tauri command isn't
// registered and the override field is ignored by the launcher.
const isLinuxHost = computed(() => platform() === "linux");

function applyProfileName() {
  showOptions.value = false;
  gameConfig.applyProfileName();
}

async function openInstallFolder() {
  showOptions.value = false;
  try {
    await invoke("open_game_install_dir", { gameId });
  } catch (e) {
    console.error("[BPM] open install folder failed:", e);
  }
}

// ── Manual VC++ runtime install ──────────────────────────────────────────
// Runs winetricks against this game's Proton prefix on demand. Progress shows
// on the existing launchStatus line (the game_prep_status listener picks up the
// "Installing Visual C++ runtime..." message); we clear it in finally because
// the BPM listener only reacts to active=true, not the active=false done edge.
const installingVc = ref(false);
async function installRuntimes() {
  showOptions.value = false;
  if (installingVc.value) return;
  installingVc.value = true;
  try {
    await invoke("install_redists", {
      gameId,
      runtimes: ["vcpp", "directx"],
    });
    showInfoToast(
      "Runtimes installed (VC++ + DirectX). Launch the game again if it was failing.",
    );
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    launchError.value = `Couldn't install runtimes: ${msg}`;
  } finally {
    installingVc.value = false;
    launchStatus.value = null;
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
    items.push({
      id: "fullscreen",
      label: "Fullscreen",
      valueLabel: (fullscreen.value ?? true) ? "On" : "Off",
      action: toggleFullscreen,
    });
  }

  // MangoHud is a Linux perf overlay for any game (not emulator-specific).
  if (isLinuxHost.value && status.value?.type === "Installed") {
    items.push({
      id: "mangohud",
      label: "MangoHud",
      valueLabel: mangohudLabel.value,
      action: cycleMangohud,
    });
  }

  if (isNativeGame.value && isWindowsGame.value && status.value?.type === "Installed") {
    items.push({
      id: "profile",
      label: "Set Account Name",
      action: applyProfileName,
    });
  }

  // Per-game Proton override. Linux host only; surfaced only when there's
  // more than the default option (i.e. at least one auto-discovered or
  // user-added Proton path), so we don't clutter the menu with a cycler
  // that has nothing to cycle through on systems without Proton-GE etc.
  if (
    isNativeGame.value &&
    isWindowsGame.value &&
    status.value?.type === "Installed" &&
    isLinuxHost.value &&
    protonOptions.value.length > 1
  ) {
    items.push({
      id: "proton",
      label: "Proton Version",
      valueLabel: protonLabel.value,
      action: cycleProton,
    });
  }

  // VC++ runtime — Windows game on a Linux host (Proton) only.
  if (
    isWindowsGame.value &&
    isLinuxHost.value &&
    status.value?.type === "Installed"
  ) {
    items.push({
      id: "install-runtimes",
      label: "Install runtimes",
      action: installRuntimes,
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

  if (status.value?.type === "Installed") {
    items.push({
      id: "open-install-folder",
      label: "Open install folder",
      action: openInstallFolder,
    });
  }

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

  const bypass = { bypassInputLock: true };

  _optionsSubs.push(
    gamepad.onButton(GamepadButton.DPadUp, () => {
      if (!showOptions.value) return;
      optionsFocusIdx.value = Math.max(0, optionsFocusIdx.value - 1);
    }, bypass),
  );
  _optionsSubs.push(
    gamepad.onButton(GamepadButton.DPadDown, () => {
      if (!showOptions.value) return;
      optionsFocusIdx.value = Math.min(
        optionsMenuItems.value.length - 1,
        optionsFocusIdx.value + 1,
      );
    }, bypass),
  );
  _optionsSubs.push(
    gamepad.onButton(GamepadButton.South, () => {
      if (!showOptions.value) return;
      const item = optionsMenuItems.value[optionsFocusIdx.value];
      if (item) item.action();
    }, bypass),
  );
  _optionsSubs.push(
    gamepad.onButton(GamepadButton.East, () => {
      if (!showOptions.value) return;
      showOptions.value = false;
    }, bypass),
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
let shelfLockId = 0;

function wireShelfGamepad() {
  unwireShelfGamepad();
  const totalItems = shelvesData.shelves.value.length + 2; // shelves + Create button + Done button
  const bypass = { bypassInputLock: true };
  // These handlers bypass the input lock, so they keep firing when the
  // on-screen keyboard opens on top of the picker — and the keyboard's own
  // handlers bypass too. Without this guard one D-pad press moves the
  // keyboard's cursor AND the row highlight behind it.
  const pickerHasInput = () =>
    showShelfPicker.value && !showNewShelfKeyboard.value;
  _shelfSubs.push(
    gamepad.onButton(GamepadButton.DPadUp, () => {
      if (!pickerHasInput()) return;
      shelfFocusIdx.value = Math.max(0, shelfFocusIdx.value - 1);
    }, bypass),
    gamepad.onButton(GamepadButton.DPadDown, () => {
      if (!pickerHasInput()) return;
      shelfFocusIdx.value = Math.min(totalItems - 1, shelfFocusIdx.value + 1);
    }, bypass),
    gamepad.onButton(GamepadButton.South, () => {
      if (!pickerHasInput()) return;
      const idx = shelfFocusIdx.value;
      const shelfCount = shelvesData.shelves.value.length;
      if (idx < shelfCount) {
        toggleGameOnShelf(shelvesData.shelves.value[idx].id);
      } else if (idx === shelfCount) {
        showNewShelfKeyboard.value = true; // Create New Shelf button
      } else {
        showShelfPicker.value = false; // Done button
      }
    }, bypass),
    gamepad.onButton(GamepadButton.East, () => {
      if (!pickerHasInput()) return;
      showShelfPicker.value = false;
    }, bypass),
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

// ── Mods ─────────────────────────────────────────────────────────────────
// A mod is a Game (type=Mod) whose files overlay into this game's install dir.
// The state and every action come from the shared game-detail layer via
// `useBpmMods`; the tab markup is <BpmModsTab>, which takes this object.
import { useBpmMods } from "~/composables/bigpicture/use-bpm-mods";
import BpmModsTab from "~/components/bigpicture/game-detail/BpmModsTab.vue";
import {
  shouldShowModsTab,
  resolveActiveTab,
} from "~/composables/game-detail/mods-tab";

const mods = useBpmMods(gameId);

// Matching the desktop game-detail page: About (description + gallery),
// Community (achievements + leaderboard), Mods, and Cloud Saves.
//
// Mods is hidden for games with no mods — an empty tab was just a dead end.
// `shouldShowModsTab` owns the loading rule and is shared with desktop, so the
// two surfaces can't drift on when the tab appears.
const tabs = computed(() => {
  const all = [
    { label: "About", value: "about" },
    { label: "Community", value: "community" },
    { label: "Mods", value: "mods" },
    { label: "Cloud Saves", value: "cloudsaves" },
  ];
  return all.filter((t) =>
    t.value === "mods"
      ? shouldShowModsTab({
          installed: status.value?.type === "Installed",
          loaded: mods.loaded.value,
          availableCount: mods.available.value.length,
          installedCount: mods.installedMods.value.length,
        })
      : true,
  );
});

// Uninstalling the last mod (or the game) can pull the current tab out from
// under the user, which otherwise leaves the panel area blank.
watch(tabs, (visible) => {
  activeTab.value = resolveActiveTab(
    visible.map((t) => t.value),
    activeTab.value,
    "about",
  );
});

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

// ── Save data (emulator + cloud + PC/Ludusavi) ───────────────────────────
// All save logic — local saves, cloud sync, PC-game saves via Ludusavi,
// the merged view, and the formatters — lives in `use-bpm-game-saves.ts`,
// decomposed out of this page. The Saves-tab markup is the
// <BpmGameSavesTab> component, which takes this `saves` object as a prop.
import { useBpmGameSaves } from "~/composables/bigpicture/use-bpm-game-saves";
import BpmGameSavesTab from "~/components/bigpicture/game-detail/BpmGameSavesTab.vue";
import BpmCloudSavesPanel from "~/components/bigpicture/BpmCloudSavesPanel.vue";

// ── Community surfaces ───────────────────────────────────────────────────
// Per-game players + first-to-unlock are fetched once and shared between
// the Friends tile, the Community tab body, and the Achievements list
// (which gets a gold-ring border around any achievement someone unlocked
// first on this server). All three soft-fail to empty.
import GameFriendsTile from "~/components/GameFriendsTile.vue";
import GameCommunityTab from "~/components/GameCommunityTab.vue";
import GameAchievementFirstBadge from "~/components/GameAchievementFirstBadge.vue";
import {
  useServerApi,
  type GamePlayerEntry,
  type GameAchievementFirst,
} from "~/composables/use-server-api";

const communityApi = useServerApi();
const gamePlayers = ref<GamePlayerEntry[]>([]);
const gameFirsts = ref<GameAchievementFirst[]>([]);
const gameFirstsMap = computed(() => {
  const m: Record<string, GameAchievementFirst> = {};
  for (const f of gameFirsts.value) m[f.achievementId] = f;
  return m;
});
onMounted(() => {
  communityApi.community
    .gamePlayers(gameId)
    .then((p) => (gamePlayers.value = p))
    .catch(() => (gamePlayers.value = []));
  communityApi.community
    .gameFirsts(gameId)
    .then((f) => (gameFirsts.value = f))
    .catch(() => (gameFirsts.value = []));
});

const saves = useBpmGameSaves(
  gameId,
  computed(() => game.value?.mName),
  isNativeGame,
  (msg) => { launchError.value = msg; },
);

/**
 * Copy for the save-action confirmation dialog. Delete is the destructive one:
 * every Saves-tab button is a gamepad action, so a stray A-press on a focused
 * row reaches it, and it unlinks a real save file.
 */
const saveConfirmCopy = computed(() => {
  const action = saves.confirmSyncAction.value;
  const name = action?.filename ?? "";
  if (action?.type === "delete") {
    return {
      title: "Delete This Save?",
      message: `This deletes '${name}' from this device. Drop keeps a timestamped backup next to it, but the game will no longer see this save.`,
      confirmLabel: "Delete Save",
      destructive: true,
    };
  }
  if (action?.type === "upload") {
    return {
      title: "Replace Cloud Save?",
      message: `This will replace the cloud version of '${name}' with your local copy. A backup of the current cloud version will be saved automatically.`,
      confirmLabel: "Replace Cloud Save",
      destructive: false,
    };
  }
  return {
    title: "Replace Local Save?",
    message: `This will replace your local copy of '${name}' with the cloud version. A backup of your current local save will be created automatically.`,
    confirmLabel: "Replace Local Save",
    destructive: false,
  };
});

// Load saves when the Cloud Saves tab is selected. Mods re-reads on open too,
// so the grid reflects on-disk state even if an install-completion event was
// missed while the page sat on another tab.
watch(
  () => activeTab.value,
  (tab) => {
    if (tab === "cloudsaves") saves.loadAll();
    if (tab === "mods") mods.refresh(status.value?.type === "Installed");
  },
);

// The available list drives the tab's visibility, so it's fetched on mount
// regardless of install state. The on-disk ledger only exists once the base
// game is installed, and `status` arrives after the game fetch resolves — so
// the installed half is (re)read whenever the game becomes installed.
onMounted(() => {
  mods.refresh(status.value?.type === "Installed");
});
watch(
  () => status.value?.type === "Installed",
  (installed) => {
    if (installed) mods.refreshInstalled();
  },
);

// A completed mod install/uninstall emits update_library; re-read the ledgers
// so the grid updates without leaving the page.
useListen("update_library", () => {
  if (status.value?.type === "Installed") mods.refreshInstalled();
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

// Live-refresh the achievement list/count when the backend reports an unlock,
// so the Community tab updates in place instead of only after re-navigating
// (the unlock toast already fires). The event carries no gameId, so any unlock
// triggers a cheap refetch of this game's achievements.
async function reloadAchievements() {
  try {
    const res = await fetch(serverUrl(`api/v1/games/${gameId}/achievements`));
    if (!res.ok) return;
    const r = await res.json();
    achievements.value = Array.isArray(r) ? r : (r.achievements ?? []);
  } catch {
    // best-effort; ignore refresh failures
  }
}
useListen("achievement_unlocked", () => {
  reloadAchievements();
});

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
useListen<RomHashResult>(`ra_hash_check/${gameId}`, (event) => {
  romHashResult.value = event.payload;
});

// Cloud save conflicts are handled app-wide by `SaveSyncConflictHost`
// (mounted in app.vue), which picks the Big Picture dialog when BPM is
// active. Quick-launch from the grid never had a listener here.

// ── RetroArch controller cheatsheet ─────────────────────────────────────
const raCheatsheetOpen = ref(false);
function openRaCheatsheet() {
  raCheatsheetOpen.value = true;
}
function closeRaCheatsheet() {
  raCheatsheetOpen.value = false;
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
  devLog("state", `[BPM:GAME] === Page mounted for gameId: ${gameId} ===`);
  devLog("state", `[BPM:GAME] Route: ${route.fullPath}`);

  // Wire up gamepad immediately — don't wait for data to load
  const { isGamescope: _pageIsGs } = useDeckMode();
  // Physical X button — maps to West on normal controllers, North under gamescope.
  // Opens the play/stream dropdown regardless of focus so users can launch or
  // stream even when focus has drifted elsewhere on the page.
  const _playMenuBtn = _pageIsGs.value ? GamepadButton.North : GamepadButton.West;
  _unsubs.push(
    gamepad.onButton(GamepadButton.Start, () => {
      showOptions.value = true;
    }),
    gamepad.onButton(_playMenuBtn, () => {
      // Focus-nav also routes this button to the focused element's onContext.
      // When the Play button itself is focused, onContext = togglePlayMenu
      // already ran this tick — running it again here would flip it back
      // closed on a single press.
      if (focusNav.contextHandled.value) return;
      if (showOptions.value) return;
      if (!playMenuAvailable.value) return;
      togglePlayMenu();
    }),
  );

  // Listen for external launch errors (process crashes / wrong binary format)
  const { listen } = await import("@tauri-apps/api/event");
  const unlistenLaunchTrace = await listen("launch_trace", (event) => {
    const p = event.payload as any;
    devLog("launch", `[BPM:TRACE:${p.step}]`, JSON.stringify(p, null, 2));
    // Surface BIOS warnings to the user so they know why a game crashed
    if (p.step === "7_retroarch_config_result" && p.bios_warnings?.length) {
      launchError.value = p.bios_warnings.join("\n");
    }
  });
  _unsubs.push(() => unlistenLaunchTrace());

  const unlistenLaunchError = await listen("launch_external_error", (event) => {
    if (event.payload === gameId) {
      console.error("[BPM:GAME] External launch error for:", gameId);
      launchError.value = "The game may have failed to launch. Check the game's compatibility. Windows games require Proton/UMU on Linux.";
      runDiagnostics();
    }
  });
  _unsubs.push(() => unlistenLaunchError());

  // Remote install requests are handled globally in state-navigation.ts
  // to avoid duplicate downloads. No page-level listener needed.

  devLog("state", "[BPM:GAME] Gamepad wired. Starting data fetch...");

  // Fire all fetches in parallel — apply results as each resolves instead
  // of waiting for all (avoids a slow fetch blocking the entire page).

  const achievementsUrl = serverUrl(`api/v1/games/${gameId}/achievements`);
  devLog("state", "[BPM:GAME] Achievements URL:", achievementsUrl);

  // Game data — needed for the page header, status, and config
  // useGame is a local Tauri invoke (usually cached) — 5s is generous
  const gamePromise = withTimeout(useGame(gameId), 5000)
    .then((r) => {
      if (!r) { console.warn("[BPM:GAME] useGame TIMED OUT or null"); return; }
      devLog("state", "[BPM:GAME] useGame resolved:", r.game?.mName ?? "null");
      game.value = r.game;
      statusRef.value = r.status;
      version.value = r.version?.value ?? null;
      devLog("state", "[BPM:GAME] Game loaded:", r.game.mName, "| Status:", r.status?.value);
      // Seed the preset refs (controller/quality/aspect/CRT) from the
      // freshly-loaded version — see use-bpm-game-config.ts.
      gameConfig.syncFromVersion(version.value);
    })
    .catch((e) => console.error("[BPM:GAME] useGame FAILED:", e));

  // Version options — can arrive late without blocking the page
  const versionPromise = invoke<VersionOption[]>("fetch_game_version_options", { gameId })
    .then((r) => {
      devLog("state", "[BPM:GAME] version_options resolved:", r?.length ?? 0, "options");
      if (r) versionOptions.value = r;
    })
    .catch((e) => console.warn("[BPM:GAME] version_options failed:", e));

  // Achievements — server:// proxied fetch, 5s timeout
  const achievementsPromise = withTimeout(
    fetch(achievementsUrl).then((res) => {
      devLog("state", "[BPM:GAME] achievements fetch status:", res.status);
      return res.ok ? res.json() : null;
    }),
    5000,
  )
    .then((r) => {
      if (!r) { console.warn("[BPM:GAME] achievements timed out or null"); return; }
      achievements.value = Array.isArray(r) ? r : (r.achievements ?? []);
      devLog("state", "[BPM:GAME] Achievements loaded:", achievements.value.length);
      if (achievements.value.length > 0) {
        const sample = achievements.value.slice(0, 3);
        for (const a of sample) {
          devLog("state", `[BPM:GAME] Achievement "${a.title}" iconUrl: ${a.iconUrl || "(empty)"}`);
        }
      }
    })
    .catch((e) => console.warn("[BPM:GAME] achievements fetch FAILED:", e));

  // Wait for the critical data (game + achievements) before setting up focus.
  // version_options, playtime, and recommendations are intentionally NOT awaited.
  fetchPlaytime();
  fetchRecommendations();
  await Promise.all([gamePromise, achievementsPromise]);
  devLog("state", "[BPM:GAME] Critical data loaded, versions pending:", !versionOptions.value);

  devLog("state", "[BPM:GAME] Setting up focus...");
  nextTick(() => updateTabIndicator());
  focusNav.autoFocusContent("content");
  devLog("state", "[BPM:GAME] === Page setup complete ===");
});

function _onResize() {
  updateTabIndicator();
}
onMounted(() => {
  window.addEventListener("resize", _onResize);
  // Load other devices for the dropdown + start receiver-side stream polling.
  streaming.loadDevices();
  streaming.startPolling();
});

onUnmounted(() => {
  for (const unsub of _unsubs) unsub();
  _unsubs.length = 0;
  // Every overlay on this page can be open at unmount time, and each holds
  // its own input lock. Missing one leaves the app's gamepad input locked.
  unwireOptionsGamepad();
  unwirePlayMenuGamepad();
  unwireShelfGamepad();
  if (showOptions.value) focusNav.releaseInputLock(optionsLockId);
  if (playMenuOpen.value) focusNav.releaseInputLock(playMenuLockId);
  if (showShelfPicker.value) focusNav.releaseInputLock(shelfLockId);
  window.removeEventListener("resize", _onResize);
  // Tear down every streaming interval owned by the composable.
  streaming.dispose();
});

function dismissLaunchError() {
  launchError.value = null;
  launchErrorTitle.value = "Launch Failed";
}

const launchInFlight = ref(false);
const launchStatus = ref<string | null>(null);

function stepLabel(step: string): string | null {
  // Map backend launch_trace step IDs to short user-facing labels.
  if (step.startsWith("1_")) return "Preparing...";
  if (step.startsWith("2_")) return "Reading game config...";
  if (step.startsWith("3_")) return "Selecting compatibility layer...";
  if (step.startsWith("4_")) return "Setting up runtime...";
  if (step.startsWith("5_")) return "Building command...";
  if (step.startsWith("6_")) return "Finalizing...";
  if (step.startsWith("7b_")) return "Checking ROM...";
  if (step.startsWith("7c_") || step.startsWith("7d_")) return "Syncing saves...";
  if (step.startsWith("7_")) return "Configuring emulator...";
  if (step.startsWith("8_")) return "Launching...";
  return null;
}

useListen<{ step: string; game_id: string }>("launch_trace", (event) => {
  if (event.payload.game_id !== gameId) return;
  if (!launchInFlight.value && status.value?.type !== "Running") return;
  const label = stepLabel(event.payload.step);
  if (label) launchStatus.value = label;
});

// Precise prep status from the backend during a slow, blocking one-time
// prefix-prep step (e.g. installing the VC++ runtime via winetricks before a
// umu/Proton launch). Reuses the same launchStatus line as launch_trace — the
// message wins while prep is active, and the watch below clears it once the
// launch settles. Linux-only by nature (the backend only emits it on Proton
// launches).
useListen<{ gameId: string; active: boolean; message: string }>(
  "game_prep_status",
  (event) => {
    if (event.payload.gameId !== gameId) return;
    if (event.payload.active) launchStatus.value = event.payload.message;
  },
);

// Clear the transient status line when running-state settles or launch errors.
watch([launchInFlight, () => status.value?.type], ([inFlight, type]) => {
  if (!inFlight && type !== "Running") launchStatus.value = null;
});

async function launchGame() {
  // Without this guard, mashing A during launch fires multiple
  // `invoke("launch_game")` calls in parallel — the backend may accept one
  // and reject the rest with `AlreadyRunning`, which surfaces as a scary
  // error dialog over a game that is actually starting correctly.
  if (launchInFlight.value) return;
  launchInFlight.value = true;
  try {
    const result: LaunchResult = await invoke("launch_game", {
      id: gameId,
      index: 0,
    });
    if (result.result === "InstallRequired") {
      // Auto-download the required dependency (e.g. the emulator). There is no
      // install-directory picker in Big Picture, so this is the only way a pad
      // user can get out of the dead end — but say *what* is being installed:
      // "a required dependency" told nobody they were missing RetroArch.
      // Resolve the dependency's own platform rather than reusing the host
      // game's platform — Proton/Wine builds are Linux-only, and a Windows
      // game would otherwise ask for a "windows" build of the dep.
      const { gameId: depGameId, versionId: depVersionId, name } = result.data;
      const depName = name ?? "the program it needs";
      const gameName = game.value?.mName ?? "This game";
      launchErrorTitle.value = "Emulator not installed";
      try {
        const depVersions = await invoke<VersionOption[]>(
          "fetch_game_version_options",
          { gameId: depGameId },
        );
        const depVersion =
          depVersions?.find((v) => v.versionId === depVersionId)
          ?? depVersions?.[0];
        if (!depVersion) {
          throw new Error("no downloadable versions on your Drop server");
        }
        await invoke("download_game", {
          gameId: depGameId,
          versionId: depVersion.versionId,
          installDir: 0,
          targetPlatform: depVersion.platform,
          enableUpdates: true,
        });
        launchError.value = `${gameName} runs through ${depName}, which isn't installed. Drop is downloading ${depName} now. Press Play again once it finishes.`;
      } catch (depErr) {
        launchError.value = `${gameName} runs through ${depName}, which isn't installed, and Drop couldn't start the download: ${depErr instanceof Error ? depErr.message : String(depErr)}. Install ${depName} from your library on the desktop app.`;
      }
    }
    // LaunchResult is `Success | InstallRequired`. Anything other than
    // those two would surface as a thrown error from `invoke("launch_game")`,
    // caught below — no need to second-guess the discriminator.
  } catch (e) {
    console.error("[BPM:GAME] Launch error:", e);
    // Benign — Drop already has this game running. Clear any error the user
    // might be staring at so the "Stop" state is all they see.
    if (isBenignLaunchError(e)) {
      launchError.value = null;
      return;
    }
    // Auto-run diagnostics on any launch failure for debug logs
    runDiagnostics();
    const failure = describeLaunchFailure(e, game.value?.mName);
    launchErrorTitle.value = failure.title;
    launchError.value = failure.message;
  } finally {
    launchInFlight.value = false;
  }
}

async function killGame() {
  try {
    // `game_id`, not `id` — the wrong key failed argument deserialisation and
    // Stop looked like it was ignored.
    await invoke("kill_game", { gameId });
    // If we were streaming, stop everything (heartbeats, Sunshine, server sessions)
    if (streaming.hasActiveStream) {
      await stopStreaming();
    }
  } catch (e) {
    console.error("Failed to stop game:", e);
    launchErrorTitle.value = "Couldn't stop the game";
    launchError.value = `Drop couldn't stop ${game.value?.mName ?? "this game"}: ${e instanceof Error ? e.message : String(e)}`;
  }
}

/**
 * Download/install the game.
 * Fetches version options to find the best version, then starts the download.
 */
async function resumePartialDownload() {
  try {
    await invoke("resume_download", { gameId });
  } catch (e) {
    console.error("Failed to resume download:", e);
    launchError.value = `Resume failed: ${e instanceof Error ? e.message : String(e)}`;
  }
}

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

function goToRecommendation(recId: string) {
  const target = `/bigpicture/library/${recId}`;
  // When jumping between recommended titles, a B press should return to
  // the game we just came from, not the default parent (library grid).
  focusNav.setRouteState("backTo", `/bigpicture/library/${gameId}`, target);
  navigateTo(target);
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

/* Description images are clickable (delegated to the wrapper), so they need
   to look it. :deep because they come from a v-html sink. */
.bpm-description :deep(img) {
  cursor: zoom-in;
}
</style>
