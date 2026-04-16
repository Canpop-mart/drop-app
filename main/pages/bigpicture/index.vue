<template>
  <div class="flex flex-col h-full">
    <!-- Global BPM overlays -->
    <BpmScreensaver :active="screensaverEnabled && isIdle" @dismiss="isIdle = false" />
    <BpmCrtFilter v-if="crtEnabled" :enabled="true" :intensity="0.4" />
    <BpmAchievementToast v-if="currentAchievement" :theme-id="theme" :achievement="currentAchievement" @dismissed="currentAchievement = null" />
    <BpmLaunchScreen v-if="launchingGame" :theme-id="theme" :game="launchingGame" @ready="launchingGame = null" />

    <!-- Main scrollable content -->
    <div
      ref="scrollContainer"
      class="flex-1 overflow-y-auto"
      data-bp-scroll
    >

      <!-- ═══════════════════════════════════════════════════════════════════
           STEAM — Big Picture Mode / Steam Deck
           Full-width hero → "RECENT GAMES" shelf → downloads
           All sizes in vw/vh for resolution independence.
           ═══════════════════════════════════════════════════════════════════ -->
      <template v-if="theme === 'steam'">
        <div class="min-h-full" style="background-color: var(--bpm-bg)">
          <!-- Empty state -->
          <div v-if="recentGames.length === 0 && !loading" class="flex items-center justify-center" style="min-height: 70vh">
            <div class="text-center">
              <PlayIcon class="mx-auto mb-[1.5vh]" style="color: #66c0f4; opacity: 0.4; width: 3.5vw; height: 3.5vw" />
              <p class="font-medium" style="color: #8f98a0; font-size: 1.1vw">No recent games</p>
              <p style="color: #556773; font-size: 0.9vw; margin-top: 0.5vh">Play something to see it here</p>
            </div>
          </div>

          <template v-if="recentGames.length > 0">
            <!-- Steam-style status bar -->
            <div class="flex items-center justify-between" style="padding: 1vh 3vw; background-color: #1b2838; border-bottom: 1px solid #2a475e">
              <div class="flex items-center" style="gap: 2vw">
                <span class="font-bold tracking-wider uppercase" style="color: #66c0f4; font-size: 0.75vw">Home</span>
                <span style="color: #4f6a80; font-size: 0.75vw">|</span>
                <span class="tracking-wider uppercase" style="color: #4f6a80; font-size: 0.75vw">Library</span>
              </div>
              <span style="color: #4f6a80; font-size: 0.75vw">{{ recentGames.length }} game{{ recentGames.length !== 1 ? 's' : '' }}</span>
            </div>

            <!-- Hero Banner -->
            <section v-if="spotlightGame" class="relative w-full" style="height: 53vh; min-height: 14rem">
              <div class="absolute inset-0" style="background-color: #1b2838">
                <img
                  v-if="spotlightGame.game.mCoverObjectId"
                  :src="useObject(spotlightGame.game.mCoverObjectId)"
                  :alt="spotlightGame.game.mName"
                  class="w-full h-full object-cover"
                  style="opacity: 0.45"
                />
              </div>
              <div class="absolute inset-0" style="background: linear-gradient(to top, #171a21 0%, transparent 60%)" />
              <div class="absolute inset-0" style="background: linear-gradient(to right, #171a21 0%, transparent 40%)" />
              <!-- Subtle horizontal line accent at bottom -->
              <div class="absolute bottom-0 inset-x-0" style="height: 2px; background: linear-gradient(90deg, #66c0f4, transparent 60%); opacity: 0.4" />
              <div
                class="absolute bottom-0 left-0"
                style="padding: 3vh 3vw; max-width: 40vw"
              >
                <p class="font-bold tracking-widest uppercase" style="color: #66c0f4; font-size: 0.8vw; margin-bottom: 1vh">Continue Playing</p>
                <h2 class="font-bold" style="color: #c7d5e0; font-size: 2.5vw; margin-bottom: 1vh">{{ spotlightGame.game.mName }}</h2>
                <p v-if="spotlightGame.status.installed" style="color: #8f98a0; font-size: 0.9vw; margin-bottom: 2vh">
                  {{ formatPlaytime(spotlightGame.status.playtime) }} played
                </p>
                <button
                  class="rounded font-bold tracking-wide transition-all hover:brightness-110 cursor-pointer"
                  style="background: linear-gradient(135deg, #1a9fff, #0d84e0); color: #fff; padding: 0.8vh 2.5vw; font-size: 0.9vw; box-shadow: 0 0.3vh 1vw rgba(26,159,255,0.3)"
                  :ref="(el: any) => registerTile(el, {
                    onSelect: () => navigateToGame(spotlightGame.game.id),
                    onFocus: () => prefetchGame(spotlightGame.game.id),
                  })"
                >
                  PLAY
                </button>
              </div>
            </section>

            <!-- Recent Games Shelf -->
            <section style="padding: 1vh 3vw 3vh 3vw">
              <h3 class="font-bold tracking-widest uppercase" style="color: #8f98a0; font-size: 0.85vw; margin-bottom: 2vh">
                RECENT GAMES
              </h3>
              <div class="flex overflow-x-auto scrollbar-thin" style="gap: 1vw; padding: 1vh 0.5vw">
                <div
                  v-for="entry in otherGames.slice(0, 8)"
                  :key="entry.game.id"
                  class="flex-shrink-0 cursor-pointer bp-focus-delegate"
                  style="width: 9.5vw"
                  :ref="(el: any) => registerTile(el, {
                    onSelect: () => navigateToGame(entry.game.id),
                    onFocus: () => prefetchGame(entry.game.id),
                  })"
                >
                  <BigPictureGameTile :game="entry.game" :status="entry.status" :hide-titles="hideTitles" />
                </div>
              </div>
            </section>

            <!-- Downloads -->
            <section v-if="activeDownloads.length > 0" style="padding: 0 3vw 3vh 3vw">
              <h3 class="font-bold tracking-widest uppercase" style="color: #8f98a0; font-size: 0.85vw; margin-bottom: 1.5vh">
                DOWNLOADS
              </h3>
              <div style="display: flex; flex-direction: column; gap: 0.6vh">
                <div
                  v-for="item in activeDownloads.slice(0, 3)"
                  :key="item.meta.id"
                  class="flex items-center rounded-md"
                  style="background-color: #1b2838; padding: 1.2vh 1.5vw; gap: 1.5vw; border: 1px solid rgba(42,71,94,0.3)"
                >
                  <div class="flex-1 min-w-0">
                    <p class="font-medium" style="color: #c7d5e0; font-size: 0.9vw">
                      {{ gameNames[item.meta.id]?.name || item.meta.id }}
                    </p>
                    <div class="rounded-full overflow-hidden" style="margin-top: 0.8vh; height: 0.4vh; background-color: #2a475e">
                      <div
                        class="h-full rounded-full transition-all duration-300"
                        style="background-color: #1a9fff"
                        :style="{ width: `${(item.dl_progress * 100).toFixed(0)}%` }"
                      />
                    </div>
                  </div>
                  <span class="font-medium flex-shrink-0" style="color: #8f98a0; font-size: 0.8vw">
                    {{ (item.dl_progress * 100).toFixed(0) }}%
                  </span>
                </div>
              </div>
            </section>
          </template>
        </div>
      </template>


      <!-- Switch theme removed -->
      <template v-if="false && theme === 'switch-removed'">
        <div class="relative flex flex-col h-full" style="min-height: 80vh; background-color: var(--bpm-bg)">
          <BpmAnimatedBackground :enabled="animBgEnabled" :theme-id="theme" :reduced="reducedAnimations" />

          <!-- Joy-Con rail accents -->
          <div class="absolute left-0 top-0 bottom-0" style="width: 0.35vw; background: linear-gradient(180deg, #00c3e3 0%, #0ab9e0 50%, #009dc7 100%); z-index: 2" />
          <div class="absolute right-0 top-0 bottom-0" style="width: 0.35vw; background: linear-gradient(180deg, #e60012 0%, #d00010 50%, #b8000e 100%); z-index: 2" />

          <!-- Top nav bar -->
          <div class="relative flex items-center justify-between" style="padding: 1.5vh 3vw; z-index: 1">
            <div class="flex items-center" style="gap: 2vw">
              <div
                class="flex items-center justify-center cursor-pointer bp-focus-delegate"
                style="width: 2.5vw; height: 2.5vw; border-radius: 50%; background-color: rgba(255,255,255,0.08)"
                :ref="(el: any) => registerTile(el, { onSelect: () => router.push('/bigpicture/library') })"
              >
                <img v-if="userProfilePicUrl" :src="userProfilePicUrl" class="w-full h-full object-cover rounded-full" />
                <svg v-else style="width: 1.2vw; height: 1.2vw" fill="#aaa" viewBox="0 0 24 24"><path d="M12 12c2.21 0 4-1.79 4-4s-1.79-4-4-4-4 1.79-4 4 1.79 4 4 4zm0 2c-2.67 0-8 1.34-8 4v2h16v-2c0-2.66-5.33-4-8-4z"/></svg>
              </div>
            </div>
            <div class="flex items-center" style="gap: 1.5vw">
              <span style="color: #666; font-size: 0.75vw">{{ recentGames.length }} game{{ recentGames.length !== 1 ? 's' : '' }}</span>
              <span style="color: #888; font-size: 0.75vw">{{ bpmClock.time.value }}</span>
            </div>
          </div>

          <!-- Empty state -->
          <div v-if="recentGames.length === 0 && !loading" class="flex items-center justify-center flex-1">
            <div class="text-center">
              <div class="mx-auto flex items-center justify-center" style="width: 8vw; height: 8vw; border-radius: 1.2vw; background-color: #2d2d2d; margin-bottom: 2vh">
                <PlayIcon style="color: #e60012; opacity: 0.6; width: 4vw; height: 4vw" />
              </div>
              <p style="color: #888; font-size: 1vw">No games yet</p>
            </div>
          </div>

          <template v-if="recentGames.length > 0">
            <!-- Selected game banner area -->
            <div class="relative flex-shrink-0 mx-auto" style="max-width: 70vw; width: 100%; margin-top: 2vh; margin-bottom: 2vh; height: 22vh">
              <div v-if="focusedSwitchGame" class="relative h-full w-full overflow-hidden transition-all duration-300" style="border-radius: 1vw">
                <img
                  v-if="focusedSwitchGame.game.mBannerObjectId"
                  :src="useObject(focusedSwitchGame.game.mBannerObjectId)"
                  class="absolute inset-0 w-full h-full object-cover"
                />
                <div v-else class="absolute inset-0" style="background: linear-gradient(135deg, #1a1a2e, #16213e)" />
                <div class="absolute inset-0" style="background: linear-gradient(to top, rgba(0,0,0,0.85) 0%, rgba(0,0,0,0.3) 50%, transparent 100%)" />
                <div class="absolute bottom-0 left-0 right-0" style="padding: 2vh 2vw">
                  <h2 class="font-bold" style="color: #fff; font-size: 1.6vw; text-shadow: 0 2px 6px rgba(0,0,0,0.6)">
                    {{ focusedSwitchGame.game.mName }}
                  </h2>
                  <p v-if="focusedSwitchGame.game.mShortDescription" class="mt-0.5" style="color: rgba(255,255,255,0.6); font-size: 0.8vw; max-width: 40vw; overflow: hidden; text-overflow: ellipsis; white-space: nowrap">
                    {{ focusedSwitchGame.game.mShortDescription }}
                  </p>
                </div>
              </div>
            </div>

            <!-- Game icon row -->
            <div class="flex justify-center items-center" style="gap: 1vw; padding: 0 3vw">
              <div
                v-for="(entry, idx) in recentGames.slice(0, 12)"
                :key="entry.game.id"
                class="flex-shrink-0 cursor-pointer transition-all duration-200 bp-focus-delegate"
                :ref="(el: any) => registerTile(el, {
                  onSelect: () => navigateToGame(entry.game.id),
                  onFocus: () => { prefetchGame(entry.game.id); focusedSwitchIdx = idx; },
                })"
              >
                <div
                  class="overflow-hidden transition-all duration-200"
                  :style="{
                    width: focusedSwitchIdx === idx ? '9vw' : '7.5vw',
                    height: focusedSwitchIdx === idx ? '9vw' : '7.5vw',
                    borderRadius: '0.8vw',
                    transform: focusedSwitchIdx === idx ? 'translateY(-0.5vh)' : 'translateY(0)',
                    boxShadow: focusedSwitchIdx === idx
                      ? '0 0 0 0.2vw #e60012, 0 0.6vh 2vw rgba(230,0,18,0.3), 0 0.4vh 1.5vw rgba(0,0,0,0.6)'
                      : '0 0.2vh 0.8vw rgba(0,0,0,0.35)',
                  }"
                >
                  <img
                    v-if="entry.game.mCoverObjectId"
                    :src="useObject(entry.game.mCoverObjectId)"
                    :alt="entry.game.mName"
                    class="w-full h-full object-cover"
                  />
                  <div
                    v-else
                    class="w-full h-full flex items-center justify-center font-bold"
                    :style="{ backgroundColor: '#2d2d2d', color: '#e60012', fontSize: '1.8vw' }"
                  >
                    {{ entry.game.mName.charAt(0).toUpperCase() }}
                  </div>
                </div>
              </div>

              <!-- Library icon -->
              <div
                class="flex-shrink-0 cursor-pointer transition-all duration-200 bp-focus-delegate"
                :ref="(el: any) => registerTile(el, { onSelect: () => router.push('/bigpicture/library') })"
              >
                <div
                  class="flex items-center justify-center overflow-hidden transition-all duration-200"
                  style="width: 7.5vw; height: 7.5vw; border-radius: 0.8vw; background-color: rgba(255,255,255,0.05); border: 1px dashed rgba(255,255,255,0.15)"
                >
                  <svg style="width: 2vw; height: 2vw" fill="none" stroke="rgba(255,255,255,0.3)" stroke-width="1.5" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" d="M3.75 6A2.25 2.25 0 016 3.75h2.25A2.25 2.25 0 0110.5 6v2.25a2.25 2.25 0 01-2.25 2.25H6A2.25 2.25 0 013.75 8.25V6zM3.75 15.75A2.25 2.25 0 016 13.5h2.25a2.25 2.25 0 012.25 2.25V18a2.25 2.25 0 01-2.25 2.25H6A2.25 2.25 0 013.75 18v-2.25zM13.5 6a2.25 2.25 0 012.25-2.25H18A2.25 2.25 0 0120.25 6v2.25A2.25 2.25 0 0118 10.5h-2.25a2.25 2.25 0 01-2.25-2.25V6zM13.5 15.75a2.25 2.25 0 012.25-2.25H18a2.25 2.25 0 012.25 2.25V18A2.25 2.25 0 0118 20.25h-2.25a2.25 2.25 0 01-2.25-2.25v-2.25z" />
                  </svg>
                </div>
              </div>
            </div>

            <!-- Bottom quick actions bar -->
            <div class="mx-auto w-full" style="max-width: 70vw; margin-top: 3vh">
              <div style="height: 1px; width: 100%; background: linear-gradient(90deg, transparent, rgba(255,255,255,0.08), transparent)" />
              <div class="flex justify-center items-center" style="padding: 1.5vh 0; gap: 3vw">
                <span class="cursor-pointer transition-colors" style="color: #666; font-size: 0.8vw; letter-spacing: 0.05em">All Software</span>
                <span style="color: #333; font-size: 0.8vw">|</span>
                <span class="cursor-pointer transition-colors" style="color: #666; font-size: 0.8vw; letter-spacing: 0.05em">News</span>
                <span style="color: #333; font-size: 0.8vw">|</span>
                <span class="cursor-pointer transition-colors" style="color: #666; font-size: 0.8vw; letter-spacing: 0.05em">eShop</span>
              </div>
            </div>

            <!-- Downloads -->
            <section v-if="activeDownloads.length > 0" class="mx-auto w-full" style="max-width: 50vw; margin-top: 1vh; padding-bottom: 2vh">
              <div style="display: flex; flex-direction: column; gap: 0.6vh">
                <div
                  v-for="item in activeDownloads.slice(0, 2)"
                  :key="item.meta.id"
                  class="flex items-center"
                  style="background-color: #1a1a1a; border-radius: 0.6vw; padding: 1vh 1.2vw; gap: 1vw"
                >
                  <div class="flex-1 min-w-0">
                    <p class="font-medium" style="color: #fff; font-size: 0.85vw">
                      {{ gameNames[item.meta.id]?.name || item.meta.id }}
                    </p>
                    <div class="rounded-full overflow-hidden" style="margin-top: 0.6vh; height: 0.35vh; background-color: #2d2d2d">
                      <div
                        class="h-full rounded-full transition-all duration-300"
                        style="background-color: #e60012"
                        :style="{ width: `${Math.min(item.dl_progress * 100, 100).toFixed(0)}%` }"
                      />
                    </div>
                  </div>
                  <span class="font-medium flex-shrink-0" style="color: #888; font-size: 0.8vw">
                    {{ Math.min(item.dl_progress * 100, 100).toFixed(0) }}%
                  </span>
                </div>
              </div>
            </section>
          </template>
        </div>
      </template>


      <!-- ═══════════════════════════════════════════════════════════════════
           XBOX — Xbox Series X|S Dashboard
           Large spotlight left + 2×2 grid right, recently played row below.
           ═══════════════════════════════════════════════════════════════════ -->
      <template v-else-if="theme === 'xbox'">
        <div class="min-h-full" style="background-color: var(--bpm-bg)">
          <!-- Empty state -->
          <div v-if="recentGames.length === 0 && !loading" class="flex items-center justify-center" style="min-height: 70vh">
            <div class="text-center">
              <div class="mx-auto flex items-center justify-center" style="width: 5vw; height: 5vw; border-radius: 0.6vw; background-color: #1a1a1a; margin-bottom: 2vh">
                <PlayIcon style="color: #107c10; opacity: 0.6; width: 2.5vw; height: 2.5vw" />
              </div>
              <p class="font-medium" style="color: #b0b0b0; font-size: 1vw">No recent games</p>
              <p style="color: #666; font-size: 0.85vw; margin-top: 0.5vh">Your games will appear here</p>
            </div>
          </div>

          <template v-if="recentGames.length > 0">
            <!-- Xbox-style profile bar -->
            <div class="flex items-center justify-between" style="padding: 1.5vh 2.5vw; border-bottom: 1px solid rgba(255,255,255,0.06)">
              <div class="flex items-center" style="gap: 1vw">
                <div class="flex items-center justify-center overflow-hidden" style="width: 2vw; height: 2vw; border-radius: 50%; background-color: #107c10">
                  <img v-if="userProfilePicUrl" :src="userProfilePicUrl" class="w-full h-full object-cover" />
                  <svg v-else style="width: 1vw; height: 1vw" fill="#fff" viewBox="0 0 24 24"><path d="M12 12c2.21 0 4-1.79 4-4s-1.79-4-4-4-4 1.79-4 4 1.79 4 4 4zm0 2c-2.67 0-8 1.34-8 4v2h16v-2c0-2.66-5.33-4-8-4z"/></svg>
                </div>
                <span class="font-semibold" style="color: #fff; font-size: 0.9vw">Home</span>
              </div>
              <span style="color: #666; font-size: 0.75vw">{{ recentGames.length }} game{{ recentGames.length !== 1 ? 's' : '' }}</span>
            </div>

            <!-- Dashboard grid -->
            <section style="padding: 2vh 2.5vw">
              <div class="flex" style="height: 42vh; gap: 1vw">
                <!-- Main spotlight -->
                <div
                  v-if="spotlightGame"
                  class="relative flex-[3] rounded-lg overflow-hidden cursor-pointer group transition-transform hover:scale-[1.01]"
                  :ref="(el: any) => registerTile(el, {
                    onSelect: () => navigateToGame(spotlightGame.game.id),
                    onFocus: () => prefetchGame(spotlightGame.game.id),
                  })"
                >
                  <div class="absolute inset-0" style="background-color: #1a1a1a">
                    <img
                      v-if="spotlightGame.game.mCoverObjectId"
                      :src="useObject(spotlightGame.game.mCoverObjectId)"
                      :alt="spotlightGame.game.mName"
                      class="w-full h-full object-cover opacity-80"
                    />
                  </div>
                  <div class="absolute inset-0" style="background: linear-gradient(to top, rgba(0,0,0,0.9) 0%, transparent 50%)" />
                  <div class="absolute bottom-0 left-0" style="padding: 3vh 2.5vw">
                    <h3 class="font-bold" style="color: #fff; font-size: 2vw; margin-bottom: 1vh">{{ spotlightGame.game.mName }}</h3>
                    <p v-if="spotlightGame.status.installed" style="color: #b0b0b0; font-size: 0.9vw; margin-bottom: 2vh">
                      {{ formatPlaytime(spotlightGame.status.playtime) }} played
                    </p>
                    <button
                      class="rounded-sm font-bold tracking-wide text-white transition-colors"
                      style="background-color: #107c10; padding: 0.8vh 2vw; font-size: 0.9vw"
                    >
                      Play
                    </button>
                  </div>
                </div>

                <!-- Small tiles grid -->
                <div v-if="otherGames.length > 0" class="flex-[2] grid grid-cols-2" style="gap: 0.8vw">
                  <div
                    v-for="entry in otherGames.slice(0, 4)"
                    :key="entry.game.id"
                    class="relative rounded-md overflow-hidden cursor-pointer group transition-transform hover:scale-[1.02]"
                    :ref="(el: any) => registerTile(el, {
                      onSelect: () => navigateToGame(entry.game.id),
                      onFocus: () => prefetchGame(entry.game.id),
                    })"
                  >
                    <div class="absolute inset-0" style="background-color: #1a1a1a">
                      <img
                        v-if="entry.game.mCoverObjectId"
                        :src="useObject(entry.game.mCoverObjectId)"
                        :alt="entry.game.mName"
                        class="w-full h-full object-cover opacity-80"
                      />
                    </div>
                    <div class="absolute inset-0" style="background: linear-gradient(to top, rgba(0,0,0,0.85) 0%, transparent 50%)" />
                    <div class="absolute bottom-0 left-0" style="padding: 1.2vh 1vw">
                      <p class="font-semibold truncate" style="color: #fff; font-size: 1vw">{{ entry.game.mName }}</p>
                    </div>
                  </div>
                </div>
              </div>
            </section>

            <!-- Recently Played grid — 4-column portrait boxes (Xbox case style) -->
            <section v-if="recentGames.length > 1" style="padding: 0 2.5vw 2.5vh 2.5vw">
              <h3 class="font-bold tracking-widest uppercase" style="color: #b0b0b0; font-size: 0.85vw; margin-bottom: 2vh">
                Recently Played
              </h3>
              <div class="grid grid-cols-6 mx-auto" style="gap: 1vw; max-width: 80vw; justify-items: center">
                <div
                  v-for="(entry, idx) in otherGames.slice(4, 10)"
                  :key="entry.game.id"
                  class="flex flex-col items-center cursor-pointer bp-focus-delegate"
                  :ref="(el: any) => registerTile(el, {
                    onSelect: () => navigateToGame(entry.game.id),
                    onFocus: () => { prefetchGame(entry.game.id); focusedXboxIdx = idx; },
                  })"
                >
                  <div
                    class="relative overflow-hidden transition-all duration-150"
                    :style="{
                      aspectRatio: '530/708',
                      height: '26vh',
                      borderRadius: '0.3vw',
                      backgroundColor: '#0a1a1a',
                      boxShadow: focusedXboxIdx === idx
                        ? '0 0 1.5vw rgba(16,200,16,0.7), 0 0 3.5vw rgba(16,200,16,0.3)'
                        : '0 0.3vh 1vw rgba(16,124,16,0.3)',
                      border: focusedXboxIdx === idx
                        ? '2px solid rgba(16,200,16,0.8)'
                        : '1px solid rgba(16,124,16,0.3)',
                    }"
                  >
                    <img
                      v-if="entry.game.mCoverObjectId"
                      :src="useObject(entry.game.mCoverObjectId)"
                      :alt="entry.game.mName"
                      class="absolute inset-0 w-full h-full object-cover"
                    />
                    <div
                      v-else
                      class="absolute inset-0 flex items-center justify-center"
                      style="background: linear-gradient(135deg, #107c10, #208020)"
                    >
                      <span class="font-bold" style="color: #fff; font-size: 2vw">
                        {{ entry.game.mName.charAt(0).toUpperCase() }}
                      </span>
                    </div>
                    <BpmBoxArtOverlay :theme-id="theme" />
                  </div>
                  <p
                    v-if="!hideTitles"
                    class="text-center truncate w-full"
                    style="color: #b0b0b0; font-size: 0.8vw; margin-top: 0.8vh"
                  >
                    {{ entry.game.mName }}
                  </p>
                </div>
              </div>
            </section>

            <!-- My Games & Apps link -->
            <section style="padding: 0 2.5vw 2.5vh 2.5vw">
              <div
                class="flex items-center cursor-pointer transition-colors"
                style="background-color: #1a1a1a; border-radius: 0.5vw; padding: 1.5vh 1.5vw; gap: 1.2vw"
                :ref="(el: any) => registerTile(el, {
                  onSelect: () => router.push('/bigpicture/library'),
                })"
              >
                <div class="flex items-center justify-center" style="width: 3vw; height: 3vw; border-radius: 0.4vw; background-color: #107c10">
                  <svg style="width: 1.5vw; height: 1.5vw" fill="none" stroke="#fff" stroke-width="2" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" d="M3.75 6A2.25 2.25 0 016 3.75h2.25A2.25 2.25 0 0110.5 6v2.25a2.25 2.25 0 01-2.25 2.25H6A2.25 2.25 0 013.75 8.25V6zM3.75 15.75A2.25 2.25 0 016 13.5h2.25a2.25 2.25 0 012.25 2.25V18a2.25 2.25 0 01-2.25 2.25H6A2.25 2.25 0 013.75 18v-2.25zM13.5 6a2.25 2.25 0 012.25-2.25H18A2.25 2.25 0 0120.25 6v2.25A2.25 2.25 0 0118 10.5h-2.25a2.25 2.25 0 01-2.25-2.25V6zM13.5 15.75a2.25 2.25 0 012.25-2.25H18a2.25 2.25 0 012.25 2.25V18A2.25 2.25 0 0118 20.25h-2.25a2.25 2.25 0 01-2.25-2.25v-2.25z" />
                  </svg>
                </div>
                <div>
                  <p class="font-semibold" style="color: #fff; font-size: 1.1vw">My games & apps</p>
                  <p style="color: #888; font-size: 0.85vw">{{ recentGames.length }} game{{ recentGames.length !== 1 ? 's' : '' }}</p>
                </div>
              </div>
            </section>

            <!-- Downloads -->
            <section v-if="activeDownloads.length > 0" style="padding: 0 2.5vw 3vh 2.5vw">
              <h3 class="font-bold tracking-widest uppercase" style="color: #b0b0b0; font-size: 0.85vw; margin-bottom: 1.2vh">
                Queue
              </h3>
              <div style="display: flex; flex-direction: column; gap: 0.6vh">
                <div
                  v-for="item in activeDownloads.slice(0, 3)"
                  :key="item.meta.id"
                  class="flex items-center"
                  style="background-color: #1a1a1a; border-radius: 0.4vw; padding: 1.2vh 1.5vw; gap: 1.5vw"
                >
                  <div class="flex-1 min-w-0">
                    <p class="font-medium" style="color: #fff; font-size: 0.9vw">
                      {{ gameNames[item.meta.id]?.name || item.meta.id }}
                    </p>
                    <div class="rounded-full overflow-hidden" style="margin-top: 0.8vh; height: 0.35vh; background-color: #2d2d2d">
                      <div
                        class="h-full rounded-full transition-all duration-300"
                        style="background-color: #107c10"
                        :style="{ width: `${(item.dl_progress * 100).toFixed(0)}%` }"
                      />
                    </div>
                  </div>
                  <span class="font-medium flex-shrink-0" style="color: #888; font-size: 0.8vw">
                    {{ (item.dl_progress * 100).toFixed(0) }}%
                  </span>
                </div>
              </div>
            </section>
          </template>
        </div>
      </template>



      <!-- ═══════════════════════════════════════════════════════════════════
           WII — Wii Channel Menu (Light Mode)
           4-column grid of white rounded channel cards, page dots.
           ═══════════════════════════════════════════════════════════════════ -->
      <template v-else-if="theme === 'wii'">
        <div class="min-h-full flex flex-col relative" :style="{ background: gradientBg || 'var(--bpm-bg)' }">
          <BpmAnimatedBackground :enabled="animBgEnabled" :theme-id="theme" :reduced="reducedAnimations" />
          <!-- Empty state -->
          <div v-if="recentGames.length === 0 && !loading" class="flex items-center justify-center" style="min-height: 60vh">
            <div class="text-center">
              <div
                class="mx-auto flex items-center justify-center"
                :style="{
                  width: '7vw', height: '7vw', borderRadius: '1.5vw',
                  background: 'linear-gradient(135deg, #ffffff, #e8f4ff)',
                  boxShadow: '0 0.3vh 1.2vw rgba(52,190,237,0.15)',
                  marginBottom: '2vh',
                }"
              >
                <PlayIcon style="color: #34beed; opacity: 0.6; width: 3.5vw; height: 3.5vw" />
              </div>
              <p class="font-medium" style="color: #4a5568; font-size: 1vw">No channels yet</p>
              <p style="color: #8898a8; font-size: 0.85vw; margin-top: 0.5vh">Play a game to create a channel</p>
            </div>
          </div>

          <template v-if="recentGames.length > 0">
            <!-- Wii-style top bar -->
            <div class="flex items-center justify-between" style="padding: 1.5vh 3vw; background: rgba(255,255,255,0.3)">
              <span class="font-bold" style="color: #4a7090; font-size: 1vw">Wii Menu</span>
              <span style="color: #6a8aaa; font-size: 0.85vw">{{ recentGames.length }} channel{{ recentGames.length !== 1 ? 's' : '' }}</span>
            </div>

            <!-- Channel Grid — 4 cols, portrait game cases with overlay -->
            <div class="flex-1 flex items-center justify-center" style="padding: 3vh 4vw">
              <div class="grid grid-cols-5 w-full" style="gap: 1.5vw; max-width: 78vw; justify-items: center">
                <div
                  v-for="(entry, idx) in recentGames.slice(0, 14)"
                  :key="entry.game.id"
                  class="flex flex-col items-center cursor-pointer bp-focus-delegate"
                  :ref="(el: any) => registerTile(el, {
                    onSelect: () => navigateToGame(entry.game.id),
                    onFocus: () => { prefetchGame(entry.game.id); focusedWiiIdx = idx; },
                  })"
                >
                  <!-- Game case card — portrait DVD case (515x731 aspect ratio) -->
                  <div
                    class="relative overflow-hidden transition-all duration-150"
                    :style="{
                      aspectRatio: '515/731',
                      height: '28vh',
                      borderRadius: '0.3vw',
                      background: 'linear-gradient(155deg, #ffffff 0%, #f4f8fc 100%)',
                      boxShadow: focusedWiiIdx === idx
                        ? '0 0 1.5vw rgba(52,190,237,0.8), 0 0 3.5vw rgba(20,160,220,0.4)'
                        : '0 0.4vh 1.2vw rgba(0,0,0,0.10), 0 0.15vh 0.4vw rgba(0,0,0,0.06), inset 0 1px 0 rgba(255,255,255,0.9)',
                      border: focusedWiiIdx === idx
                        ? '2px solid rgba(52,190,237,0.9)'
                        : '1px solid rgba(180,200,220,0.4)',
                    }"
                  >
                    <img
                      v-if="entry.game.mCoverObjectId"
                      :src="useObject(entry.game.mCoverObjectId)"
                      :alt="entry.game.mName"
                      class="absolute inset-0 w-full h-full object-cover"
                    />
                    <div
                      v-else
                      class="absolute inset-0 flex items-center justify-center"
                      style="background: linear-gradient(135deg, #34beed, #009ac7)"
                    >
                      <span class="font-bold" style="color: #ffffff; font-size: 2.5vw">
                        {{ entry.game.mName.charAt(0).toUpperCase() }}
                      </span>
                    </div>
                    <BpmBoxArtOverlay :theme-id="theme" />
                  </div>
                  <!-- Channel label -->
                  <p
                    v-if="!hideTitles"
                    class="font-semibold text-center truncate w-full"
                    style="color: #3a5068; font-size: 0.85vw; margin-top: 1vh; padding: 0 0.3vw; text-shadow: 0 1px 0 rgba(255,255,255,0.6)"
                  >
                    {{ entry.game.mName }}
                  </p>
                </div>

                <!-- Library channel -->
                <div class="flex flex-col items-center">
                  <div
                    class="flex items-center justify-center cursor-pointer transition-all duration-150"
                    :style="{
                      aspectRatio: '515/731',
                      height: '28vh',
                      borderRadius: '1.4vw',
                      background: 'linear-gradient(155deg, #f0f5fa 0%, #e4ecf4 100%)',
                      boxShadow: '0 0.3vh 0.8vw rgba(0,0,0,0.06), inset 0 1px 0 rgba(255,255,255,0.7)',
                      border: '1px solid rgba(180,200,220,0.3)',
                    }"
                    :ref="(el: any) => registerTile(el, {
                      onSelect: () => router.push('/bigpicture/library'),
                    })"
                  >
                    <svg style="width: 2.5vw; height: 2.5vw" fill="none" stroke="#34beed" stroke-width="1.5" viewBox="0 0 24 24">
                      <path stroke-linecap="round" stroke-linejoin="round" d="M3.75 6A2.25 2.25 0 016 3.75h2.25A2.25 2.25 0 0110.5 6v2.25a2.25 2.25 0 01-2.25 2.25H6A2.25 2.25 0 013.75 8.25V6zM3.75 15.75A2.25 2.25 0 016 13.5h2.25a2.25 2.25 0 012.25 2.25V18a2.25 2.25 0 01-2.25 2.25H6A2.25 2.25 0 013.75 18v-2.25zM13.5 6a2.25 2.25 0 012.25-2.25H18A2.25 2.25 0 0120.25 6v2.25A2.25 2.25 0 0118 10.5h-2.25a2.25 2.25 0 01-2.25-2.25V6zM13.5 15.75a2.25 2.25 0 012.25-2.25H18a2.25 2.25 0 012.25 2.25V18A2.25 2.25 0 0118 20.25h-2.25a2.25 2.25 0 01-2.25-2.25v-2.25z" />
                    </svg>
                  </div>
                  <p class="font-semibold text-center" style="color: #7a98b0; font-size: 0.85vw; margin-top: 1vh">Library</p>
                </div>
              </div>
            </div>

            <!-- Page indicator dots -->
            <div class="flex justify-center" style="margin-bottom: 2vh; gap: 0.6vw">
              <div class="rounded-full" style="width: 0.6vw; height: 0.6vw; background-color: #34beed" />
              <div class="rounded-full" style="width: 0.6vw; height: 0.6vw; background-color: rgba(52,190,237,0.25)" />
            </div>

            <!-- Downloads -->
            <section v-if="activeDownloads.length > 0" class="mx-auto" style="max-width: 50vw; padding-bottom: 2vh">
              <div style="display: flex; flex-direction: column; gap: 0.6vh">
                <div
                  v-for="item in activeDownloads.slice(0, 3)"
                  :key="item.meta.id"
                  class="flex items-center"
                  :style="{
                    backgroundColor: 'rgba(255,255,255,0.85)',
                    borderRadius: '0.8vw',
                    padding: '1vh 1.5vw',
                    gap: '1.5vw',
                    boxShadow: '0 0.15vh 0.6vw rgba(0,0,0,0.06)',
                  }"
                >
                  <div class="flex-1 min-w-0">
                    <p class="font-medium" style="color: #2d3748; font-size: 0.85vw">
                      {{ gameNames[item.meta.id]?.name || item.meta.id }}
                    </p>
                    <div class="rounded-full overflow-hidden" style="margin-top: 0.5vh; height: 0.4vh; background-color: #d0dce8">
                      <div
                        class="h-full rounded-full transition-all duration-300"
                        style="background-color: #34beed"
                        :style="{ width: `${(item.dl_progress * 100).toFixed(0)}%` }"
                      />
                    </div>
                  </div>
                  <span class="font-medium flex-shrink-0" style="color: #8898a8; font-size: 0.8vw">
                    {{ (item.dl_progress * 100).toFixed(0) }}%
                  </span>
                </div>
              </div>
            </section>
          </template>
        </div>
      </template>


      <!-- ═══════════════════════════════════════════════════════════════════
           PS2 — Memory Card Browser
           Grid of game icons, focused one enlarged + glowing.
           Header shows selected game name. Classic dark PS2 blue.
           ═══════════════════════════════════════════════════════════════════ -->
      <template v-else-if="theme === 'ps2'">
        <div class="relative min-h-full overflow-hidden" :style="{ background: gradientBg || 'var(--bpm-bg)' }">
          <BpmAnimatedBackground :enabled="animBgEnabled" :theme-id="theme" :reduced="reducedAnimations" />
          <!-- Decorative vertical lines — PS2 towers effect -->
          <div class="absolute inset-0 pointer-events-none" style="opacity: 0.04; background: repeating-linear-gradient(90deg, transparent, transparent 3vw, #4060ff 3vw, #4060ff 3.05vw)" />

          <!-- Empty state -->
          <div v-if="recentGames.length === 0 && !loading" class="flex items-center justify-center" style="min-height: 70vh">
            <div class="text-center">
              <div class="mx-auto flex items-center justify-center" style="width: 5vw; height: 5vw; background-color: #14142e; border-radius: 0.5vw; margin-bottom: 2vh">
                <PlayIcon style="color: #2040c0; opacity: 0.6; width: 2.5vw; height: 2.5vw" />
              </div>
              <p class="font-medium" style="color: #8080c0; font-size: 1vw">No save data</p>
              <p style="color: #5050a0; font-size: 0.85vw; margin-top: 0.5vh">Play a game to see it here</p>
            </div>
          </div>

          <template v-if="recentGames.length > 0">
            <!-- Header bar — PS2 memory card browser style -->
            <div class="relative" style="padding: 2vh 3.5vw 1.5vh 3.5vw; background: linear-gradient(180deg, rgba(20,20,60,0.6) 0%, transparent 100%)">
              <div class="flex items-center justify-between">
                <div class="flex items-center" style="gap: 1.2vw">
                  <!-- Memory card icon — more detailed -->
                  <div class="flex items-center justify-center" style="width: 2.2vw; height: 3vw; background: linear-gradient(145deg, #1a1a40, #12122a); border-radius: 0.3vw; border: 1px solid #2a2a58">
                    <div style="width: 1vw; height: 0.5vw; background-color: #3040a0; border-radius: 0.1vw" />
                  </div>
                  <div>
                    <p class="font-medium tracking-wide" style="color: #d0d0f0; font-size: 1.1vw">
                      Memory Card (PS2)
                    </p>
                    <p style="color: #5060a0; font-size: 0.75vw">
                      {{ recentGames.length }} file{{ recentGames.length !== 1 ? 's' : '' }}
                    </p>
                  </div>
                </div>
                <div v-if="ps2FocusedGame" class="text-right">
                  <p class="font-medium" style="color: #b0b8e0; font-size: 0.95vw">{{ ps2FocusedGame.game.mName }}</p>
                  <p v-if="ps2FocusedGame.status.installed" style="color: #4060c0; font-size: 0.75vw">
                    {{ formatPlaytime(ps2FocusedGame.status.playtime) }} played
                  </p>
                </div>
              </div>
            </div>

            <!-- Divider — glowing blue line -->
            <div style="height: 1px; background: linear-gradient(90deg, transparent 5%, #3050c0 30%, #5070e0 50%, #3050c0 70%, transparent 95%); margin: 0 2vw" />

            <!-- Memory card icon grid — portrait DVD cases (515x732 aspect ratio) -->
            <div class="flex flex-col items-center justify-center" style="padding: 3vh 3vw 8vh 3vw; min-height: 65vh">
              <div class="grid grid-cols-5 mx-auto" style="gap: 1.5vw; width: 80%; max-width: 75vw">
                <div
                  v-for="(entry, idx) in recentGames.slice(0, 15)"
                  :key="entry.game.id"
                  class="flex flex-col items-center cursor-pointer bp-focus-delegate"
                  :ref="(el: any) => registerTile(el, {
                    onSelect: () => navigateToGame(entry.game.id),
                    onFocus: () => { prefetchGame(entry.game.id); ps2FocusedIdx = idx; },
                  })"
                >
                  <!-- Game case card — focused one glows bright blue -->
                  <div
                    class="relative overflow-hidden transition-all duration-200"
                    :style="{
                      aspectRatio: '515/732',
                      height: '26vh',
                      borderRadius: '0.3vw',
                      backgroundColor: '#0c0c24',
                      boxShadow: ps2FocusedIdx === idx
                        ? '0 0 1.5vw rgba(80,130,255,0.8), 0 0 3.5vw rgba(60,100,240,0.4)'
                        : '0 0.2vh 0.6vw rgba(0,0,0,0.6)',
                      border: ps2FocusedIdx === idx
                        ? '2px solid rgba(100,150,255,0.9)'
                        : '1px solid rgba(30,30,70,0.4)',
                    }"
                  >
                    <img
                      v-if="entry.game.mCoverObjectId"
                      :src="useObject(entry.game.mCoverObjectId)"
                      :alt="entry.game.mName"
                      class="absolute inset-0 w-full h-full object-cover"
                    />
                    <div
                      v-else
                      class="absolute inset-0 flex items-center justify-center font-bold"
                      :style="{ background: 'linear-gradient(135deg, #2040c0, #4060e0)', color: '#c8d0f0', fontSize: '2vw' }"
                    >
                      {{ entry.game.mName.charAt(0).toUpperCase() }}
                    </div>
                    <BpmBoxArtOverlay :theme-id="theme" />
                  </div>
                  <!-- Game name below icon -->
                  <p
                    v-if="!hideTitles"
                    class="text-center truncate w-full transition-colors duration-200"
                    :style="{
                      fontSize: '0.8vw',
                      marginTop: '0.8vh',
                      color: ps2FocusedIdx === idx ? '#c8d0f0' : '#505080',
                    }"
                  >
                    {{ entry.game.mName }}
                  </p>
                </div>
              </div>
            </div>

            <!-- Bottom bar — PS2 button prompts -->
            <div class="absolute bottom-0 inset-x-0 flex items-center justify-center" style="padding: 1.8vh 3.5vw; gap: 3vw; background: linear-gradient(to top, rgba(8,8,24,0.95), transparent)">
              <span style="color: #4060a0; font-size: 0.8vw">
                <span style="color: #6888e0; font-weight: 600; margin-right: 0.3vw">X</span> Enter
              </span>
              <span style="color: #4060a0; font-size: 0.8vw">
                <span style="color: #e06868; font-weight: 600; margin-right: 0.3vw">O</span> Back
              </span>
              <span style="color: #4060a0; font-size: 0.8vw">
                <span style="color: #60c088; font-weight: 600; margin-right: 0.3vw">△</span> Options
              </span>
            </div>

            <!-- Downloads -->
            <section v-if="activeDownloads.length > 0" style="padding: 0 5vw 5vh 5vw">
              <div style="display: flex; flex-direction: column; gap: 0.6vh">
                <div
                  v-for="item in activeDownloads.slice(0, 2)"
                  :key="item.meta.id"
                  class="flex items-center"
                  style="background-color: rgba(16, 16, 48, 0.9); border-radius: 0.4vw; padding: 0.8vh 1.5vw; gap: 1.5vw"
                >
                  <div class="flex-1 min-w-0">
                    <p class="font-medium" style="color: #c0c0e0; font-size: 0.8vw">
                      {{ gameNames[item.meta.id]?.name || item.meta.id }}
                    </p>
                    <div class="rounded-full overflow-hidden" style="margin-top: 0.4vh; height: 0.3vh; background-color: #1a1a4a">
                      <div
                        class="h-full rounded-full transition-all duration-300"
                        style="background-color: #3050c0"
                        :style="{ width: `${(item.dl_progress * 100).toFixed(0)}%` }"
                      />
                    </div>
                  </div>
                  <span class="font-medium flex-shrink-0" style="color: #8090c0; font-size: 0.75vw">
                    {{ (item.dl_progress * 100).toFixed(0) }}%
                  </span>
                </div>
              </div>
            </section>
          </template>
        </div>
      </template>


      <!-- ═══════════════════════════════════════════════════════════════════
           DS — Nintendo DS Menu Style (Light Mode)
           Dual-screen inspired layout: top shows game info, bottom shows grid.
           Silver-grey light theme with red-orange accents.
           ═══════════════════════════════════════════════════════════════════ -->
      <template v-else-if="theme === 'ds'">
        <div class="min-h-full flex flex-col relative" :style="{ background: gradientBg || 'var(--bpm-bg)' }">
          <BpmAnimatedBackground :enabled="animBgEnabled" :theme-id="theme" :reduced="reducedAnimations" />
          <!-- Empty state -->
          <div v-if="recentGames.length === 0 && !loading" class="flex items-center justify-center" style="min-height: 70vh">
            <div class="text-center">
              <div
                class="mx-auto flex items-center justify-center"
                :style="{
                  width: '6vw', height: '6vw', borderRadius: '1vw',
                  background: 'linear-gradient(135deg, #ffffff, #f0f0f0)',
                  boxShadow: '0 0.3vh 1vw rgba(208,80,40,0.12)',
                  marginBottom: '2vh',
                }"
              >
                <PlayIcon style="color: #d05028; opacity: 0.6; width: 3vw; height: 3vw" />
              </div>
              <p class="font-medium" style="color: #555; font-size: 1vw">No games yet</p>
              <p style="color: #888; font-size: 0.85vw; margin-top: 0.5vh">Play a game to see it here</p>
            </div>
          </div>

          <template v-if="recentGames.length > 0">
            <!-- "Top screen" — feature area with spotlight game -->
            <section
              class="relative"
              style="background: linear-gradient(180deg, #c8c8c8 0%, #b8b8b8 100%)"
            >
              <!-- Top screen header -->
              <div class="flex items-center justify-between" style="padding: 0.8vh 2vw; background: rgba(0,0,0,0.05)">
                <span class="font-bold" style="color: #777; font-size: 0.75vw">TOP SCREEN</span>
                <span style="color: #999; font-size: 0.7vw">{{ recentGames.length }} game{{ recentGames.length !== 1 ? 's' : '' }}</span>
              </div>
              <!-- DS hinge — thick gradient line between screens -->
              <div class="absolute bottom-0 inset-x-0" style="height: 0.6vh; background: linear-gradient(90deg, #999, #888, #777, #888, #999); box-shadow: 0 1px 3px rgba(0,0,0,0.15); z-index: 1" />

              <div v-if="spotlightGame" class="flex items-center justify-center" style="gap: 3vw; padding: 2.5vh 4vw 3vh 4vw">
                <!-- Spotlight cover -->
                <div
                  class="flex-shrink-0 overflow-hidden"
                  :style="{
                    width: '14vw',
                    maxHeight: '42vh',
                    borderRadius: '1vw',
                    backgroundColor: '#a0a0a0',
                    boxShadow: '0 0.3vh 1.5vw rgba(0,0,0,0.12)',
                  }"
                >
                  <img
                    v-if="spotlightGame.game.mCoverObjectId"
                    :src="useObject(spotlightGame.game.mCoverObjectId)"
                    :alt="spotlightGame.game.mName"
                    class="w-full block"
                  />
                </div>
                <!-- Game info -->
                <div style="max-width: 30vw">
                  <p class="font-medium tracking-wider uppercase" style="color: #d05028; font-size: 0.75vw; margin-bottom: 1vh">Now Playing</p>
                  <h2 class="font-bold" style="color: #333; font-size: 2.2vw; margin-bottom: 1vh">{{ spotlightGame.game.mName }}</h2>
                  <p v-if="spotlightGame.status.installed" style="color: #666; font-size: 0.85vw; margin-bottom: 2vh">
                    {{ formatPlaytime(spotlightGame.status.playtime) }} played
                  </p>
                  <button
                    class="rounded-lg font-bold tracking-wide transition-colors"
                    style="background: linear-gradient(135deg, #d05028, #e06030); color: #fff; padding: 0.8vh 2.5vw; font-size: 0.9vw"
                    :ref="(el: any) => registerTile(el, {
                      onSelect: () => navigateToGame(spotlightGame.game.id),
                      onFocus: () => prefetchGame(spotlightGame.game.id),
                    })"
                  >
                    Play
                  </button>
                </div>
              </div>
            </section>

            <!-- "Bottom screen" — game grid (touch screen) -->
            <section style="padding: 2vh 4vw 3vh 4vw; flex: 1; background: linear-gradient(180deg, #d8d8d8 0%, #d0d0d0 100%)">
              <div class="flex items-center justify-between" style="margin-bottom: 2vh">
                <h3 class="font-bold tracking-widest uppercase" style="color: #777; font-size: 0.8vw">
                  My Games
                </h3>
                <span class="tracking-wider uppercase" style="color: #aaa; font-size: 0.65vw">Touch Screen</span>
              </div>
              <div class="grid grid-cols-6 mx-auto" style="gap: 1.2vw; max-width: 80vw; justify-items: center">
                <div
                  v-for="(entry, idx) in otherGames.slice(0, 11)"
                  :key="entry.game.id"
                  class="flex flex-col items-center cursor-pointer group"
                >
                  <!-- DS cartridge — nearly square (571x600), cover sits inside cart frame -->
                  <div
                    class="relative overflow-hidden transition-all duration-150 bp-focus-delegate"
                    :ref="(el: any) => registerTile(el, {
                      onSelect: () => navigateToGame(entry.game.id),
                      onFocus: () => { prefetchGame(entry.game.id); focusedDsIdx = idx; },
                    })"
                    :style="{
                      aspectRatio: '571/600',
                      height: '22vh',
                      borderRadius: '5%',
                      backgroundColor: '#2a2a2a',
                      boxShadow: focusedDsIdx === idx
                        ? '0 0 1.5vw rgba(230,100,50,0.8), 0 0 3.5vw rgba(230,100,50,0.4)'
                        : '0 0.2vh 0.8vw rgba(0,0,0,0.2)',
                      border: focusedDsIdx === idx
                        ? '2px solid rgba(230,100,50,0.9)'
                        : '1px solid transparent',
                    }"
                  >
                    <!-- Cover image clipped to label area inside cart -->
                    <img
                      v-if="entry.game.mCoverObjectId"
                      :src="useObject(entry.game.mCoverObjectId)"
                      :alt="entry.game.mName"
                      class="absolute object-cover"
                      style="top: 13%; left: 7%; width: 86%; height: 73%; border-radius: 2%"
                    />
                    <div
                      v-else
                      class="absolute inset-0 flex items-center justify-center"
                      style="background: linear-gradient(135deg, #d05028, #e06030)"
                    >
                      <span class="font-bold" style="color: #fff; font-size: 2vw">
                        {{ entry.game.mName.charAt(0).toUpperCase() }}
                      </span>
                    </div>
                    <BpmBoxArtOverlay :theme-id="theme" />
                  </div>
                  <p
                    v-if="!hideTitles"
                    class="font-semibold text-center truncate w-full"
                    style="color: #444; font-size: 0.8vw; margin-top: 0.8vh; padding: 0 0.2vw"
                  >
                    {{ entry.game.mName }}
                  </p>
                </div>

                <!-- Library tile -->
                <div class="flex flex-col items-center opacity-50">
                  <div
                    class="flex items-center justify-center cursor-pointer transition-transform duration-150 hover:scale-105"
                    :style="{
                      aspectRatio: '571/600',
                      height: '22vh',
                      borderRadius: '1vw',
                      background: 'linear-gradient(145deg, #e8e8e8, #d8d8d8)',
                      boxShadow: '0 0.15vh 0.5vw rgba(0,0,0,0.06)',
                    }"
                    :ref="(el: any) => registerTile(el, {
                      onSelect: () => router.push('/bigpicture/library'),
                    })"
                  >
                    <svg style="width: 2.5vw; height: 2.5vw" fill="none" stroke="#d05028" stroke-width="1.5" viewBox="0 0 24 24">
                      <path stroke-linecap="round" stroke-linejoin="round" d="M3.75 6A2.25 2.25 0 016 3.75h2.25A2.25 2.25 0 0110.5 6v2.25a2.25 2.25 0 01-2.25 2.25H6A2.25 2.25 0 013.75 8.25V6zM3.75 15.75A2.25 2.25 0 016 13.5h2.25a2.25 2.25 0 012.25 2.25V18a2.25 2.25 0 01-2.25 2.25H6A2.25 2.25 0 013.75 18v-2.25zM13.5 6a2.25 2.25 0 012.25-2.25H18A2.25 2.25 0 0120.25 6v2.25A2.25 2.25 0 0118 10.5h-2.25a2.25 2.25 0 01-2.25-2.25V6zM13.5 15.75a2.25 2.25 0 012.25-2.25H18a2.25 2.25 0 012.25 2.25V18A2.25 2.25 0 0118 20.25h-2.25a2.25 2.25 0 01-2.25-2.25v-2.25z" />
                    </svg>
                  </div>
                  <p class="font-semibold text-center" style="color: #888; font-size: 0.8vw; margin-top: 0.8vh">Library</p>
                </div>
              </div>
            </section>

            <!-- Downloads -->
            <section v-if="activeDownloads.length > 0" class="mx-auto" style="max-width: 55vw; padding-bottom: 2vh">
              <div style="display: flex; flex-direction: column; gap: 0.6vh">
                <div
                  v-for="item in activeDownloads.slice(0, 3)"
                  :key="item.meta.id"
                  class="flex items-center"
                  :style="{
                    backgroundColor: '#ffffff',
                    borderRadius: '0.6vw',
                    padding: '1vh 1.5vw',
                    gap: '1.5vw',
                    boxShadow: '0 0.1vh 0.4vw rgba(0,0,0,0.06)',
                  }"
                >
                  <div class="flex-1 min-w-0">
                    <p class="font-medium" style="color: #444; font-size: 0.85vw">
                      {{ gameNames[item.meta.id]?.name || item.meta.id }}
                    </p>
                    <div class="rounded-full overflow-hidden" style="margin-top: 0.5vh; height: 0.4vh; background-color: #e0e0e0">
                      <div
                        class="h-full rounded-full transition-all duration-300"
                        style="background-color: #d05028"
                        :style="{ width: `${(item.dl_progress * 100).toFixed(0)}%` }"
                      />
                    </div>
                  </div>
                  <span class="font-medium flex-shrink-0" style="color: #888; font-size: 0.8vw">
                    {{ (item.dl_progress * 100).toFixed(0) }}%
                  </span>
                </div>
              </div>
            </section>
          </template>
        </div>
      </template>


      <!-- ═══════════════════════════════════════════════════════════════════
           DREAMCAST — Sega Dreamcast Dashboard
           Left sidebar nav + 3×2 game grid, teal-aqua gradient bg,
           Sega orange accents, top bar with date/time.
           ═══════════════════════════════════════════════════════════════════ -->
      <template v-else-if="theme === 'dreamcast'">
        <div class="min-h-full flex flex-col relative" :style="{ background: gradientBg || 'var(--bpm-bg)' }">
          <BpmAnimatedBackground :enabled="animBgEnabled" :theme-id="theme" :reduced="reducedAnimations" />
          <!-- Empty state -->
          <div v-if="recentGames.length === 0 && !loading" class="flex items-center justify-center" style="min-height: 70vh">
            <div class="text-center">
              <div class="mx-auto flex items-center justify-center" style="width: 5vw; height: 5vw; border-radius: 50%; background-color: rgba(208,80,16,0.15); margin-bottom: 2vh">
                <PlayIcon style="color: #d05010; opacity: 0.7; width: 2.5vw; height: 2.5vw" />
              </div>
              <p class="font-medium" style="color: #8ab0c0; font-size: 1vw">No games yet</p>
              <p style="color: #5a8898; font-size: 0.85vw; margin-top: 0.5vh">Play a game to see it here</p>
            </div>
          </div>

          <template v-if="recentGames.length > 0">
            <!-- Top bar — Dreamcast-style with swirl logo, user info, date/time -->
            <div class="flex items-center justify-between" style="padding: 1.5vh 2.5vw; background: linear-gradient(90deg, rgba(15,60,75,0.9) 0%, rgba(20,80,100,0.7) 50%, rgba(15,60,75,0.9) 100%); border-bottom: 2px solid rgba(208,80,16,0.4)">
              <div class="flex items-center" style="gap: 1vw">
                <!-- Dreamcast logo -->
                <img :src="usePublicUrl('img/boxart/logos/dreamcast.png')" alt="Dreamcast" style="height: 2.2vw; object-fit: contain; opacity: 0.9; filter: brightness(1.2)" />
              </div>
              <div class="flex items-center" style="gap: 2vw">
                <span style="color: #8ab0c0; font-size: 0.8vw">{{ recentGames.length }} game{{ recentGames.length !== 1 ? 's' : '' }}</span>
                <span class="font-medium" style="color: #d05010; font-size: 0.8vw">{{ new Date().toLocaleDateString('en-US', { month: 'short', day: 'numeric', year: 'numeric' }) }}</span>
              </div>
            </div>

            <!-- Main content: sidebar + game grid -->
            <div class="flex flex-1" style="padding: 2.5vh 2.5vw; gap: 2vw">
              <!-- Left sidebar — menu pills -->
              <div class="flex flex-col flex-shrink-0" style="width: 9vw; gap: 1.2vh; padding-top: 1vh">
                <div
                  class="flex items-center cursor-pointer transition-all duration-150"
                  :style="{
                    padding: '1vh 1vw',
                    borderRadius: '2vw',
                    background: 'linear-gradient(135deg, #c02020, #e03030)',
                    boxShadow: '0 0.2vh 0.8vw rgba(192,32,32,0.3)',
                  }"
                  :ref="(el: any) => registerTile(el, {
                    onSelect: () => router.push('/bigpicture/library'),
                  })"
                >
                  <span class="font-bold" style="color: #fff; font-size: 0.85vw">Play</span>
                </div>
                <div
                  class="flex items-center cursor-pointer transition-all duration-150"
                  :style="{
                    padding: '1vh 1vw',
                    borderRadius: '2vw',
                    background: 'linear-gradient(135deg, #1070a0, #2088c0)',
                    boxShadow: '0 0.2vh 0.8vw rgba(16,112,160,0.3)',
                  }"
                >
                  <span class="font-bold" style="color: #fff; font-size: 0.85vw">Online</span>
                </div>
                <div
                  class="flex items-center cursor-pointer transition-all duration-150"
                  :style="{
                    padding: '1vh 1vw',
                    borderRadius: '2vw',
                    background: 'linear-gradient(135deg, #d07020, #e88830)',
                    boxShadow: '0 0.2vh 0.8vw rgba(208,112,32,0.3)',
                  }"
                >
                  <span class="font-bold" style="color: #fff; font-size: 0.85vw">Media</span>
                </div>
                <div
                  class="flex items-center cursor-pointer transition-all duration-150"
                  :style="{
                    padding: '1vh 1vw',
                    borderRadius: '2vw',
                    background: 'linear-gradient(135deg, #30a040, #40b850)',
                    boxShadow: '0 0.2vh 0.8vw rgba(48,160,64,0.3)',
                  }"
                >
                  <span class="font-bold" style="color: #fff; font-size: 0.85vw">Store</span>
                </div>
                <div
                  class="flex items-center cursor-pointer transition-all duration-150"
                  :style="{
                    padding: '1vh 1vw',
                    borderRadius: '2vw',
                    background: 'linear-gradient(135deg, #606060, #808080)',
                    boxShadow: '0 0.2vh 0.8vw rgba(96,96,96,0.3)',
                  }"
                  :ref="(el: any) => registerTile(el, {
                    onSelect: () => router.push('/bigpicture/settings'),
                  })"
                >
                  <span class="font-bold" style="color: #fff; font-size: 0.85vw">Settings</span>
                </div>
              </div>

              <!-- Right side — game grid -->
              <div class="flex-1">
                <!-- "In Disc Drive" label -->
                <div class="text-center" style="margin-bottom: 1.5vh">
                  <span class="font-medium tracking-wider" style="color: #8ab0c0; font-size: 0.8vw">In Disc Drive</span>
                </div>

                <!-- 3×2 game grid with overlay -->
                <div class="grid grid-cols-3" style="gap: 1.2vw; justify-items: center">
                  <div
                    v-for="(entry, idx) in recentGames.slice(0, 6)"
                    :key="entry.game.id"
                    class="flex flex-col items-center cursor-pointer bp-focus-delegate"
                    :ref="(el: any) => registerTile(el, {
                      onSelect: () => navigateToGame(entry.game.id),
                      onFocus: () => { prefetchGame(entry.game.id); focusedDcIdx = idx; },
                    })"
                  >
                    <div
                      class="relative overflow-hidden transition-all duration-200"
                      :style="{
                        borderRadius: '0.3vw',
                        backgroundColor: '#0a2830',
                        boxShadow: focusedDcIdx === idx
                          ? '0 0 1.5vw rgba(230,100,30,0.8), 0 0 3.5vw rgba(230,100,30,0.4)'
                          : '0 0.3vh 1vw rgba(0,0,0,0.3)',
                        border: focusedDcIdx === idx
                          ? '2px solid rgba(230,100,30,0.9)'
                          : '1px solid rgba(30,100,120,0.4)',
                        aspectRatio: '1/1',
                        height: '26vh',
                      }"
                    >
                      <img
                        v-if="entry.game.mCoverObjectId"
                        :src="useObject(entry.game.mCoverObjectId)"
                        :alt="entry.game.mName"
                        class="absolute inset-0 w-full h-full object-cover"
                      />
                      <div
                        v-else
                        class="absolute inset-0 flex items-center justify-center"
                        style="background: linear-gradient(135deg, #d05010, #e87030)"
                      >
                        <span class="font-bold" style="color: #fff; font-size: 2.5vw">
                          {{ entry.game.mName.charAt(0).toUpperCase() }}
                        </span>
                      </div>
                      <BpmBoxArtOverlay :theme-id="theme" />
                    </div>
                    <!-- Game name -->
                    <p
                      v-if="!hideTitles"
                      class="text-center truncate w-full transition-colors duration-200"
                      :style="{
                        fontSize: '0.8vw',
                        marginTop: '0.8vh',
                        color: focusedDcIdx === idx ? '#e8f0f4' : '#6a98a8',
                      }"
                    >
                      {{ entry.game.mName }}
                    </p>
                  </div>
                </div>
              </div>
            </div>

            <!-- Bottom bar — controller button prompts -->
            <div class="flex items-center justify-center" style="padding: 1.5vh 2.5vw; gap: 3vw; background: linear-gradient(to top, rgba(6,40,48,0.95), transparent)">
              <span style="color: #5a8898; font-size: 0.8vw">
                <span style="color: #d05010; font-weight: 600; margin-right: 0.3vw">A</span> Select
              </span>
              <span style="color: #5a8898; font-size: 0.8vw">
                <span style="color: #e87030; font-weight: 600; margin-right: 0.3vw">B</span> Back
              </span>
              <span style="color: #5a8898; font-size: 0.8vw">
                <span style="color: #f09050; font-weight: 600; margin-right: 0.3vw">X</span> Fullscreen
              </span>
            </div>

            <!-- Downloads -->
            <section v-if="activeDownloads.length > 0" style="padding: 0 3vw 2vh 3vw">
              <div style="display: flex; flex-direction: column; gap: 0.6vh">
                <div
                  v-for="item in activeDownloads.slice(0, 3)"
                  :key="item.meta.id"
                  class="flex items-center"
                  style="background-color: rgba(10,40,48,0.9); border-radius: 0.5vw; padding: 1vh 1.5vw; gap: 1.5vw; border: 1px solid rgba(30,100,120,0.3)"
                >
                  <div class="flex-1 min-w-0">
                    <p class="font-medium" style="color: #e8f0f4; font-size: 0.85vw">
                      {{ gameNames[item.meta.id]?.name || item.meta.id }}
                    </p>
                    <div class="rounded-full overflow-hidden" style="margin-top: 0.6vh; height: 0.35vh; background-color: #0e3848">
                      <div
                        class="h-full rounded-full transition-all duration-300"
                        style="background-color: #d05010"
                        :style="{ width: `${(item.dl_progress * 100).toFixed(0)}%` }"
                      />
                    </div>
                  </div>
                  <span class="font-medium flex-shrink-0" style="color: #8ab0c0; font-size: 0.8vw">
                    {{ (item.dl_progress * 100).toFixed(0) }}%
                  </span>
                </div>
              </div>
            </section>
          </template>
        </div>
      </template>


      <!-- ═══════════════════════════════════════════════════════════════════
           GAMECUBE — Indigo & Purple Aesthetic
           Centered cube-inspired layout with 2×2 grid of game zones.
           ═══════════════════════════════════════════════════════════════════ -->
      <template v-else-if="theme === 'gamecube'">
        <div class="relative min-h-full" style="background-color: var(--bpm-bg)">
          <BpmAnimatedBackground :enabled="animBgEnabled" :theme-id="theme" :reduced="reducedAnimations" />
          <div class="relative" style="z-index: 1">
          <!-- Empty state -->
          <div v-if="recentGames.length === 0 && !loading" class="flex items-center justify-center" style="min-height: 70vh">
            <div class="text-center">
              <div class="mx-auto flex items-center justify-center" style="width: 5vw; height: 5vw; border-radius: 50%; background-color: rgba(82,78,170,0.15); margin-bottom: 2vh">
                <PlayIcon style="color: #524EAA; opacity: 0.7; width: 2.5vw; height: 2.5vw" />
              </div>
              <p class="font-medium" style="color: #a8a0d0; font-size: 1vw">No games yet</p>
              <p style="color: #6a5890; font-size: 0.85vw; margin-top: 0.5vh">Play a game to see it here</p>
            </div>
          </div>

          <template v-if="recentGames.length > 0">
            <!-- Header with GameCube branding -->
            <div class="flex items-center justify-between" style="padding: 2vh 3vw; background: linear-gradient(90deg, rgba(40,30,80,0.8) 0%, rgba(60,40,120,0.6) 50%, rgba(40,30,80,0.8) 100%); border-bottom: 2px solid rgba(82,78,170,0.4)">
              <img :src="usePublicUrl('img/boxart/logos/gamecube.png')" alt="GameCube" style="height: 2.2vw; object-fit: contain; opacity: 0.9; filter: brightness(1.2)" />
              <div class="flex items-center" style="gap: 2vw">
                <BpmClock :theme-id="theme" :show-date="true" />
                <span style="color: #7a6aa0; font-size: 0.8vw">{{ recentGames.length }} game{{ recentGames.length !== 1 ? 's' : '' }}</span>
              </div>
            </div>

            <!-- Main content: 4-column grid -->
            <div class="flex-1 flex items-center justify-center" style="padding: 3vh 4vw">
              <div class="grid grid-cols-4 w-full" style="gap: 1.5vw; max-width: 75vw; justify-items: center">
                <div
                  v-for="(entry, idx) in recentGames.slice(0, 12)"
                  :key="entry.game.id"
                  class="flex flex-col items-center cursor-pointer bp-focus-delegate"
                  :ref="(el: any) => registerTile(el, {
                    onSelect: () => navigateToGame(entry.game.id),
                    onFocus: () => { prefetchGame(entry.game.id); focusedGcIdx = idx; },
                  })"
                >
                  <div
                    class="relative overflow-hidden transition-all duration-150"
                    :style="{
                      aspectRatio: '514/732',
                      height: '28vh',
                      borderRadius: '0.3vw',
                      backgroundColor: '#1a1530',
                      boxShadow: focusedGcIdx === idx
                        ? '0 0 1.5vw rgba(120,110,220,0.8), 0 0 3.5vw rgba(100,90,200,0.4)'
                        : '0 0.3vh 1vw rgba(0,0,0,0.3)',
                      border: focusedGcIdx === idx
                        ? '2px solid rgba(140,130,240,0.9)'
                        : '1px solid rgba(50,40,100,0.4)',
                    }"
                  >
                    <img
                      v-if="entry.game.mCoverObjectId"
                      :src="useObject(entry.game.mCoverObjectId)"
                      :alt="entry.game.mName"
                      class="absolute inset-0 w-full h-full object-cover"
                    />
                    <div
                      v-else
                      class="absolute inset-0 flex items-center justify-center"
                      style="background: linear-gradient(135deg, #524EAA, #6a5890)"
                    >
                      <span class="font-bold" style="color: #fff; font-size: 2.5vw">
                        {{ entry.game.mName.charAt(0).toUpperCase() }}
                      </span>
                    </div>
                    <BpmBoxArtOverlay :theme-id="theme" />
                  </div>
                  <p
                    v-if="!hideTitles"
                    class="text-center truncate w-full transition-colors duration-200"
                    :style="{
                      fontSize: '0.85vw',
                      marginTop: '1vh',
                      color: focusedGcIdx === idx ? '#e8d0ff' : '#7a6aa0',
                    }"
                  >
                    {{ entry.game.mName }}
                  </p>
                </div>
              </div>
            </div>

            <!-- Downloads -->
            <section v-if="activeDownloads.length > 0" style="padding: 0 3vw 2vh 3vw">
              <div style="display: flex; flex-direction: column; gap: 0.6vh">
                <div
                  v-for="item in activeDownloads.slice(0, 3)"
                  :key="item.meta.id"
                  class="flex items-center"
                  style="background-color: rgba(20,15,50,0.9); border-radius: 0.5vw; padding: 1vh 1.5vw; gap: 1.5vw; border: 1px solid rgba(82,78,170,0.3)"
                >
                  <div class="flex-1 min-w-0">
                    <p class="font-medium" style="color: #c8b0ff; font-size: 0.85vw">
                      {{ gameNames[item.meta.id]?.name || item.meta.id }}
                    </p>
                    <div class="rounded-full overflow-hidden" style="margin-top: 0.6vh; height: 0.35vh; background-color: #1a1530">
                      <div
                        class="h-full rounded-full transition-all duration-300"
                        style="background-color: #524EAA"
                        :style="{ width: `${(item.dl_progress * 100).toFixed(0)}%` }"
                      />
                    </div>
                  </div>
                  <span class="font-medium flex-shrink-0" style="color: #a8a0d0; font-size: 0.8vw">
                    {{ (item.dl_progress * 100).toFixed(0) }}%
                  </span>
                </div>
              </div>
            </section>
          </template>
          </div><!-- /z-index content wrapper -->
        </div>
      </template>


      <!-- ═══════════════════════════════════════════════════════════════════
           PSP — XMB-inspired Horizontal Bar Layout
           Black background with blue-grey panels and horizontal scrolling.
           ═══════════════════════════════════════════════════════════════════ -->
      <template v-else-if="theme === 'psp'">
        <div class="relative min-h-full flex flex-col" style="background-color: var(--bpm-bg)">
          <BpmAnimatedBackground :enabled="animBgEnabled" :theme-id="theme" :reduced="reducedAnimations" />
          <div class="relative flex-1 flex flex-col" style="z-index: 1">
          <!-- Empty state -->
          <div v-if="recentGames.length === 0 && !loading" class="flex items-center justify-center" style="min-height: 70vh">
            <div class="text-center">
              <div class="mx-auto flex items-center justify-center" style="width: 5vw; height: 5vw; border-radius: 0.5vw; background-color: rgba(60,80,120,0.15); margin-bottom: 2vh">
                <PlayIcon style="color: #3C5078; opacity: 0.7; width: 2.5vw; height: 2.5vw" />
              </div>
              <p class="font-medium" style="color: #6a8aaa; font-size: 1vw">No games yet</p>
              <p style="color: #4a6a8a; font-size: 0.85vw; margin-top: 0.5vh">Play a game to see it here</p>
            </div>
          </div>

          <template v-if="recentGames.length > 0">
            <!-- Top category bar -->
            <div class="flex items-center" style="padding: 1.5vh 2vw; background: linear-gradient(90deg, rgba(30,40,60,0.9) 0%, rgba(40,60,90,0.7) 50%, rgba(30,40,60,0.9) 100%); border-bottom: 1px solid rgba(60,80,120,0.3); gap: 2.5vw">
              <span class="font-bold" style="color: #5a7a9a; font-size: 0.9vw">Games</span>
              <span style="color: #4a5a7a; font-size: 0.9vw">Music</span>
              <span style="color: #4a5a7a; font-size: 0.9vw">Video</span>
              <span style="color: #4a5a7a; font-size: 0.9vw">Settings</span>
              <div style="margin-left: auto">
                <BpmClock :theme-id="theme" :show-date="false" />
              </div>
            </div>

            <!-- Game grid — 6 columns, tall UMD cases (462x801 aspect ratio) -->
            <div class="flex-1 flex items-center justify-center" style="padding: 2vh 2vw">
              <div class="grid grid-cols-6 mx-auto" style="gap: 1vw; max-width: 82vw; justify-items: center">
                <div
                  v-for="(entry, idx) in recentGames"
                  :key="entry.game.id"
                  class="flex flex-col items-center cursor-pointer bp-focus-delegate"
                  :ref="(el: any) => registerTile(el, {
                    onSelect: () => navigateToGame(entry.game.id),
                    onFocus: () => { prefetchGame(entry.game.id); focusedPspIdx = idx; },
                  })"
                >
                  <!-- PSP UMD case — tall aspect ratio (462x801) -->
                  <div
                    class="relative overflow-hidden transition-all duration-150"
                    :style="{
                      aspectRatio: '462/801',
                      height: '32vh',
                      borderRadius: '0.3vw',
                      backgroundColor: '#0a1520',
                      boxShadow: focusedPspIdx === idx
                        ? '0 0 1.5vw rgba(100,140,220,0.8), 0 0 3.5vw rgba(80,120,200,0.4)'
                        : '0 0.2vh 0.8vw rgba(0,0,0,0.3)',
                      border: focusedPspIdx === idx
                        ? '2px solid rgba(120,160,240,0.9)'
                        : '1px solid rgba(40,60,90,0.2)',
                    }"
                  >
                    <img
                      v-if="entry.game.mCoverObjectId"
                      :src="useObject(entry.game.mCoverObjectId)"
                      :alt="entry.game.mName"
                      class="absolute inset-0 w-full h-full object-cover"
                    />
                    <div
                      v-else
                      class="absolute inset-0 flex items-center justify-center"
                      style="background: linear-gradient(135deg, #3C5078, #5a7a9a)"
                    >
                      <span class="font-bold" style="color: #fff; font-size: 1.5vw">
                        {{ entry.game.mName.charAt(0).toUpperCase() }}
                      </span>
                    </div>
                    <BpmBoxArtOverlay :theme-id="theme" />
                  </div>
                  <p
                    v-if="!hideTitles"
                    class="truncate text-center w-full"
                    :style="{ color: focusedPspIdx === idx ? '#c8d8e8' : '#7a8aaa', fontSize: '0.75vw', fontWeight: '500', marginTop: '0.6vh' }"
                  >
                    {{ entry.game.mName }}
                  </p>
                </div>
              </div>
            </div>

            <!-- Downloads -->
            <section v-if="activeDownloads.length > 0" style="padding: 1vh 2vw 2vh 2vw">
              <div style="display: flex; flex-direction: column; gap: 0.6vh">
                <div
                  v-for="item in activeDownloads.slice(0, 2)"
                  :key="item.meta.id"
                  class="flex items-center"
                  style="background-color: rgba(20,30,50,0.9); border-radius: 0.4vw; padding: 0.8vh 1.2vw; gap: 1vw; border: 1px solid rgba(60,80,120,0.2)"
                >
                  <div class="flex-1 min-w-0">
                    <p class="font-medium" style="color: #8a9aaa; font-size: 0.8vw">
                      {{ gameNames[item.meta.id]?.name || item.meta.id }}
                    </p>
                    <div class="rounded-full overflow-hidden" style="margin-top: 0.4vh; height: 0.3vh; background-color: #0a1520">
                      <div
                        class="h-full rounded-full transition-all duration-300"
                        style="background-color: #3C5078"
                        :style="{ width: `${(item.dl_progress * 100).toFixed(0)}%` }"
                      />
                    </div>
                  </div>
                  <span class="font-medium flex-shrink-0" style="color: #6a8aaa; font-size: 0.75vw">
                    {{ (item.dl_progress * 100).toFixed(0) }}%
                  </span>
                </div>
              </div>
            </section>
          </template>
          </div><!-- /z-index content wrapper -->
        </div>
      </template>


      <!-- ═══════════════════════════════════════════════════════════════════
           GAME BOY — Classic DMG Green Pixel Art Aesthetic
           3×3 grid with thick pixelated borders and retro styling.
           ═══════════════════════════════════════════════════════════════════ -->
      <template v-else-if="theme === 'gameboy'">
        <div class="relative min-h-full flex flex-col" style="background-color: var(--bpm-bg); color: var(--bpm-text)">
          <BpmAnimatedBackground :enabled="animBgEnabled" :theme-id="theme" :reduced="reducedAnimations" />
          <div class="relative flex-1 flex flex-col" style="z-index: 1">
          <!-- Empty state -->
          <div v-if="recentGames.length === 0 && !loading" class="flex items-center justify-center" style="min-height: 70vh">
            <div class="text-center">
              <div class="mx-auto flex items-center justify-center" style="width: 5vw; height: 5vw; border: 0.3vw solid #9BBC0F; margin-bottom: 2vh">
                <PlayIcon style="color: #9BBC0F; opacity: 0.7; width: 2.5vw; height: 2.5vw" />
              </div>
              <p class="font-medium" style="color: #9BBC0F; font-size: 0.6vw">NO GAMES</p>
              <p style="color: #5a7a2f; font-size: 0.45vw; margin-top: 0.5vh">PLAY TO START</p>
            </div>
          </div>

          <template v-if="recentGames.length > 0">
            <!-- Header -->
            <div class="flex items-center justify-between" style="padding: 1.5vh 2vw; border-bottom: 0.4vw solid #9BBC0F; background-color: rgba(9,32,15,0.5)">
              <img :src="usePublicUrl('img/boxart/logos/gameboy.png')" alt="Game Boy" style="height: 1.8vw; object-fit: contain; opacity: 0.9" />
              <span style="color: #9BBC0F; font-size: 0.5vw">{{ recentGames.length }} GAMES</span>
            </div>

            <!-- 4×4 grid with chunky borders — nearly square cartridge boxes (509x494 aspect ratio) -->
            <div class="flex-1 flex items-center justify-center" style="padding: 2.5vh 3vw">
              <div class="grid grid-cols-4 w-full" style="gap: 0.8vw; max-width: 60vw; justify-items: center">
                <div
                  v-for="(entry, idx) in recentGames.slice(0, 16)"
                  :key="entry.game.id"
                  class="flex flex-col items-center cursor-pointer bp-focus-delegate"
                  :ref="(el: any) => registerTile(el, {
                    onSelect: () => navigateToGame(entry.game.id),
                    onFocus: () => { prefetchGame(entry.game.id); focusedGbIdx = idx; },
                  })"
                >
                  <!-- Game Boy cartridge box — nearly square (509x494) -->
                  <div
                    class="relative overflow-hidden transition-all duration-150"
                    :style="{
                      aspectRatio: '509/494',
                      height: '18vh',
                      borderRadius: '0.2vw',
                      border: focusedGbIdx === idx ? '0.4vw solid #9BBC0F' : '0.3vw solid #9BBC0F',
                      backgroundColor: '#051810',
                      boxShadow: focusedGbIdx === idx ? '0 0 1.5vw rgba(155,188,15,0.8), 0 0 3.5vw rgba(155,188,15,0.4)' : 'none',
                    }"
                  >
                    <img
                      v-if="entry.game.mCoverObjectId"
                      :src="useObject(entry.game.mCoverObjectId)"
                      :alt="entry.game.mName"
                      class="absolute inset-0 w-full h-full object-cover"
                    />
                    <div
                      v-else
                      class="absolute inset-0 flex items-center justify-center"
                      style="background-color: #0F380F"
                    >
                      <span class="font-bold" style="color: #9BBC0F; font-size: 1.5vw">
                        {{ entry.game.mName.charAt(0).toUpperCase() }}
                      </span>
                    </div>
                    <BpmBoxArtOverlay :theme-id="theme" />
                  </div>
                  <p
                    v-if="!hideTitles"
                    class="text-center truncate w-full transition-colors duration-200 mt-1"
                    :style="{
                      fontSize: '0.45vw',
                      color: '#9BBC0F',
                      letterSpacing: '0.05vw',
                    }"
                  >
                    {{ entry.game.mName.substring(0, 8) }}
                  </p>
                </div>
              </div>
            </div>

            <!-- Downloads -->
            <section v-if="activeDownloads.length > 0" style="padding: 0 3vw 2vh 3vw">
              <div style="display: flex; flex-direction: column; gap: 0.6vh">
                <div
                  v-for="item in activeDownloads.slice(0, 2)"
                  :key="item.meta.id"
                  class="flex items-center"
                  style="background-color: rgba(9,32,15,0.9); border: 0.2vw solid #9BBC0F; border-radius: 0.2vw; padding: 0.8vh 1.2vw; gap: 1vw"
                >
                  <div class="flex-1 min-w-0">
                    <p class="font-medium" style="color: #9BBC0F; font-size: 0.5vw">
                      {{ gameNames[item.meta.id]?.name || item.meta.id }}
                    </p>
                    <div class="rounded overflow-hidden" style="margin-top: 0.4vh; height: 0.3vh; background-color: #051810">
                      <div
                        class="h-full transition-all duration-300"
                        style="background-color: #9BBC0F"
                        :style="{ width: `${(item.dl_progress * 100).toFixed(0)}%` }"
                      />
                    </div>
                  </div>
                  <span class="font-medium flex-shrink-0" style="color: #9BBC0F; font-size: 0.5vw">
                    {{ (item.dl_progress * 100).toFixed(0) }}%
                  </span>
                </div>
              </div>
            </section>
          </template>
          </div><!-- /z-index content wrapper -->
        </div>
      </template>




      <!-- ═══════════════════════════════════════════════════════════════════
           SNES — Light Grey with Colorful Button Categories
           2×2 button grid for main categories using SNES controller colors.
           ═══════════════════════════════════════════════════════════════════ -->
      <template v-else-if="theme === 'snes'">
        <div class="relative min-h-full flex flex-col" style="background-color: var(--bpm-bg)">
          <BpmAnimatedBackground :enabled="animBgEnabled" :theme-id="theme" :reduced="reducedAnimations" />
          <div class="relative flex-1 flex flex-col" style="z-index: 1">
          <!-- Empty state -->
          <div v-if="recentGames.length === 0 && !loading" class="flex items-center justify-center" style="min-height: 70vh">
            <div class="text-center">
              <div class="mx-auto flex items-center justify-center" style="width: 5vw; height: 5vw; border-radius: 0.6vw; background: linear-gradient(135deg, #ea3323, #2862c8, #5ab033, #f5c51b); margin-bottom: 2vh">
                <PlayIcon style="color: #f5f5f5; opacity: 0.8; width: 2.5vw; height: 2.5vw" />
              </div>
              <p class="font-medium" style="color: #3a3a4a; font-size: 1vw">No games yet</p>
              <p style="color: #6a6a7a; font-size: 0.85vw; margin-top: 0.5vh">Start your adventure!</p>
            </div>
          </div>

          <template v-if="recentGames.length > 0">
            <!-- SNES-style header -->
            <div class="flex items-center justify-between" style="padding: 1.5vh 3vw; background: linear-gradient(90deg, rgba(180,180,190,0.5) 0%, rgba(200,200,210,0.3) 50%, rgba(180,180,190,0.5) 100%); border-bottom: 2px solid rgba(100,100,110,0.3)">
              <img :src="usePublicUrl('img/boxart/logos/snes.png')" alt="SNES" style="height: 2.2vw; object-fit: contain; opacity: 0.85" />
              <div class="flex items-center" style="gap: 2vw">
                <BpmClock :theme-id="theme" :show-date="true" />
                <span style="color: #5a5a6a; font-size: 0.8vw">{{ recentGames.length }} game{{ recentGames.length !== 1 ? 's' : '' }}</span>
              </div>
            </div>

            <!-- Main content: category buttons + game shelf -->
            <div class="flex-1 flex flex-col items-center justify-center" style="padding: 2.5vh 4vw">
              <!-- 2×2 colorful button grid -->
              <div class="grid grid-cols-2 w-full" style="gap: 1.5vw; max-width: 50vw; justify-items: center; margin-bottom: 3vh">
                <div
                  class="flex items-center justify-center cursor-pointer transition-all duration-150"
                  style="height: 6vh; width: 20vw; border-radius: 0.8vw; background: linear-gradient(135deg, #ea3323, #f05050); box-shadow: 0 0.3vh 1vw rgba(234,51,35,0.3); font-weight: bold; color: #fff; font-size: 1vw"
                >
                  Sports
                </div>
                <div
                  class="flex items-center justify-center cursor-pointer transition-all duration-150"
                  style="height: 6vh; width: 20vw; border-radius: 0.8vw; background: linear-gradient(135deg, #2862c8, #4080e8); box-shadow: 0 0.3vh 1vw rgba(40,98,200,0.3); font-weight: bold; color: #fff; font-size: 1vw"
                >
                  Action
                </div>
                <div
                  class="flex items-center justify-center cursor-pointer transition-all duration-150"
                  style="height: 6vh; width: 20vw; border-radius: 0.8vw; background: linear-gradient(135deg, #5ab033, #70d050); box-shadow: 0 0.3vh 1vw rgba(90,176,51,0.3); font-weight: bold; color: #fff; font-size: 1vw"
                >
                  RPG
                </div>
                <div
                  class="flex items-center justify-center cursor-pointer transition-all duration-150"
                  style="height: 6vh; width: 20vw; border-radius: 0.8vw; background: linear-gradient(135deg, #f5c51b, #f5d550); box-shadow: 0 0.3vh 1vw rgba(245,197,27,0.3); font-weight: bold; color: #2a2a3a; font-size: 1vw"
                >
                  Puzzle
                </div>
              </div>

              <!-- Game shelf — 3 columns landscape cartridge boxes (730x508 aspect ratio) -->
              <div style="width: 100%; margin-top: 2vh">
                <span class="font-bold" style="color: #3a3a4a; font-size: 0.9vw; padding: 0 3vw; display: block; margin-bottom: 1.5vh">All Games</span>
                <div class="grid grid-cols-3 mx-auto" style="gap: 1.2vw; max-width: 70vw; justify-items: center; padding: 0 3vw 1vh 3vw">
                  <div
                    v-for="(entry, idx) in recentGames.slice(0, 9)"
                    :key="entry.game.id"
                    class="flex flex-col items-center cursor-pointer bp-focus-delegate"
                    :ref="(el: any) => registerTile(el, {
                      onSelect: () => navigateToGame(entry.game.id),
                      onFocus: () => { prefetchGame(entry.game.id); focusedSnesIdx = idx; },
                    })"
                  >
                    <!-- SNES cartridge box — landscape (730x508) -->
                    <div
                      class="relative overflow-hidden transition-all duration-150"
                      :style="{
                        aspectRatio: '730/508',
                        height: '18vh',
                        borderRadius: '0.2vw',
                        backgroundColor: '#d0d0d8',
                        boxShadow: focusedSnesIdx === idx
                          ? '0 0 1.5vw rgba(100,100,200,0.8), 0 0 3.5vw rgba(100,100,200,0.4)'
                          : '0 0.2vh 0.8vw rgba(0,0,0,0.15)',
                        border: focusedSnesIdx === idx
                          ? '2px solid rgba(120,120,220,0.9)'
                          : '1px solid rgba(150,150,160,0.4)',
                      }"
                    >
                      <img
                        v-if="entry.game.mCoverObjectId"
                        :src="useObject(entry.game.mCoverObjectId)"
                        :alt="entry.game.mName"
                        class="absolute inset-0 w-full h-full object-cover"
                      />
                      <div
                        v-else
                        class="absolute inset-0 flex items-center justify-center"
                        style="background: linear-gradient(135deg, #6464B4, #8080c8)"
                      >
                        <span class="font-bold" style="color: #fff; font-size: 2.5vw">
                          {{ entry.game.mName.charAt(0).toUpperCase() }}
                        </span>
                      </div>
                      <BpmBoxArtOverlay :theme-id="theme" />
                    </div>
                    <p
                      v-if="!hideTitles"
                      class="text-center truncate w-full transition-colors duration-200"
                      :style="{
                        fontSize: '0.8vw',
                        marginTop: '0.6vh',
                        color: focusedSnesIdx === idx ? '#2a2a3a' : '#5a5a6a',
                      }"
                    >
                      {{ entry.game.mName }}
                    </p>
                  </div>
                </div>
              </div>
            </div>

            <!-- Downloads -->
            <section v-if="activeDownloads.length > 0" style="padding: 0 3vw 2vh 3vw">
              <div style="display: flex; flex-direction: column; gap: 0.6vh">
                <div
                  v-for="item in activeDownloads.slice(0, 2)"
                  :key="item.meta.id"
                  class="flex items-center"
                  style="background-color: rgba(220,220,225,0.9); border-radius: 0.4vw; padding: 0.8vh 1.2vw; gap: 1vw; border: 1px solid rgba(100,100,110,0.3)"
                >
                  <div class="flex-1 min-w-0">
                    <p class="font-medium" style="color: #3a3a4a; font-size: 0.8vw">
                      {{ gameNames[item.meta.id]?.name || item.meta.id }}
                    </p>
                    <div class="rounded overflow-hidden" style="margin-top: 0.4vh; height: 0.3vh; background-color: #d0d0d8">
                      <div
                        class="h-full transition-all duration-300"
                        style="background: linear-gradient(90deg, #ea3323, #2862c8, #5ab033)"
                        :style="{ width: `${(item.dl_progress * 100).toFixed(0)}%` }"
                      />
                    </div>
                  </div>
                  <span class="font-medium flex-shrink-0" style="color: #5a5a6a; font-size: 0.75vw">
                    {{ (item.dl_progress * 100).toFixed(0) }}%
                  </span>
                </div>
              </div>
            </section>
          </template>
          </div><!-- /z-index content wrapper -->
        </div>
      </template>

    </div>
  </div>
