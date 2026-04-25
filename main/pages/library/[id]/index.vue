<template>
  <div
    class="mx-auto w-full relative flex flex-col justify-center pt-72 overflow-hidden"
  >
    <!-- Game options gear — pinned top-left above banner -->
    <Menu as="div" class="absolute top-3 left-4 z-20">
      <MenuButton
        class="rounded-lg p-1.5 bg-zinc-800/50 text-zinc-100 hover:bg-zinc-800 transition-colors"
      >
        <Cog6ToothIcon class="size-5" />
      </MenuButton>
      <Transition
        enter-active-class="transition ease-out duration-100"
        enter-from-class="opacity-0 scale-95"
        enter-to-class="opacity-100 scale-100"
        leave-active-class="transition ease-in duration-75"
        leave-from-class="opacity-100 scale-100"
        leave-to-class="opacity-0 scale-95"
      >
        <MenuItems
          class="absolute left-0 z-[500] mt-2 w-56 origin-top-left rounded-lg bg-zinc-900 shadow-lg ring-1 ring-zinc-100/5 focus:outline-none overflow-hidden"
        >
          <div class="py-1">
            <MenuItem v-if="isNativeGame" v-slot="{ active }">
              <button
                @click="applyProfileName"
                :class="[
                  active
                    ? 'bg-zinc-800 text-zinc-100 outline-none'
                    : 'text-zinc-400',
                  'w-full px-4 py-2 text-sm inline-flex justify-between',
                ]"
              >
                Set Account Name
                <UserIcon class="size-5 text-green-400" />
              </button>
            </MenuItem>
            <!-- Controller layout selector — only for emulated (RetroArch) games -->
            <MenuItem v-if="isEmulatedGame" as="div" disabled>
              <div class="w-full px-4 py-2 text-sm text-zinc-300">
                <div class="flex justify-between items-center mb-1.5">
                  <span>Controller Layout</span>
                  <AdjustmentsHorizontalIcon class="size-4 text-blue-400" />
                </div>
                <div class="flex gap-1">
                  <button
                    v-for="opt in controllerOptions"
                    :key="opt.value"
                    class="flex-1 px-2 py-1 rounded text-xs font-medium transition-colors"
                    :class="
                      selectedController === opt.value
                        ? 'bg-blue-600 text-white'
                        : 'bg-zinc-700 text-zinc-400 hover:bg-zinc-600 hover:text-zinc-200'
                    "
                    @click.stop="setController(opt.value)"
                  >
                    {{ opt.label }}
                  </button>
                </div>
              </div>
            </MenuItem>
            <!-- Quality preset selector — only for emulated (RetroArch) games -->
            <MenuItem v-if="isEmulatedGame" as="div" disabled>
              <div class="w-full px-4 py-2 text-sm text-zinc-300">
                <div class="flex justify-between items-center mb-1.5">
                  <span>Quality Preset</span>
                  <SparklesIcon class="size-4 text-purple-400" />
                </div>
                <div class="flex gap-1">
                  <button
                    v-for="opt in qualityOptions"
                    :key="opt.value"
                    class="flex-1 px-2 py-1 rounded text-xs font-medium transition-colors"
                    :class="
                      selectedQuality === opt.value
                        ? 'bg-purple-600 text-white'
                        : 'bg-zinc-700 text-zinc-400 hover:bg-zinc-600 hover:text-zinc-200'
                    "
                    @click.stop="setQuality(opt.value)"
                  >
                    {{ opt.label }}
                  </button>
                </div>
              </div>
            </MenuItem>
            <!-- Aspect ratio cycle — only for emulated (RetroArch) games -->
            <MenuItem v-if="isEmulatedGame" as="div" disabled>
              <div class="w-full px-4 py-2 text-sm text-zinc-300">
                <div class="flex justify-between items-center">
                  <span>Aspect Ratio</span>
                  <button
                    class="px-2.5 py-0.5 rounded-md text-xs font-medium transition-colors"
                    :class="aspectRatio !== 'Standard' ? 'bg-green-600 text-white' : 'bg-zinc-700 text-zinc-300'"
                    @click.stop="toggleWidescreen"
                  >
                    {{ aspectLabel }}
                  </button>
                </div>
              </div>
            </MenuItem>
            <div class="border-t border-zinc-800 my-1" />
            <MenuItem v-if="achievements.length > 0" v-slot="{ active }">
              <button
                @click="confirmResetAchievements"
                :class="[
                  active
                    ? 'bg-zinc-800 text-zinc-100 outline-none'
                    : 'text-zinc-400',
                  'w-full px-4 py-2 text-sm inline-flex justify-between',
                ]"
              >
                Reset Achievements
                <TrophyIcon class="size-5 text-yellow-500" />
              </button>
            </MenuItem>
          </div>
        </MenuItems>
      </Transition>
    </Menu>

    <div class="absolute inset-0 z-0">
      <img
        :src="bannerUrl"
        class="w-full h-[24rem] object-cover blur-sm scale-105"
      />
      <div
        class="absolute inset-0 bg-gradient-to-t from-zinc-900 via-zinc-900/80 to-transparent opacity-90"
      />
      <div
        class="absolute inset-0 bg-gradient-to-r from-zinc-900/95 via-zinc-900/80 to-transparent opacity-90"
      />
    </div>

    <div class="relative z-10">
      <div class="px-8">
        <h1
          class="text-5xl text-zinc-100 font-bold font-display drop-shadow-lg"
        >
          {{ game.mName }}
        </h1>
        <div
          v-if="
            status.type === 'Installed' &&
            status.install_type.type != InstalledType.PartiallyInstalled
          "
          class="mt-1"
        >
          <div
            v-if="!version?.userConfiguration?.enableUpdates"
            class="inline-flex items-center gap-x-1 text-xs text-zinc-400"
          >
            Version pinned
            <svg
              class="size-3 text-blue-600"
              viewBox="0 0 24 24"
              fill="none"
              xmlns="http://www.w3.org/2000/svg"
            >
              <path
                d="M19.1835 7.80516L16.2188 4.83755C14.1921 2.8089 13.1788 1.79457 12.0904 2.03468C11.0021 2.2748 10.5086 3.62155 9.5217 6.31506L8.85373 8.1381C8.59063 8.85617 8.45908 9.2152 8.22239 9.49292C8.11619 9.61754 7.99536 9.72887 7.86251 9.82451C7.56644 10.0377 7.19811 10.1392 6.46145 10.3423C4.80107 10.8 3.97088 11.0289 3.65804 11.5721C3.5228 11.8069 3.45242 12.0735 3.45413 12.3446C3.45809 12.9715 4.06698 13.581 5.28476 14.8L6.69935 16.2163L2.22345 20.6964C1.92552 20.9946 1.92552 21.4782 2.22345 21.7764C2.52138 22.0746 3.00443 22.0746 3.30236 21.7764L7.77841 17.2961L9.24441 18.7635C10.4699 19.9902 11.0827 20.6036 11.7134 20.6045C11.9792 20.6049 12.2404 20.5358 12.4713 20.4041C13.0192 20.0914 13.2493 19.2551 13.7095 17.5825C13.9119 16.8472 14.013 16.4795 14.2254 16.1835C14.3184 16.054 14.4262 15.9358 14.5468 15.8314C14.8221 15.593 15.1788 15.459 15.8922 15.191L17.7362 14.4981C20.4 13.4973 21.7319 12.9969 21.9667 11.9115C22.2014 10.826 21.1954 9.81905 19.1835 7.80516Z"
                fill="currentColor"
              />
            </svg>
          </div>
          <div
            v-else-if="!status.update_available"
            class="inline-flex items-center gap-x-1 text-xs text-zinc-400"
          >
            Up to date <CheckCircleIcon class="size-3 text-green-600" />
          </div>
          <div
            v-else-if="status.update_available"
            class="inline-flex items-center gap-x-1 text-xs text-zinc-400"
          >
            Update available <ArrowDownTrayIcon class="size-3 text-blue-600" />
          </div>
        </div>

        <div class="mt-3 flex flex-row gap-x-4 items-stretch">
          <!-- Do not add scale animations to this: https://stackoverflow.com/a/35683068 -->
          <GameStatusButton
            @install="() => installFlow()"
            @launch="() => launch()"
            @queue="() => queue()"
            @uninstall="() => uninstall()"
            @kill="() => kill()"
            @options="() => (configureModalOpen = true)"
            @resume="() => resumeDownload()"
            :status="status"
          />
          <StreamButton
            :game-id="game.id"
            :game-name="game.mName"
            :is-installed="status.type === 'Installed'"
          />
          <button
            v-if="status.type === 'Installed' && status.update_available"
            class="transition-transform duration-300 hover:scale-105 active:scale-95 inline-flex gap-x-2 items-center rounded-md bg-blue-600 px-6 font-semibold text-white shadow-xl backdrop-blur-sm hover:bg-blue-700 uppercase font-display"
            @click="() => installFlow()"
          >
            Update <ArrowDownTrayIcon class="size-5" />
          </button>
          <CompatTestButton
            :game-id="game.id"
            :is-installed="status.type === 'Installed'"
            @result="(outcome) => onCompatTestResult(outcome)"
          />
          <NuxtLink
            class="transition-transform duration-300 hover:scale-105 active:scale-95 inline-flex items-center rounded-md bg-zinc-800/50 px-6 font-semibold text-white shadow-xl backdrop-blur-sm hover:bg-zinc-800/80 uppercase font-display"
            :to="{
              path: '/store',
              query: {
                gameId: game.id,
              },
            }"
          >
            <BuildingStorefrontIcon class="mr-2 size-5" aria-hidden="true" />
            Store
          </NuxtLink>
        </div>
      </div>

      <!-- Stat bar -->
      <div
        v-if="!statsLoading"
        class="mt-6 mx-8 flex items-center gap-6 rounded-lg bg-zinc-800/60 backdrop-blur-sm px-6 py-3 border border-zinc-700/50"
      >
        <div
          v-if="gameStats.lastPlayedAt"
          class="flex items-center gap-2 text-sm"
        >
          <CalendarIcon class="size-4 text-zinc-400 shrink-0" />
          <span class="text-zinc-400">Last Played</span>
          <span class="text-zinc-100 font-medium">{{
            formatLastPlayed(gameStats.lastPlayedAt)
          }}</span>
        </div>
        <div v-if="gameStats.lastPlayedAt" class="w-px h-4 bg-zinc-600" />
        <div class="flex items-center gap-2 text-sm">
          <ClockIcon class="size-4 text-zinc-400 shrink-0" />
          <span class="text-zinc-400">Play Time</span>
          <span class="text-zinc-100 font-medium">{{
            formatPlaytime(gameStats.playtimeSeconds)
          }}</span>
        </div>
        <div
          v-if="gameStats.achievementsTotal > 0"
          class="w-px h-4 bg-zinc-600"
        />
        <div
          v-if="gameStats.achievementsTotal > 0"
          class="flex items-center gap-2 text-sm"
        >
          <TrophyIcon class="size-4 text-yellow-500 shrink-0" />
          <span class="text-zinc-400">Achievements</span>
          <span class="text-zinc-100 font-medium"
            >{{ gameStats.achievementsUnlocked }}/{{
              gameStats.achievementsTotal
            }}</span
          >
          <div class="w-24 h-1.5 bg-zinc-700 rounded-full overflow-hidden">
            <div
              class="h-full bg-blue-500 rounded-full transition-all duration-500"
              :style="{
                width:
                  Math.round(
                    (gameStats.achievementsUnlocked /
                      gameStats.achievementsTotal) *
                      100,
                  ) + '%',
              }"
            />
          </div>
        </div>
      </div>

      <!-- Main content -->
      <div class="mt-8 w-full bg-zinc-900 px-8">
        <div class="grid grid-cols-[2fr,1fr] gap-8">
          <div class="space-y-4">
            <!-- Collapsible Description -->
            <div
              class="bg-zinc-800/50 rounded-xl backdrop-blur-sm overflow-hidden"
            >
              <button
                @click="descriptionOpen = !descriptionOpen"
                class="w-full flex items-center justify-between p-6 text-left hover:bg-zinc-700/30 transition-colors"
              >
                <h2 class="text-xl font-display font-semibold text-zinc-100">
                  About
                </h2>
                <ChevronDownIcon
                  class="size-5 text-zinc-400 transition-transform duration-200"
                  :class="{ 'rotate-180': descriptionOpen }"
                />
              </button>
              <Transition
                enter-active-class="transition-all duration-300 ease-out"
                enter-from-class="max-h-0 opacity-0"
                enter-to-class="max-h-[2000px] opacity-100"
                leave-active-class="transition-all duration-200 ease-in"
                leave-from-class="max-h-[2000px] opacity-100"
                leave-to-class="max-h-0 opacity-0"
              >
                <div v-show="descriptionOpen" class="overflow-hidden">
                  <div class="px-6 pb-6">
                    <div
                      v-html="htmlDescription"
                      class="prose prose-invert prose-blue overflow-y-auto custom-scrollbar max-w-none"
                    ></div>
                  </div>
                </div>
              </Transition>
            </div>

            <!-- Future server-rendered sections can go here -->
          </div>

          <div class="space-y-6">
            <div class="bg-zinc-800/50 rounded-xl p-6 backdrop-blur-sm">
              <h2 class="text-xl font-display font-semibold text-zinc-100 mb-4">
                Game Images
              </h2>
              <div class="relative">
                <div v-if="game.mImageCarouselObjectIds.length > 0">
                  <div
                    class="relative aspect-video rounded-lg overflow-hidden cursor-pointer group"
                  >
                    <div
                      class="absolute inset-0"
                      @click="
                        fullscreenImage =
                          game.mImageCarouselObjectIds[currentImageIndex]
                      "
                    >
                      <TransitionGroup name="slide" tag="div" class="h-full">
                        <img
                          v-for="(url, index) in game.mImageCarouselObjectIds"
                          :key="url"
                          :src="useObject(url)"
                          class="absolute inset-0 w-full h-full object-cover"
                          v-show="index === currentImageIndex"
                        />
                      </TransitionGroup>
                    </div>

                    <div
                      class="absolute inset-0 flex items-center justify-between px-4 opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none"
                    >
                      <div class="pointer-events-auto">
                        <button
                          v-if="game.mImageCarouselObjectIds.length > 1"
                          @click.stop="previousImage()"
                          class="p-2 rounded-full bg-zinc-900/50 text-zinc-100 hover:bg-zinc-900/80 transition-all duration-300 hover:scale-110"
                        >
                          <ChevronLeftIcon class="size-5" />
                        </button>
                      </div>
                      <div class="pointer-events-auto">
                        <button
                          v-if="game.mImageCarouselObjectIds.length > 1"
                          @click.stop="nextImage()"
                          class="p-2 rounded-full bg-zinc-900/50 text-zinc-100 hover:bg-zinc-900/80 transition-all duration-300 hover:scale-110"
                        >
                          <ChevronRightIcon class="size-5" />
                        </button>
                      </div>
                    </div>

                    <div
                      class="absolute inset-0 bg-gradient-to-t from-black/50 to-transparent opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none"
                    />
                    <div
                      class="absolute bottom-4 right-4 flex items-center gap-x-2 text-white opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none"
                    >
                      <ArrowsPointingOutIcon class="size-5" />
                      <span class="text-sm font-medium">View Fullscreen</span>
                    </div>
                  </div>

                  <div
                    class="absolute -bottom-2 left-1/2 -translate-x-1/2 flex gap-x-2"
                  >
                    <button
                      v-for="(_, index) in game.mImageCarouselObjectIds"
                      :key="index"
                      @click.stop="currentImageIndex = index"
                      class="w-1.5 h-1.5 rounded-full transition-all"
                      :class="[
                        currentImageIndex === index
                          ? 'bg-zinc-100 scale-125'
                          : 'bg-zinc-600 hover:bg-zinc-500',
                      ]"
                    />
                  </div>
                </div>

                <div
                  v-else
                  class="aspect-video rounded-lg overflow-hidden bg-zinc-900/50 flex flex-col items-center justify-center text-center px-4"
                >
                  <PhotoIcon class="size-12 text-zinc-500 mb-2" />
                  <p class="text-zinc-400 font-medium">No images available</p>
                  <p class="text-zinc-500 text-sm">
                    Game screenshots will appear here when available
                  </p>
                </div>
              </div>
            </div>

            <!-- Achievements -->
            <div class="bg-zinc-800/50 rounded-xl p-6 backdrop-blur-sm">
              <div class="flex items-center justify-between mb-4">
                <h2
                  class="text-xl font-display font-semibold text-zinc-100"
                >
                  Achievements
                </h2>
              </div>

              <!-- ROM Hash Status Banner -->
              <div
                v-if="romHashResult?.status === 'Mismatch'"
                class="mb-4 rounded-lg bg-amber-500/10 p-3 outline outline-1 outline-amber-500/20"
              >
                <p class="text-sm font-medium text-amber-400 mb-1">
                  ROM not recognised by RetroAchievements
                </p>
                <p class="text-xs text-zinc-400 mb-2">
                  Your ROM hash
                  (<code class="text-zinc-300">{{
                    romHashResult.rom_hash?.slice(0, 12)
                  }}…</code>) doesn't match any known hash. Achievements won't
                  track until the ROM is patched or replaced.
                </p>
                <div
                  v-if="
                    romHashResult.expected_hashes?.some((h) => h.patchUrl)
                  "
                  class="flex flex-wrap gap-2"
                >
                  <a
                    v-for="h in romHashResult.expected_hashes?.filter(
                      (h) => h.patchUrl,
                    )"
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
                class="mb-4 rounded-lg bg-emerald-500/10 p-2 outline outline-1 outline-emerald-500/20"
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
                class="mb-4 rounded-lg bg-red-500/10 p-2 outline outline-1 outline-red-500/20"
              >
                <p class="text-xs text-red-400">
                  Hash check failed: {{ romHashResult.message }}
                </p>
              </div>

              <div v-if="achievementsLoading" class="flex justify-center py-4">
                <div
                  class="w-5 h-5 border-2 border-zinc-600 border-t-zinc-100 rounded-full animate-spin"
                />
              </div>
              <div
                v-else-if="achievements.length === 0"
                class="flex flex-col items-center justify-center text-center py-4"
              >
                <svg
                  class="size-10 text-zinc-600 mb-2"
                  xmlns="http://www.w3.org/2000/svg"
                  viewBox="0 0 24 24"
                  fill="currentColor"
                >
                  <path
                    fill-rule="evenodd"
                    d="M5.166 2.621v.858c-1.035.148-2.059.33-3.071.543a.75.75 0 0 0-.584.859 6.753 6.753 0 0 0 6.138 5.6 6.73 6.73 0 0 0 2.743 1.346A6.707 6.707 0 0 1 9.279 15H8.54c-1.036 0-1.875.84-1.875 1.875V19.5h-.75a2.25 2.25 0 0 0-2.25 2.25c0 .414.336.75.75.75h15.19a.75.75 0 0 0 .75-.75 2.25 2.25 0 0 0-2.25-2.25h-.75v-2.625c0-1.036-.84-1.875-1.875-1.875h-.739a6.707 6.707 0 0 1-1.112-3.173 6.73 6.73 0 0 0 2.743-1.347 6.753 6.753 0 0 0 6.139-5.6.75.75 0 0 0-.585-.858 47.077 47.077 0 0 0-3.07-.543V2.62a.75.75 0 0 0-.658-.744 49.22 49.22 0 0 0-6.093-.377c-2.063 0-4.096.128-6.093.377a.75.75 0 0 0-.657.744Zm0 2.629c0 1.196.312 2.32.857 3.294A5.266 5.266 0 0 1 3.16 5.337a45.6 45.6 0 0 1 2.006-.343v.256Zm13.5 0v-.256c.674.1 1.343.214 2.006.343a5.265 5.265 0 0 1-2.863 3.207 6.72 6.72 0 0 0 .857-3.294Z"
                    clip-rule="evenodd"
                  />
                </svg>
                <p class="text-zinc-500 text-sm">No achievements available</p>
              </div>
              <div
                v-else
                class="space-y-2 max-h-80 overflow-y-auto custom-scrollbar pr-1"
              >
                <div class="flex items-center justify-between mb-2">
                  <span class="text-xs text-zinc-400">
                    {{ achievementsUnlocked }} /
                    {{ achievements.length }} unlocked
                  </span>
                  <div
                    class="flex-1 ml-3 h-1.5 bg-zinc-700 rounded-full overflow-hidden"
                  >
                    <div
                      class="h-full bg-yellow-500 rounded-full transition-all"
                      :style="{
                        width: `${achievements.length > 0 ? (achievementsUnlocked / achievements.length) * 100 : 0}%`,
                      }"
                    />
                  </div>
                </div>
                <div
                  v-for="ach in achievements"
                  :key="ach.id"
                  class="flex items-center gap-3 py-2 px-2 rounded-lg hover:bg-zinc-700/30 transition-colors"
                >
                  <img
                    v-if="ach.iconUrl"
                    :src="ach.iconUrl"
                    :class="[
                      'size-9 rounded shrink-0',
                      ach.unlocked ? '' : 'grayscale opacity-50',
                    ]"
                  />
                  <div
                    v-else
                    :class="[
                      'size-9 rounded shrink-0 bg-zinc-700/50 flex items-center justify-center',
                      ach.unlocked ? '' : 'opacity-50',
                    ]"
                  >
                    <svg
                      class="size-5 text-zinc-500"
                      xmlns="http://www.w3.org/2000/svg"
                      viewBox="0 0 24 24"
                      fill="currentColor"
                    >
                      <path
                        fill-rule="evenodd"
                        d="M5.166 2.621v.858c-1.035.148-2.059.33-3.071.543a.75.75 0 0 0-.584.859 6.753 6.753 0 0 0 6.138 5.6 6.73 6.73 0 0 0 2.743 1.346A6.707 6.707 0 0 1 9.279 15H8.54c-1.036 0-1.875.84-1.875 1.875V19.5h-.75a2.25 2.25 0 0 0-2.25 2.25c0 .414.336.75.75.75h15.19a.75.75 0 0 0 .75-.75 2.25 2.25 0 0 0-2.25-2.25h-.75v-2.625c0-1.036-.84-1.875-1.875-1.875h-.739a6.707 6.707 0 0 1-1.112-3.173 6.73 6.73 0 0 0 2.743-1.347 6.753 6.753 0 0 0 6.139-5.6.75.75 0 0 0-.585-.858 47.077 47.077 0 0 0-3.07-.543V2.62a.75.75 0 0 0-.658-.744 49.22 49.22 0 0 0-6.093-.377c-2.063 0-4.096.128-6.093.377a.75.75 0 0 0-.657.744Zm0 2.629c0 1.196.312 2.32.857 3.294A5.266 5.266 0 0 1 3.16 5.337a45.6 45.6 0 0 1 2.006-.343v.256Zm13.5 0v-.256c.674.1 1.343.214 2.006.343a5.265 5.265 0 0 1-2.863 3.207 6.72 6.72 0 0 0 .857-3.294Z"
                        clip-rule="evenodd"
                      />
                    </svg>
                  </div>
                  <div class="flex-1 min-w-0">
                    <p
                      :class="[
                        'text-sm font-medium truncate',
                        ach.unlocked ? 'text-zinc-100' : 'text-zinc-500',
                      ]"
                    >
                      {{ ach.title }}
                    </p>
                    <p class="text-xs text-zinc-500 truncate">
                      {{ ach.description }}
                    </p>
                  </div>
                  <div v-if="ach.unlocked" class="shrink-0">
                    <CheckCircleIcon class="size-4 text-yellow-500" />
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>

  <ModalTemplate v-model="installFlowOpen">
    <template #default>
      <div class="sm:flex sm:items-start">
        <div class="mt-3 text-center sm:mt-0 sm:text-left">
          <h3 class="text-base font-semibold text-zinc-100">
            Install {{ game.mName }}?
          </h3>
          <div class="mt-2">
            <p class="text-sm text-zinc-400">
              Drop will add {{ game.mName }} to the queue to be downloaded.
              While downloading, Drop may use up a large amount of resources,
              particularly network bandwidth and CPU utilisation.
            </p>
          </div>
        </div>
      </div>

      <div class="space-y-6">
        <!-- Compatibility test results — only renders when this game has
             been tested on at least one of the user's devices. -->
        <GameCompatPanel :compat="gameCompat" />
        <div v-if="versionOptions && versionOptions.length > 0">
          <Listbox as="div" v-model="installVersionIndex">
            <ListboxLabel class="block text-sm/6 font-medium text-zinc-100"
              >Version</ListboxLabel
            >
            <div class="relative mt-2">
              <ListboxButton
                class="relative w-full cursor-default rounded-md bg-zinc-800 py-1.5 pl-3 pr-10 text-left text-zinc-100 shadow-sm ring-1 ring-inset ring-zinc-700 focus:outline-none focus:ring-2 focus:ring-blue-600 sm:text-sm/6"
              >
                <span class="block truncate">{{
                  formatVersionOptionText(installVersionIndex)
                }}</span>
                <span
                  class="pointer-events-none absolute inset-y-0 right-0 flex items-center pr-2"
                >
                  <ChevronUpDownIcon
                    class="h-5 w-5 text-gray-400"
                    aria-hidden="true"
                  />
                </span>
              </ListboxButton>

              <div
                v-if="installVersionIndex == -1"
                class="mt-3 rounded-md bg-blue-500/10 p-2 outline outline-blue-500/20"
              >
                <div class="flex">
                  <div class="shrink-0">
                    <InformationCircleIcon
                      class="size-4 text-blue-400"
                      aria-hidden="true"
                    />
                  </div>
                  <div class="ml-2 flex-1 md:flex md:justify-between">
                    <p class="text-xs text-blue-300">
                      "Latest" will notify you when there is a new version
                      available. Choose another version to pin this game's
                      version.
                    </p>
                  </div>
                </div>
              </div>
              <div
                v-else
                class="mt-3 rounded-md bg-blue-500/10 p-2 outline outline-blue-500/20"
              >
                <div class="flex">
                  <div class="shrink-0">
                    <InformationCircleIcon
                      class="size-4 text-blue-400"
                      aria-hidden="true"
                    />
                  </div>
                  <div class="ml-2 flex-1 md:flex md:justify-between">
                    <p class="text-xs text-blue-300">
                      This game will be pinned to "{{
                        currentVersionOption?.displayName ||
                        currentVersionOption?.versionPath
                      }}"
                    </p>
                  </div>
                </div>
              </div>

              <transition
                leave-active-class="transition ease-in duration-100"
                leave-from-class="opacity-100"
                leave-to-class="opacity-0"
              >
                <ListboxOptions
                  class="absolute z-10 mt-1 max-h-60 w-full overflow-auto rounded-md bg-zinc-900 py-1 text-base shadow-lg ring-1 ring-black ring-opacity-5 focus:outline-none sm:text-sm"
                >
                  <ListboxOption
                    as="template"
                    :value="-1"
                    v-slot="{ active, selected }"
                  >
                    <li
                      :class="[
                        active ? 'bg-blue-600 text-white' : 'text-zinc-300',
                        'relative cursor-default select-none py-2 pl-3 pr-9',
                      ]"
                    >
                      <span
                        :class="[
                          selected
                            ? 'font-semibold text-zinc-100'
                            : 'font-normal',
                          'block truncate',
                        ]"
                        >{{ formatVersionOptionText(-1) }}</span
                      >

                      <span
                        v-if="selected"
                        :class="[
                          active ? 'text-white' : 'text-blue-600',
                          'absolute inset-y-0 right-0 flex items-center pr-4',
                        ]"
                      >
                        <CheckIcon class="h-5 w-5" aria-hidden="true" />
                      </span>
                    </li>
                  </ListboxOption>

                  <ListboxOption
                    as="template"
                    v-for="(version, versionIdx) in versionOptions"
                    :key="version.versionId"
                    :value="versionIdx"
                    v-slot="{ active, selected }"
                  >
                    <li
                      :class="[
                        active ? 'bg-blue-600 text-white' : 'text-zinc-300',
                        'relative cursor-default select-none py-2 pl-3 pr-9',
                      ]"
                    >
                      <span
                        :class="[
                          selected
                            ? 'font-semibold text-zinc-100'
                            : 'font-normal',
                          'block truncate',
                        ]"
                        >{{ formatVersionOptionText(versionIdx) }}</span
                      >

                      <span
                        v-if="selected"
                        :class="[
                          active ? 'text-white' : 'text-blue-600',
                          'absolute inset-y-0 right-0 flex items-center pr-4',
                        ]"
                      >
                        <CheckIcon class="h-5 w-5" aria-hidden="true" />
                      </span>
                    </li>
                  </ListboxOption>
                </ListboxOptions>
              </transition>
            </div>
          </Listbox>
        </div>
        <div
          v-else-if="versionOptions === null || versionOptions?.length == 0"
          class="mt-1 rounded-md bg-red-600/10 p-4"
        >
          <div class="flex">
            <div class="flex-shrink-0">
              <XCircleIcon class="h-5 w-5 text-red-600" aria-hidden="true" />
            </div>
            <div class="ml-3">
              <h3 class="text-sm font-medium text-red-600">
                There are no supported versions to install. Please contact your
                server admin or try again later.
              </h3>
            </div>
          </div>
        </div>
        <div v-else class="w-full flex items-center justify-center p-4">
          <div role="status">
            <svg
              aria-hidden="true"
              class="w-7 h-7 text-transparent animate-spin fill-white"
              viewBox="0 0 100 101"
              fill="none"
              xmlns="http://www.w3.org/2000/svg"
            >
              <path
                d="M100 50.5908C100 78.2051 77.6142 100.591 50 100.591C22.3858 100.591 0 78.2051 0 50.5908C0 22.9766 22.3858 0.59082 50 0.59082C77.6142 0.59082 100 22.9766 100 50.5908ZM9.08144 50.5908C9.08144 73.1895 27.4013 91.5094 50 91.5094C72.5987 91.5094 90.9186 73.1895 90.9186 50.5908C90.9186 27.9921 72.5987 9.67226 50 9.67226C27.4013 9.67226 9.08144 27.9921 9.08144 50.5908Z"
                fill="currentColor"
              />
              <path
                d="M93.9676 39.0409C96.393 38.4038 97.8624 35.9116 97.0079 33.5539C95.2932 28.8227 92.871 24.3692 89.8167 20.348C85.8452 15.1192 80.8826 10.7238 75.2124 7.41289C69.5422 4.10194 63.2754 1.94025 56.7698 1.05124C51.7666 0.367541 46.6976 0.446843 41.7345 1.27873C39.2613 1.69328 37.813 4.19778 38.4501 6.62326C39.0873 9.04874 41.5694 10.4717 44.0505 10.1071C47.8511 9.54855 51.7191 9.52689 55.5402 10.0491C60.8642 10.7766 65.9928 12.5457 70.6331 15.2552C75.2735 17.9648 79.3347 21.5619 82.5849 25.841C84.9175 28.9121 86.7997 32.2913 88.1811 35.8758C89.083 38.2158 91.5421 39.6781 93.9676 39.0409Z"
                fill="currentFill"
              />
            </svg>
            <span class="sr-only">Loading...</span>
          </div>
        </div>
        <div v-if="installDirs">
          <InstallDirectorySelector
            :install-dirs="installDirs"
            v-model="installDir"
          />
        </div>
        <div
          v-if="
            currentVersionOption?.requiredContent &&
            currentVersionOption.requiredContent.length > 0
          "
        >
          <div class="border-b border-white/10 py-2">
            <h3 class="text-sm font-semibold text-white">
              Install additional dependencies?
            </h3>
            <p class="mt-1 text-xs text-gray-400">
              This game requires additional content to run. Click the components
              to automatically queue for download.
            </p>
          </div>
          <ul role="list" class="mt-2 divide-y divide-white/5">
            <li
              v-for="content in currentVersionOption.requiredContent"
              :key="content.versionId"
              :class="[
                !installDepsDisabled[content.versionId]
                  ? 'bg-zinc-950 ring-2 ring-zinc-800'
                  : '',
                'rounded-lg relative flex justify-between px-2 py-3',
              ]"
            >
              <div class="flex min-w-0 gap-x-2">
                <img
                  class="size-12 flex-none"
                  :src="useObject(content.iconObjectId)"
                  alt=""
                />
                <div class="min-w-0 flex-auto">
                  <p class="text-sm/6 font-semibold text-white">
                    <button
                      @click="
                        () =>
                          (installDepsDisabled[content.versionId] =
                            !installDepsDisabled[content.versionId])
                      "
                    >
                      <span class="absolute inset-x-0 -top-px bottom-0"></span>
                      {{ content.name }}
                    </button>
                  </p>
                  <p class="mt-1 flex text-xs/5 text-gray-400">
                    {{ content.shortDescription }}
                  </p>
                </div>
              </div>
              <div class="flex shrink-0 items-center gap-x-2">
                <div class="hidden sm:flex sm:flex-col sm:items-end">
                  <p
                    class="inline-flex items-center gap-x-1 text-xs/5 text-gray-400"
                  >
                    {{ formatKilobytes(content.size.installSize / 1024) }}B
                    <ServerIcon class="size-3" />
                  </p>
                </div>
                <CheckIcon
                  v-if="!installDepsDisabled[content.versionId]"
                  class="size-5 flex-none text-green-500"
                  aria-hidden="true"
                />
                <MinusIcon
                  v-else
                  class="size-5 flex-none text-gray-500"
                  aria-hidden="true"
                />
              </div>
            </li>
          </ul>
        </div>
      </div>

      <div v-if="installError" class="mt-1 rounded-md bg-red-600/10 p-4">
        <div class="flex">
          <div class="flex-shrink-0">
            <XCircleIcon class="h-5 w-5 text-red-600" aria-hidden="true" />
          </div>
          <div class="ml-3">
            <h3 class="text-sm font-medium text-red-600">
              {{ installError }}
            </h3>
          </div>
        </div>
      </div>
    </template>
    <template #buttons>
      <LoadingButton
        @click="() => install()"
        :disabled="!(versionOptions && versionOptions.length > 0)"
        :loading="installLoading"
        type="submit"
        class="ml-2 w-full sm:w-fit"
      >
        Install
      </LoadingButton>
      <button
        type="button"
        class="mt-3 inline-flex w-full justify-center rounded-md bg-zinc-800 px-3 py-2 text-sm font-semibold text-zinc-100 shadow-sm ring-1 ring-inset ring-zinc-700 hover:bg-zinc-900 sm:mt-0 sm:w-auto"
        @click="installFlowOpen = false"
        ref="cancelButtonRef"
      >
        Cancel
      </button>
    </template>
  </ModalTemplate>

  <ModalTemplate :model-value="launchOptionsOpen">
    <template #default>
      <div class="sm:flex sm:items-start">
        <div class="mt-3 text-center sm:mt-0 sm:text-left">
          <h3 class="text-base font-semibold text-zinc-100">
            Launch {{ game.mName }}
          </h3>
          <div class="mt-2">
            <p class="text-sm text-zinc-400">
              The instance admin has configured multiple ways to start this
              game. Select an option to start.
            </p>
          </div>
        </div>
      </div>

      <ol class="space-y-2">
        <li v-for="(launchData, launchIdx) in launchOptions!">
          <button
            class="transition w-full rounded-sm bg-zinc-800 inline-flex items-center text-sm py-2 px-3 gap-x-2 text-zinc-100 hover:text-zinc-300 hover:bg-zinc-700"
            @click="() => launchIndex(launchIdx)"
          >
            <PlayIcon class="size-4" />
            <span>
              {{ launchData.name }}
            </span>
          </button>
        </li>
      </ol>
    </template>
    <template #buttons>
      <button
        type="button"
        class="mt-3 inline-flex w-full justify-center rounded-md bg-zinc-800 px-3 py-2 text-sm font-semibold text-zinc-100 shadow-sm ring-1 ring-inset ring-zinc-700 hover:bg-zinc-900 sm:mt-0 sm:w-auto"
        @click="launchOptions = undefined"
        ref="cancelButtonRef"
      >
        Cancel
      </button>
    </template>
  </ModalTemplate>

  <!-- 
  Dear future DecDuck,
  This v-if is necessary for Vue rendering reasons
  (it tries to access the game version for not installed games)
  You have already tried to remove it
  Don't.  
  -->
  <GameOptionsModal
    v-if="
      status.type === 'Installed' &&
      (status.install_type.type == InstalledType.Installed ||
        status.install_type.type == InstalledType.SetupRequired)
    "
    v-model="configureModalOpen"
    :game-id="game.id"
  />

  <Transition
    enter="transition ease-out duration-300"
    enter-from="opacity-0"
    enter-to="opacity-100"
    leave="transition ease-in duration-200"
    leave-from="opacity-100"
    leave-to="opacity-0"
  >
    <div
      v-if="fullscreenImage"
      class="fixed inset-0 z-50 bg-black/95 flex items-center justify-center"
      @click="fullscreenImage = null"
    >
      <div
        class="relative w-full h-full flex items-center justify-center"
        @click.stop
      >
        <button
          class="absolute top-4 right-4 p-2 rounded-full bg-zinc-900/50 text-zinc-100 hover:bg-zinc-900 transition-colors"
          @click.stop="fullscreenImage = null"
        >
          <XMarkIcon class="size-6" />
        </button>

        <button
          v-if="game.mImageCarouselObjectIds.length > 1"
          @click.stop="previousImage()"
          class="absolute left-4 p-3 rounded-full bg-zinc-900/50 text-zinc-100 hover:bg-zinc-900 transition-colors"
        >
          <ChevronLeftIcon class="size-6" />
        </button>
        <button
          v-if="game.mImageCarouselObjectIds.length > 1"
          @click.stop="nextImage()"
          class="absolute right-4 p-3 rounded-full bg-zinc-900/50 text-zinc-100 hover:bg-zinc-900 transition-colors"
        >
          <ChevronRightIcon class="size-6" />
        </button>

        <TransitionGroup
          name="slide"
          tag="div"
          class="w-full h-full flex items-center justify-center"
          @click.stop
        >
          <img
            v-for="(url, index) in game.mImageCarouselObjectIds"
            v-show="currentImageIndex === index"
            :key="index"
            :src="useObject(url)"
            class="max-h-[90vh] max-w-[90vw] object-contain"
            :alt="`${game.mName} screenshot ${index + 1}`"
          />
        </TransitionGroup>

        <div
          class="absolute bottom-4 left-1/2 -translate-x-1/2 px-4 py-2 rounded-full bg-zinc-900/50 backdrop-blur-sm"
        >
          <p class="text-zinc-100 text-sm font-medium">
            {{ currentImageIndex + 1 }} /
            {{ game.mImageCarouselObjectIds.length }}
          </p>
        </div>
      </div>
    </div>
  </Transition>

  <DependencyRequiredModal
    v-if="dependencyRequiredModal"
    v-model="dependencyRequiredModal"
  />

  <!-- Achievement Reset Confirmation Modal -->
  <Transition
    enter-active-class="ease-out duration-200"
    enter-from-class="opacity-0"
    enter-to-class="opacity-100"
    leave-active-class="ease-in duration-150"
    leave-from-class="opacity-100"
    leave-to-class="opacity-0"
  >
    <div
      v-if="resetConfirmOpen"
      class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm"
      @click.self="resetConfirmOpen = false"
    >
      <div
        class="w-full max-w-sm rounded-xl bg-zinc-900 border border-zinc-700 shadow-2xl"
      >
        <div class="px-6 py-5">
          <h3 class="text-base font-semibold font-display text-zinc-100">
            Reset Achievements
          </h3>
          <p class="mt-2 text-sm text-zinc-400">
            Reset all achievements for
            <span class="text-zinc-200 font-medium">{{ game.mName }}</span
            >? This cannot be undone.
          </p>
        </div>
        <div class="flex justify-end gap-3 border-t border-zinc-700 px-6 py-4">
          <button
            @click="resetConfirmOpen = false"
            class="rounded-md px-4 py-2 text-sm font-medium text-zinc-300 hover:bg-zinc-800 transition-colors"
          >
            Cancel
          </button>
          <button
            @click="executeResetAchievements"
            :disabled="resetBusy"
            class="rounded-md px-4 py-2 text-sm font-medium text-white bg-red-600 hover:bg-red-700 disabled:opacity-50 transition-colors"
          >
            {{ resetBusy ? "Resetting..." : "Reset" }}
          </button>
        </div>
      </div>
    </div>
  </Transition>

  <!-- Cloud Save Conflict Resolution Dialog -->
  <SaveConflictDialog
    v-model="saveConflictOpen"
    :game-id="game.id"
    :conflicts="saveConflicts"
  />