</template>

<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { PlayIcon } from "@heroicons/vue/24/outline";
import BigPictureGameTile from "~/components/bigpicture/BigPictureGameTile.vue";
import BpmClock from "~/components/bigpicture/BpmClock.vue";
import BpmScreensaver from "~/components/bigpicture/BpmScreensaver.vue";
import BpmCrtFilter from "~/components/bigpicture/BpmCrtFilter.vue";
import BpmAnimatedBackground from "~/components/bigpicture/BpmAnimatedBackground.vue";
import BpmAchievementToast from "~/components/bigpicture/BpmAchievementToast.vue";
import BpmLaunchScreen from "~/components/bigpicture/BpmLaunchScreen.vue";
import BpmBoxArtOverlay from "~/components/bigpicture/BpmBoxArtOverlay.vue";
import { parseStatus, deduplicatedInvoke } from "~/composables/game";
import { useBpFocusableGroup } from "~/composables/bp-focusable";
import { useFocusNavigation } from "~/composables/focus-navigation";
import { useQueueState } from "~/composables/downloads";
import { serverUrl } from "~/composables/use-server-fetch";
import { useGame } from "~/composables/game";
import { useBpmTheme } from "~/composables/bp-theme";
import { useObject } from "~/composables/use-object";
import { useBpmClock } from "~/composables/bp-clock";
import { useBpmIdle } from "~/composables/bp-idle";
import { useBpmAmbient } from "~/composables/bp-ambient";
import { useBpmWelcome } from "~/composables/bp-welcome";
import { useBpmCursors } from "~/composables/bp-cursors";
import type { Game, GameStatus, RawGameStatus } from "~/types";