</template>

<script setup lang="ts">
import {
  Listbox,
  ListboxButton,
  ListboxLabel,
  ListboxOption,
  ListboxOptions,
  Menu,
  MenuButton,
  MenuItem,
  MenuItems,
} from "@headlessui/vue";
import {
  CheckIcon,
  ChevronDownIcon,
  ChevronUpDownIcon,
  WrenchIcon,
  ChevronLeftIcon,
  ChevronRightIcon,
  XMarkIcon,
  ArrowsPointingOutIcon,
  PhotoIcon,
  PlayIcon,
  InformationCircleIcon,
} from "@heroicons/vue/20/solid";
import {
  BuildingStorefrontIcon,
  Cog6ToothIcon,
  TrashIcon,
} from "@heroicons/vue/24/outline";
import {
  AdjustmentsHorizontalIcon,
  ArrowDownTrayIcon,
  CalendarIcon,
  CheckCircleIcon,
  ClockIcon,
  MapPinIcon,
  MinusIcon,
  ServerIcon,
  SparklesIcon,
  TrophyIcon,
  UserIcon,
  XCircleIcon,
} from "@heroicons/vue/24/solid";
import { invoke } from "@tauri-apps/api/core";
import { useListen } from "~/composables/useListen";
import { micromark } from "micromark";
import { InstalledType } from "~/types";
import type { AspectRatio, ControllerType, QualityPreset } from "~/types";
import {
  rewriteDescriptionImages,
  serverUrl,
} from "~/composables/use-server-fetch";