definePageMeta({ layout: "bigpicture" });

interface RecentGameEntry {
  game: Game;
  status: GameStatus;
}

interface RecentGameResponse {
  gameId: string;
  gameName: string;
  coverObjectId: string | null;
  lastPlayedAt: string;
  totalPlaytimeSeconds: number;
}

const theme = ref<string>("steam");
const recentGames = ref<RecentGameEntry[]>([]);
const loading = ref(true);

// User profile for themes that show profile info (Xbox, etc.)
const appState = useAppState();
const userProfilePicUrl = computed(() => {
  const picId = appState.value?.user?.profilePictureObjectId;
  return picId ? serverUrl(`api/v1/object/${picId}`) : null;
});
const scrollContainer = ref<HTMLElement | null>(null);
const focusNav = useFocusNavigation();
const registerTile = useBpFocusableGroup("content");
const registerQuickLink = useBpFocusableGroup("content");
const router = useRouter();
const route = useRoute();
const gameNames = ref<Record<string, { name: string; coverUrl?: string }>>({});
const accentColor = ref<string>("#3b82f6");

// ── Hide titles setting ───────────────────────────────────────────────
const hideTitles = ref(
  typeof localStorage !== "undefined"
    ? localStorage.getItem("drop:hideTitles") === "true"
    : false,
);