const route = useRoute();
const router = useRouter();
const id = route.params.id.toString();

const { game, status, version } = await useGame(id);

const bannerUrl = await useObject(game.mBannerObjectId);

// Compat data scoped to this specific game. Soft-fails (returns null) so a
// server-side problem with the compat endpoints doesn't 500 the whole page.
// `gameCompat` is what the right-sidebar panel renders against; refreshing
// the underlying summary state propagates here automatically.
const compatSummaryRef = await useCompatSummary().catch(() => null);
const gameCompat = computed(() => compatSummaryRef?.value?.[id]);

const rawHtml = micromark(game.mDescription);
const htmlDescription = rewriteDescriptionImages(rawHtml);

// ── Game type detection ─────────────────────────────────────────────────
// ROM games run through an emulator (RetroArch) — show controller/quality/widescreen
// Non-emulator games may use Goldberg/Steam emu — show Set Account Name
const isEmulatedGame = computed(
  () => version.value?.launches?.some((l: any) => l.emulator != null) ?? false,
);
const isNativeGame = computed(() => !isEmulatedGame.value);

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

const selectedController = ref<ControllerType | null>(
  version.value?.userConfiguration?.controllerType ?? null,
);
const selectedQuality = ref<QualityPreset | null>(
  version.value?.userConfiguration?.qualityPreset ?? null,
);
// Backward compat: old databases may still store widescreen as a boolean
const _ws = version.value?.userConfiguration?.widescreen;
const aspectRatio = ref<AspectRatio>(
  _ws === true ? "Wide16_9" : _ws === false || _ws == null ? "Standard" : _ws as AspectRatio,
);
const ASPECT_CYCLE: AspectRatio[] = ["Standard", "Wide16_9", "Wide16_10"];
const aspectLabel = computed(() => {
  switch (aspectRatio.value) {
    case "Wide16_9": return "16:9";
    case "Wide16_10": return "16:10";
    default: return "4:3";
  }
});

async function saveUserConfig() {
  if (!version.value) return;
  try {
    const config = {
      ...version.value.userConfiguration,
      controllerType: selectedController.value,
      qualityPreset: selectedQuality.value,
      widescreen: aspectRatio.value,
    };
    await invoke("update_game_configuration", {
      gameId: game.id,
      options: config,
    });
  } catch (e) {
    console.error("Failed to save config:", e);
  }
}

function setController(value: ControllerType | null) {
  selectedController.value = value;
  saveUserConfig();
}

function setQuality(value: QualityPreset | null) {
  selectedQuality.value = value;
  saveUserConfig();
}

function toggleWidescreen() {
  const idx = ASPECT_CYCLE.indexOf(aspectRatio.value);
  aspectRatio.value = ASPECT_CYCLE[(idx + 1) % ASPECT_CYCLE.length];
  saveUserConfig();
}

const descriptionOpen = ref(true);

// Game stats bar
const statsLoading = ref(true);
const gameStats = reactive({
  playtimeSeconds: 0,
  lastPlayedAt: null as string | null,
  achievementsUnlocked: 0,
  achievementsTotal: 0,
});

onMounted(async () => {
  try {
    const res = await fetch(serverUrl(`api/v1/games/${game.id}/stats`));
    if (res.ok) {
      const data = await res.json();
      Object.assign(gameStats, data);
    }
  } catch {
    // Stats are non-critical; silently fail
  } finally {
    statsLoading.value = false;
  }
});