// ── Theme-specific focus tracking ──────────────────────────────────────
const focusedSwitchIdx = ref(0);
const focusedSwitchGame = computed(() => recentGames.value[focusedSwitchIdx.value] ?? null);

const ps2FocusedIdx = ref(0);
const ps2FocusedGame = computed(() => recentGames.value[ps2FocusedIdx.value] ?? null);

const focusedXboxIdx = ref(0);
const focusedWiiIdx = ref(0);
const focusedDsIdx = ref(0);
const focusedDcIdx = ref(0);

// ── New feature state ──
const crtEnabled = ref(
  typeof localStorage !== "undefined" ? localStorage.getItem("bpm:crtFilter") === "true" : false
);
const animBgEnabled = ref(
  typeof localStorage !== "undefined" ? localStorage.getItem("bpm:animBg") !== "false" : true
);
// Reduce animation complexity on low-power devices (Steam Deck / gamescope)
const { isSteamDeckHardware } = useDeckMode();
const reducedAnimations = computed(() => isSteamDeckHardware.value);
const currentAchievement = ref<{ title: string; game: string; icon?: string } | null>(null);
const launchingGame = ref<{ name: string; coverUrl?: string } | null>(null);
const screensaverEnabled = ref(
  typeof localStorage !== "undefined" ? localStorage.getItem("bpm:screensaver") !== "false" : true
);
const { isIdle } = useBpmIdle(120000);
const bpmClock = useBpmClock();
const ambient = useBpmAmbient();
const welcome = useBpmWelcome(recentGames as any);
const cursors = useBpmCursors();

// Focus tracking for new themes
const focusedGcIdx = ref(0);
const focusedPspIdx = ref(0);
const focusedGbIdx = ref(0);
const focusedSnesIdx = ref(0);

// Theme detection
const themeComposable = useBpmTheme();
const themeColors: Record<string, string> = {
  steam: "#66c0f4",
  xbox: "#107c10",
  wii: "#34beed",
  ps2: "#2040c0",
  ds: "#d05028",
  dreamcast: "#d05010",
  gamecube: "#524EAA",
  psp: "#3C5078",
  gameboy: "#9BBC0F",
  snes: "#6464B4",
};

// Gradient backgrounds for themes that use them — mode-aware
const themeGradientBgs: Record<string, { dark: string; light: string }> = {
  wii: {
    dark: "linear-gradient(180deg, #1a2a3a 0%, #142430 50%, #0e1c28 100%)",
    light: "linear-gradient(180deg, #b8d8f0 0%, #a8cce8 50%, #98c0e0 100%)",
  },
  ps2: {
    dark: "linear-gradient(160deg, #080818 0%, #0c0c28 30%, #101040 60%, #080820 100%)",
    light: "linear-gradient(160deg, #c8c8e0 0%, #b8b8d8 30%, #a8a8d0 60%, #b8b8e0 100%)",
  },
  ds: {
    dark: "linear-gradient(180deg, #1a1a1e 0%, #141418 100%)",
    light: "linear-gradient(180deg, #d0ccc8 0%, #c0b8b4 100%)",
  },
  dreamcast: {
    dark: "linear-gradient(135deg, #0c1820 0%, #0e3040 30%, #0a2838 60%, #062030 100%)",
    light: "linear-gradient(135deg, #b8d8d8 0%, #a8d0d0 30%, #98c8c8 60%, #a8d0d0 100%)",
  },
};
const gradientBg = computed(() => {
  const entry = themeGradientBgs[theme.value];
  if (!entry) return undefined;
  return entry[themeComposable.mode.value];
});