function formatPlaytime(seconds: number): string {
  if (seconds < 60) return "< 1 min";
  const hours = seconds / 3600;
  if (hours >= 1) {
    const rounded = Math.round(hours * 10) / 10;
    return `${rounded} ${rounded === 1 ? "hour" : "hours"}`;
  }
  return `${Math.round(seconds / 60)} min`;
}

function formatLastPlayed(dateStr: string): string {
  const date = new Date(dateStr);
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffDays = Math.floor(diffMs / (1000 * 60 * 60 * 24));

  if (diffDays === 0) return "Today";
  if (diffDays === 1) return "Yesterday";
  if (diffDays < 30) return `${diffDays} days ago`;

  return date.toLocaleDateString(undefined, {
    month: "short",
    day: "numeric",
    year: date.getFullYear() !== now.getFullYear() ? "numeric" : undefined,
  });
}

// Achievements
type AchievementData = {
  id: string;
  title: string;
  description: string;
  iconUrl: string;
  unlocked: boolean;
};

const achievements = ref<AchievementData[]>([]);
const achievementsLoading = ref(true);
const achievementsUnlocked = computed(
  () => achievements.value.filter((a) => a.unlocked).length,
);

// Fetch achievements from the server via the authenticated protocol
const achievementsUrl = serverUrl(`api/v1/games/${game.id}/achievements`);
onMounted(async () => {
  try {
    const res = await fetch(achievementsUrl);
    if (res.ok) {
      const data = await res.json();
      // Server returns a plain array
      achievements.value = Array.isArray(data)
        ? data
        : (data.achievements ?? []);
    }
  } catch {
    achievements.value = [];
  } finally {
    achievementsLoading.value = false;
  }
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
useListen<RomHashResult>(`ra_hash_check/${game.id}`, (event) => {
  romHashResult.value = event.payload;
});

// On-demand hash check (Phase 3)
async function checkRomHash() {
  romHashChecking.value = true;
  romHashResult.value = null;
  try {
    const result = await invoke<RomHashResult>("check_ra_rom_hash", {
      gameId: game.id,
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

// ── Cloud Save Conflict Resolution ──────────────────────────────────────
import type { SaveConflict } from "~/types/save-sync";

const saveConflictOpen = ref(false);
const saveConflicts = ref<SaveConflict[]>([]);

useListen<{ gameId: string; conflicts: SaveConflict[] }>(
  `save_sync_conflict/${game.id}`,
  (event) => {
    saveConflicts.value = event.payload.conflicts;
    saveConflictOpen.value = true;
  },
);

const installFlowOpen = ref(false);
const versionOptions = ref<undefined | Array<VersionOption>>();
const installDirs = ref<undefined | Array<string>>();
const currentImageIndex = ref(0);

const configureModalOpen = ref(false);

async function installFlow() {
  installFlowOpen.value = true;
  versionOptions.value = undefined;
  installDirs.value = undefined;
  installError.value = undefined;

  try {
    versionOptions.value = await invoke("fetch_game_version_options", {
      gameId: game.id,
    });
    installDirs.value = await invoke("fetch_download_dir_stats");
  } catch (error) {
    installError.value = (error as string).toString();
    versionOptions.value = undefined;
  }
}

const installLoading = ref(false);
const installError = ref<string | undefined>();
const installVersionIndex = ref(-1);
const installDir = ref(0);
const installDepsDisabled = ref<{ [key: string]: boolean }>({});

async function install() {
  try {
    if (!versionOptions.value) throw new Error("Versions have not been loaded");
    installLoading.value = true;
    const versionOption =
      versionOptions.value[Math.max(installVersionIndex.value, 0)];
    const isLatest = installVersionIndex.value == -1;

    const games = [
      { gameId: game.id, versionId: versionOption.versionId },
      ...versionOption.requiredContent
        .filter((v) => !installDepsDisabled.value[v.versionId])
        .map((v) => ({ gameId: v.gameId, versionId: v.versionId })),
    ];

    for (const game of games) {
      await invoke("download_game", {
        gameId: game.gameId,
        versionId: game.versionId,
        installDir: installDir.value,
        targetPlatform: versionOption.platform,
        enableUpdates: isLatest,
      });
    }

    installFlowOpen.value = false;
  } catch (error) {
    installError.value = (error as string).toString();
  }

  installLoading.value = false;
}

const currentVersionOption = computed(
  () => versionOptions.value?.[Math.max(installVersionIndex.value, 0)],
);

function formatVersionOptionText(index: number) {
  if (!versionOptions.value) return undefined;
  const versionOption = versionOptions.value[Math.max(index, 0)];
  const template = `${versionOption.displayName || versionOption.versionPath} on ${versionOption.platform}, ${formatKilobytes(versionOption.size.installSize / 1024)}B`;
  if (index == -1) {
    return `Latest (${template})`;
  }
  return template;
}

async function resumeDownload() {
  try {
    await invoke("resume_download", { gameId: game.id });
  } catch (e) {
    console.error(e);
  }
}

const launchOptions = ref<Array<{ name: string }> | undefined>(undefined);
const launchOptionsOpen = computed(() => launchOptions.value !== undefined);

async function launch() {
  if (
    status.value.type == "Installed" &&
    status.value.install_type.type == InstalledType.SetupRequired
  ) {
    await launchIndex(0);
    return;
  }
  try {
    const fetchedLaunchOptions = await invoke<Array<{ name: string }>>(
      "get_launch_options",
      { id: game.id },
    );
    if (fetchedLaunchOptions.length == 1) {
      await launchIndex(0);
      return;
    }
    launchOptions.value = fetchedLaunchOptions;
  } catch (e) {
    createModal(
      ModalType.Notification,
      {
        title: `Couldn't run "${game.mName}"`,
        description: `Drop failed to launch "${game.mName}": ${e}`,
        buttonText: "Close",
      },
      (e, c) => c(),
    );
    console.error(e);
  }
}

const dependencyRequiredModal = ref<
  { gameId: string; versionId: string } | undefined
>(undefined);

const launchInFlight = ref(false);

async function launchIndex(index: number) {
  // Guard against duplicate `launch_game` invocations from double-clicks /
  // repeated keyboard activations. The backend rejects the second call with
  // `AlreadyRunning`, which would show an error over a game that's actually
  // starting fine.
  if (launchInFlight.value) return;
  launchInFlight.value = true;
  launchOptions.value = undefined;
  try {
    const result = await invoke<LaunchResult>("launch_game", {
      id: game.id,
      index,
    });
    if (result.result == "InstallRequired") {
      dependencyRequiredModal.value = {
        gameId: result.data[0],
        versionId: result.data[1],
      };
    }
  } catch (e) {
    const errMsg = e instanceof Error ? e.message : String(e);
    if (errMsg.includes("AlreadyRunning") || errMsg.includes("already running")) {
      // Benign — the first invoke already started the game.
      return;
    }
    createModal(
      ModalType.Notification,
      {
        title: `Couldn't run "${game.mName}"`,
        description: `Drop failed to launch "${game.mName}": ${errMsg}`,
        buttonText: "Close",
      },
      (e, c) => c(),
    );
  } finally {
    launchInFlight.value = false;
  }
}

async function queue() {
  router.push("/queue");
}

type CompatTestOutcome = {
  status: string;
  signature: string | null;
  elapsedSecs: number;
  posted: boolean;
  // Set by the button before emitting if the user confirmed render.
  protonVersion?: string | null;
};

const COMPAT_STATUS_LABELS: Record<string, string> = {
  AliveRenders: "Plays correctly",
  AliveNoRender: "Launches but no visible render",
  EarlyExit: "Exits before main menu",
  Crash: "Crashes on launch",
  NoLaunch: "Won't launch",
  InstallFailed: "Install failed",
};

/**
 * Show the user a one-shot summary of what the compat test found. The
 * actual result has already been POSTed to drop-server by the Rust side
 * before this fires; the modal is purely informational.
 *
 * Includes the runtime version (Proton/Wine) and the crash signature
 * when applicable, so the user has the same triage info the badge
 * tooltip would surface — no need to navigate to drop-server's web UI
 * to see why a test failed.
 */
function onCompatTestResult(outcome: CompatTestOutcome) {
  const label = COMPAT_STATUS_LABELS[outcome.status] ?? outcome.status;
  const lines = [
    `Result: ${label}`,
    `Observed for ${outcome.elapsedSecs}s.`,
  ];
  if (outcome.protonVersion) {
    lines.push(`Runtime: ${outcome.protonVersion}`);
  }
  if (outcome.signature) {
    lines.push(`Signature: ${outcome.signature}`);
  }
  if (!outcome.posted) {
    lines.push(
      "(Server didn't accept the result — may be offline or unauthenticated.)",
    );
  }
  createModal(
    ModalType.Notification,
    {
      title: `Compatibility test — ${game.mName}`,
      description: lines.join("\n"),
      buttonText: "OK",
    },
    (e, c) => c(),
  );
}

async function uninstall() {
  await invoke("uninstall_game", { gameId: game.id });
}

async function kill() {
  try {
    await invoke("kill_game", { gameId: game.id });
  } catch (e) {
    createModal(
      ModalType.Notification,
      {
        title: `Couldn't stop "${game.mName}"`,
        description: `Drop failed to stop "${game.mName}": ${e}`,
        buttonText: "Close",
      },
      (e, c) => c(),
    );
    console.error(e);
  }
}

function nextImage() {
  currentImageIndex.value =
    (currentImageIndex.value + 1) % game.mImageCarouselObjectIds.length;
}

function previousImage() {
  currentImageIndex.value =
    (currentImageIndex.value - 1 + game.mImageCarouselObjectIds.length) %
    game.mImageCarouselObjectIds.length;
}

const fullscreenImage = ref<string | null>(null);

// ── Achievement reset (per-game) ──────────────────────────────────────────

const resetConfirmOpen = ref(false);
const resetBusy = ref(false);

function confirmResetAchievements() {
  resetConfirmOpen.value = true;
}

async function executeResetAchievements() {
  resetBusy.value = true;
  try {
    const res = await fetch(
      serverUrl(`api/v1/user/achievements/reset?gameId=${game.id}`),
      { method: "DELETE" },
    );
    if (res.ok) {
      const data = await res.json();
      achievements.value = achievements.value.map((a) => ({
        ...a,
        unlocked: false,
      }));
      resetConfirmOpen.value = false;
    }
  } catch {
    // Keep modal open so user sees the failure context
  } finally {
    resetBusy.value = false;
  }
}

// ── Apply profile name to Steam emulator ──────────────────────────────────

async function applyProfileName() {
  try {
    const msg = await invoke<string>("configure_game_emulator", {
      gameId: game.id,
    });
    console.log("[EMU]", msg);
  } catch (e) {
    console.error("[EMU] Failed to apply profile:", e);
  }
}
</script>

<style scoped>
.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.3s ease;
}

.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}

.slide-enter-active,
.slide-leave-active {
  transition: all 0.3s ease;
  position: absolute;
}

.slide-enter-from {
  opacity: 0;
  transform: translateX(100%);
}

.slide-leave-to {
  opacity: 0;
  transform: translateX(-100%);
}

.custom-scrollbar {
  scrollbar-width: thin;
  scrollbar-color: rgb(82 82 91) transparent;
}

.custom-scrollbar::-webkit-scrollbar {
  width: 6px;
}

.custom-scrollbar::-webkit-scrollbar-track {
  background: transparent;
}

.custom-scrollbar::-webkit-scrollbar-thumb {
  background-color: rgb(82 82 91);
  border-radius: 3px;
}
</style>