watchEffect(() => {
  theme.value = themeComposable.themeId.value;
  accentColor.value = themeColors[theme.value] ?? "#66c0f4";
});

// Download state
const queueState = useQueueState();
const queue = computed(() => queueState.value?.queue ?? []);
const activeDownloads = computed(() => queue.value.filter((item) => item.status !== "Completed"));

// Computed properties
const spotlightGame = computed(() => recentGames.value[0] ?? null);
const otherGames = computed(() => recentGames.value.slice(1));
const installedGames = computed(() => recentGames.value.filter((e) => e.status.installed));

function prefetchGame(gameId: string) {
  deduplicatedInvoke("fetch_game", { gameId }).catch(() => {});
}

function navigateToGame(gameId: string) {
  focusNav.saveFocusSnapshot(route.path);
  router.push(`/bigpicture/library/${gameId}`).catch((e: any) => {
    console.error(`[BPM:HOME] Navigation FAILED for ${gameId}:`, e);
  });
}

function formatPlaytime(seconds: number): string {
  if (seconds < 3600) {
    const minutes = Math.round(seconds / 60);
    return `${minutes}m`;
  }
  const hours = Math.round(seconds / 3600);
  return `${hours}h`;
}

async function loadRecentGames() {
  try {
    const url = serverUrl("api/v1/client/playtime/recent");
    console.log("[BPM:HOME] Fetching recent games from:", url);
    const response = await fetch(url);
    if (!response.ok) {
      console.error("[BPM:HOME] Recent games fetch failed:", response.status, response.statusText);
      recentGames.value = [];
      return;
    }
    const recentData = await response.json() as RecentGameResponse[];
    console.log("[BPM:HOME] Got recent games data:", JSON.stringify(recentData).slice(0, 200));

    if (!Array.isArray(recentData)) {
      console.warn("[BPM:HOME] Recent games response is not an array:", typeof recentData);
      recentGames.value = [];
      return;
    }

    const gamesToLoad = recentData.slice(0, 20);
    const entries: RecentGameEntry[] = [];

    for (const gameData of gamesToLoad) {
      try {
        const statusData: RawGameStatus = await invoke("fetch_game_status", { id: gameData.gameId });
        const game: Game = {
          id: gameData.gameId,
          mName: gameData.gameName,
          mCoverObjectId: gameData.coverObjectId,
          mTaglineUrl: null,
          mReleaseDate: null,
          mPlatformId: null,
          mSummary: null,
          mBackgroundUrl: null,
          mPublisher: null,
          mGenre: null,
        } as Game;

        entries.push({
          game,
          status: parseStatus(statusData),
        });
      } catch (e) {
        console.error(`Failed to load recent game ${gameData.gameId}:`, e);
      }
    }

    recentGames.value = entries;

    for (const item of queue.value) {
      if (!gameNames.value[item.meta.id]) {
        try {
          const gameFetch = await useGame(item.meta.id);
          gameNames.value[item.meta.id] = {
            name: gameFetch.game.mName,
            coverUrl: gameFetch.game.mCoverObjectId
              ? serverUrl(`api/v1/object/${gameFetch.game.mCoverObjectId}`)
              : undefined,
          };
        } catch {
          // Game data not available
        }
      }
    }
  } catch (e) {
    console.error("[BPM:HOME] Failed to fetch recent games:", e);
  } finally {
    loading.value = false;
  }
}

console.log("[BPM:HOME] Active theme:", theme.value);

onMounted(async () => {
  await loadRecentGames();
  if (!focusNav.restoreFocusSnapshot(route.path)) {
    focusNav.autoFocusContent("content");
  }
});
</script>
