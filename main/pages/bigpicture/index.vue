<template>
  <!-- Scrolling belongs to the BPM layout's single [data-bp-scroll] container;
       everything below is a plain flex child. -->
  <div
    class="flex flex-col min-h-full"
    :style="{ backgroundColor: 'var(--bpm-bg)', color: 'var(--bpm-text)' }"
  >
    <!-- Global BPM overlays -->
    <BpmScreensaver :active="screensaverEnabled && isIdle" @dismiss="isIdle = false" />
    <BpmCrtFilter v-if="crtEnabled" :enabled="true" :intensity="0.4" />
    <BpmAchievementToast
      v-if="currentAchievement"
      :theme-id="theme"
      :achievement="currentAchievement"
      @dismissed="currentAchievement = null"
    />
    <BpmLaunchScreen
      v-if="launchingGame"
      :theme-id="theme"
      :game="launchingGame"
      @ready="launchingGame = null"
    />

    <!-- One skeleton for all ten themes, ahead of the theme chain. Held back
         ~180ms so a warm start goes straight to the real home screen. -->
    <BpmHomeSkeleton v-if="showSkeleton" />

    <template v-else-if="!pageLoading">
      <!-- ═══════════════════════════════════════════════════════════════════
           SPOTLIGHT HERO — the one skinned zone.
           Ten console skins, one behaviour: art + name + playtime + a primary
           action. Everything below the hero is theme-neutral markup driven by
           the --bpm-* tokens, which is how the library finally inherits the
           theme instead of being the one un-themed BPM page.
           ═══════════════════════════════════════════════════════════════════ -->
      <section
        v-if="heroEntry && !searching"
        class="relative overflow-hidden shrink-0"
        :style="{ background: gradientBg || 'var(--bpm-bg)' }"
      >
        <BpmAnimatedBackground
          v-if="themeHasAnimatedBg"
          :enabled="animBgEnabled"
          :theme-id="theme"
          :reduced="reducedAnimations"
        />
        <div class="relative" style="z-index: 1">
          <!-- ── XBOX — Series X|S dashboard spotlight ────────────────────── -->
          <template v-if="theme === 'xbox'">
            <div
              class="flex items-center justify-between"
              style="padding: 1.5vh 2.5vw; border-bottom: 1px solid rgba(255,255,255,0.06)"
            >
              <div class="flex items-center" style="gap: 1vw">
                <div
                  class="flex items-center justify-center overflow-hidden"
                  style="width: 2vw; height: 2vw; border-radius: 50%; background-color: #107c10"
                >
                  <img v-if="userProfilePicUrl" :src="userProfilePicUrl" class="w-full h-full object-cover" />
                  <svg v-else style="width: 1vw; height: 1vw" fill="#fff" viewBox="0 0 24 24"><path d="M12 12c2.21 0 4-1.79 4-4s-1.79-4-4-4-4 1.79-4 4 1.79 4 4 4zm0 2c-2.67 0-8 1.34-8 4v2h16v-2c0-2.66-5.33-4-8-4z"/></svg>
                </div>
                <span class="font-semibold" style="color: #fff; font-size: 0.9vw">Home</span>
              </div>
              <span style="color: #666; font-size: 0.75vw">{{ gameCountLabel }}</span>
            </div>

            <div style="padding: 2vh 2.5vw">
              <div class="relative rounded-lg overflow-hidden" style="height: 42vh">
                <div class="absolute inset-0" style="background-color: #1a1a1a">
                  <img
                    v-if="heroArtId"
                    :src="objectUrl(heroArtId)"
                    :alt="heroName"
                    class="w-full h-full object-cover opacity-80"
                  />
                </div>
                <div class="absolute inset-0" style="background: linear-gradient(to top, rgba(0,0,0,0.9) 0%, transparent 50%)" />
                <div class="absolute bottom-0 left-0" style="padding: 3vh 2.5vw">
                  <h3 class="font-bold" style="color: #fff; font-size: 2vw; margin-bottom: 1vh">{{ heroName }}</h3>
                  <p v-if="heroPlaytimeLabel" style="color: #b0b0b0; font-size: 0.9vw; margin-bottom: 2vh">
                    {{ heroPlaytimeLabel }}
                  </p>
                  <div class="flex items-center" style="gap: 1vw">
                    <button
                      :ref="registerHeroAction"
                      class="rounded-sm font-bold tracking-wide text-white transition-colors"
                      style="background-color: #107c10; padding: 0.8vh 2vw; font-size: 0.9vw"
                      @click="heroAction"
                    >
                      {{ heroActionLabel }}
                    </button>
                    <button
                      v-if="showHeroDetails"
                      :ref="registerHeroDetails"
                      class="rounded-sm font-semibold transition-colors"
                      style="background-color: rgba(255,255,255,0.12); color: #fff; padding: 0.8vh 1.6vw; font-size: 0.85vw"
                      @click="openHeroGame"
                    >
                      Details
                    </button>
                  </div>
                </div>
              </div>
            </div>
          </template>

          <!-- ── WII — Wii Menu channel ───────────────────────────────────── -->
          <template v-else-if="theme === 'wii'">
            <div class="flex items-center justify-between" style="padding: 1.5vh 3vw; background: rgba(255,255,255,0.3)">
              <span class="font-bold" style="color: #4a7090; font-size: 1vw">Wii Menu</span>
              <span style="color: #6a8aaa; font-size: 0.85vw">
                {{ library.length }} channel{{ library.length !== 1 ? "s" : "" }}
              </span>
            </div>

            <div class="flex items-center" style="gap: 3vw; padding: 3vh 4vw 1vh 4vw">
              <div
                class="relative flex-shrink-0 overflow-hidden"
                style="
                  aspect-ratio: 515/731;
                  height: 32vh;
                  border-radius: 0.3vw;
                  background: linear-gradient(155deg, #ffffff 0%, #f4f8fc 100%);
                  box-shadow: 0 0.4vh 1.2vw rgba(0,0,0,0.10), 0 0.15vh 0.4vw rgba(0,0,0,0.06), inset 0 1px 0 rgba(255,255,255,0.9);
                  border: 1px solid rgba(180,200,220,0.4);
                "
              >
                <img v-if="heroCoverId" :src="objectUrl(heroCoverId)" :alt="heroName" class="absolute inset-0 w-full h-full object-cover" />
                <div v-else class="absolute inset-0 flex items-center justify-center" style="background: linear-gradient(135deg, #34beed, #009ac7)">
                  <span class="font-bold" style="color: #ffffff; font-size: 2.5vw">{{ heroInitial }}</span>
                </div>
                <BpmBoxArtOverlay v-if="showBoxArtOverlays" :theme-id="theme" />
              </div>
              <div style="max-width: 34vw">
                <p class="font-medium tracking-wider uppercase" style="color: #34beed; font-size: 0.75vw; margin-bottom: 1vh">Continue Playing</p>
                <h2 class="font-bold" style="color: #3a5068; font-size: 2.2vw; margin-bottom: 1vh; text-shadow: 0 1px 0 rgba(255,255,255,0.6)">{{ heroName }}</h2>
                <p v-if="heroPlaytimeLabel" style="color: #6a8aaa; font-size: 0.85vw; margin-bottom: 2vh">{{ heroPlaytimeLabel }}</p>
                <div class="flex items-center" style="gap: 1vw">
                  <button
                    :ref="registerHeroAction"
                    class="font-bold transition-all"
                    style="background: linear-gradient(135deg, #34beed, #009ac7); color: #fff; padding: 0.9vh 2.5vw; font-size: 0.9vw; border-radius: 2vw; box-shadow: 0 0.3vh 1vw rgba(52,190,237,0.3)"
                    @click="heroAction"
                  >
                    {{ heroActionLabel }}
                  </button>
                  <button
                    v-if="showHeroDetails"
                    :ref="registerHeroDetails"
                    class="font-semibold transition-all"
                    style="background: linear-gradient(155deg, #f0f5fa 0%, #e4ecf4 100%); color: #4a7090; padding: 0.9vh 1.8vw; font-size: 0.85vw; border-radius: 2vw; border: 1px solid rgba(180,200,220,0.4)"
                    @click="openHeroGame"
                  >
                    Details
                  </button>
                </div>
              </div>
            </div>

            <!-- Wii Menu page dots -->
            <div class="flex justify-center" style="padding-bottom: 2vh; gap: 0.6vw">
              <div class="rounded-full" style="width: 0.6vw; height: 0.6vw; background-color: #34beed" />
              <div class="rounded-full" style="width: 0.6vw; height: 0.6vw; background-color: rgba(52,190,237,0.25)" />
            </div>
          </template>

          <!-- ── PS2 — Memory Card browser ────────────────────────────────── -->
          <template v-else-if="theme === 'ps2'">
            <!-- Decorative vertical lines — PS2 towers effect -->
            <div class="absolute inset-0 pointer-events-none" style="opacity: 0.04; background: repeating-linear-gradient(90deg, transparent, transparent 3vw, #4060ff 3vw, #4060ff 3.05vw)" />

            <div class="relative" style="padding: 2vh 3.5vw 1.5vh 3.5vw; background: linear-gradient(180deg, rgba(20,20,60,0.6) 0%, transparent 100%)">
              <div class="flex items-center justify-between">
                <div class="flex items-center" style="gap: 1.2vw">
                  <!-- Memory card icon -->
                  <div class="flex items-center justify-center" style="width: 2.2vw; height: 3vw; background: linear-gradient(145deg, #1a1a40, #12122a); border-radius: 0.3vw; border: 1px solid #2a2a58">
                    <div style="width: 1vw; height: 0.5vw; background-color: #3040a0; border-radius: 0.1vw" />
                  </div>
                  <div>
                    <p class="font-medium tracking-wide" style="color: #d0d0f0; font-size: 1.1vw">Memory Card (PS2)</p>
                    <p style="color: #5060a0; font-size: 0.75vw">
                      {{ library.length }} file{{ library.length !== 1 ? "s" : "" }}
                    </p>
                  </div>
                </div>
                <!-- Live readout of whatever the Continue Playing rail has focused -->
                <div v-if="focusedRecent" class="text-right">
                  <p class="font-medium" style="color: #b0b8e0; font-size: 0.95vw">{{ focusedRecent.game.mName }}</p>
                  <p v-if="focusedRecent.installed" style="color: #4060c0; font-size: 0.75vw">
                    {{ formatPlaytime(focusedRecent.playtimeSeconds) }} played
                  </p>
                </div>
              </div>
            </div>

            <!-- Glowing blue divider -->
            <div style="height: 1px; background: linear-gradient(90deg, transparent 5%, #3050c0 30%, #5070e0 50%, #3050c0 70%, transparent 95%); margin: 0 2vw" />

            <div class="relative flex items-center" style="gap: 3vw; padding: 3vh 3.5vw">
              <div
                class="relative flex-shrink-0 overflow-hidden"
                style="aspect-ratio: 515/732; height: 30vh; border-radius: 0.3vw; background-color: #0c0c24; box-shadow: 0 0 1.5vw rgba(80,130,255,0.5), 0 0 3.5vw rgba(60,100,240,0.25); border: 1px solid rgba(100,150,255,0.5)"
              >
                <img v-if="heroCoverId" :src="objectUrl(heroCoverId)" :alt="heroName" class="absolute inset-0 w-full h-full object-cover" />
                <div v-else class="absolute inset-0 flex items-center justify-center font-bold" style="background: linear-gradient(135deg, #2040c0, #4060e0); color: #c8d0f0; font-size: 2vw">
                  {{ heroInitial }}
                </div>
                <BpmBoxArtOverlay v-if="showBoxArtOverlays" :theme-id="theme" />
              </div>
              <div style="max-width: 34vw">
                <p class="font-medium tracking-widest uppercase" style="color: #5070e0; font-size: 0.75vw; margin-bottom: 1vh">Continue Playing</p>
                <h2 class="font-bold" style="color: #c8d0f0; font-size: 2.2vw; margin-bottom: 1vh">{{ heroName }}</h2>
                <p v-if="heroPlaytimeLabel" style="color: #5060a0; font-size: 0.85vw; margin-bottom: 2vh">{{ heroPlaytimeLabel }}</p>
                <div class="flex items-center" style="gap: 1vw">
                  <button
                    :ref="registerHeroAction"
                    class="font-bold tracking-wide"
                    style="background: linear-gradient(135deg, #3050c0, #5070e0); color: #e8ecff; padding: 0.8vh 2.5vw; font-size: 0.9vw; border-radius: 0.3vw"
                    @click="heroAction"
                  >
                    {{ heroActionLabel }}
                  </button>
                  <button
                    v-if="showHeroDetails"
                    :ref="registerHeroDetails"
                    class="font-semibold"
                    style="background-color: rgba(16,16,48,0.9); color: #8090c0; padding: 0.8vh 1.8vw; font-size: 0.85vw; border-radius: 0.3vw; border: 1px solid rgba(48,80,192,0.5)"
                    @click="openHeroGame"
                  >
                    Details
                  </button>
                </div>
              </div>
            </div>
          </template>

          <!-- ── DS — dual-screen top half ────────────────────────────────── -->
          <template v-else-if="theme === 'ds'">
            <section class="relative" style="background: linear-gradient(180deg, #c8c8c8 0%, #b8b8b8 100%)">
              <div class="flex items-center justify-between" style="padding: 0.8vh 2vw; background: rgba(0,0,0,0.05)">
                <span class="font-bold" style="color: #777; font-size: 0.75vw">TOP SCREEN</span>
                <span style="color: #999; font-size: 0.7vw">{{ gameCountLabel }}</span>
              </div>
              <!-- DS hinge — thick gradient line between screens -->
              <div class="absolute bottom-0 inset-x-0" style="height: 0.6vh; background: linear-gradient(90deg, #999, #888, #777, #888, #999); box-shadow: 0 1px 3px rgba(0,0,0,0.15); z-index: 1" />

              <div class="flex items-center justify-center" style="gap: 3vw; padding: 2.5vh 4vw 3vh 4vw">
                <div
                  class="relative flex-shrink-0 overflow-hidden"
                  style="width: 14vw; max-height: 42vh; border-radius: 1vw; background-color: #a0a0a0; box-shadow: 0 0.3vh 1.5vw rgba(0,0,0,0.12)"
                >
                  <img v-if="heroCoverId" :src="objectUrl(heroCoverId)" :alt="heroName" class="w-full block" />
                  <div v-else class="flex items-center justify-center" style="aspect-ratio: 571/600; background: linear-gradient(135deg, #d05028, #e06030)">
                    <span class="font-bold" style="color: #fff; font-size: 2vw">{{ heroInitial }}</span>
                  </div>
                </div>
                <div style="max-width: 30vw">
                  <p class="font-medium tracking-wider uppercase" style="color: #d05028; font-size: 0.75vw; margin-bottom: 1vh">Now Playing</p>
                  <h2 class="font-bold" style="color: #333; font-size: 2.2vw; margin-bottom: 1vh">{{ heroName }}</h2>
                  <p v-if="heroPlaytimeLabel" style="color: #666; font-size: 0.85vw; margin-bottom: 2vh">{{ heroPlaytimeLabel }}</p>
                  <div class="flex items-center" style="gap: 1vw">
                    <button
                      :ref="registerHeroAction"
                      class="rounded-lg font-bold tracking-wide transition-colors"
                      style="background: linear-gradient(135deg, #d05028, #e06030); color: #fff; padding: 0.8vh 2.5vw; font-size: 0.9vw"
                      @click="heroAction"
                    >
                      {{ heroActionLabel }}
                    </button>
                    <button
                      v-if="showHeroDetails"
                      :ref="registerHeroDetails"
                      class="rounded-lg font-semibold transition-colors"
                      style="background: linear-gradient(145deg, #e8e8e8, #d8d8d8); color: #555; padding: 0.8vh 1.8vw; font-size: 0.85vw"
                      @click="openHeroGame"
                    >
                      Details
                    </button>
                  </div>
                </div>
              </div>
            </section>
          </template>

          <!-- ── DREAMCAST — disc drive panel ─────────────────────────────── -->
          <template v-else-if="theme === 'dreamcast'">
            <div
              class="flex items-center justify-between"
              style="padding: 1.5vh 2.5vw; background: linear-gradient(90deg, rgba(15,60,75,0.9) 0%, rgba(20,80,100,0.7) 50%, rgba(15,60,75,0.9) 100%); border-bottom: 2px solid rgba(208,80,16,0.4)"
            >
              <img :src="usePublicUrl('img/boxart/logos/dreamcast.png')" alt="Dreamcast" style="height: 2.2vw; object-fit: contain; opacity: 0.9; filter: brightness(1.2)" />
              <div class="flex items-center" style="gap: 2vw">
                <span style="color: #8ab0c0; font-size: 0.8vw">{{ gameCountLabel }}</span>
                <span class="font-medium" style="color: #d05010; font-size: 0.8vw">{{ todayLabel }}</span>
              </div>
            </div>

            <div class="text-center" style="padding-top: 2vh">
              <span class="font-medium tracking-wider" style="color: #8ab0c0; font-size: 0.8vw">In Disc Drive</span>
            </div>

            <div class="flex items-center justify-center" style="gap: 3vw; padding: 2vh 3vw 3vh 3vw">
              <div
                class="relative flex-shrink-0 overflow-hidden"
                style="aspect-ratio: 1/1; height: 28vh; border-radius: 0.3vw; background-color: #0a2830; box-shadow: 0 0 1.5vw rgba(230,100,30,0.5), 0 0 3.5vw rgba(230,100,30,0.25); border: 1px solid rgba(230,100,30,0.6)"
              >
                <img v-if="heroCoverId" :src="objectUrl(heroCoverId)" :alt="heroName" class="absolute inset-0 w-full h-full object-cover" />
                <div v-else class="absolute inset-0 flex items-center justify-center" style="background: linear-gradient(135deg, #d05010, #e87030)">
                  <span class="font-bold" style="color: #fff; font-size: 2.5vw">{{ heroInitial }}</span>
                </div>
                <BpmBoxArtOverlay v-if="showBoxArtOverlays" :theme-id="theme" />
              </div>
              <div style="max-width: 32vw">
                <p class="font-medium tracking-widest uppercase" style="color: #d05010; font-size: 0.75vw; margin-bottom: 1vh">Continue Playing</p>
                <h2 class="font-bold" style="color: #e8f0f4; font-size: 2.2vw; margin-bottom: 1vh">{{ heroName }}</h2>
                <p v-if="heroPlaytimeLabel" style="color: #8ab0c0; font-size: 0.85vw; margin-bottom: 2vh">{{ heroPlaytimeLabel }}</p>
                <div class="flex items-center" style="gap: 1vw">
                  <button
                    :ref="registerHeroAction"
                    class="font-bold transition-all"
                    style="background: linear-gradient(135deg, #c02020, #e03030); color: #fff; padding: 0.9vh 2.5vw; font-size: 0.9vw; border-radius: 2vw; box-shadow: 0 0.2vh 0.8vw rgba(192,32,32,0.3)"
                    @click="heroAction"
                  >
                    {{ heroActionLabel }}
                  </button>
                  <button
                    v-if="showHeroDetails"
                    :ref="registerHeroDetails"
                    class="font-bold transition-all"
                    style="background: linear-gradient(135deg, #1070a0, #2088c0); color: #fff; padding: 0.9vh 1.8vw; font-size: 0.85vw; border-radius: 2vw; box-shadow: 0 0.2vh 0.8vw rgba(16,112,160,0.3)"
                    @click="openHeroGame"
                  >
                    Details
                  </button>
                </div>
              </div>
            </div>
          </template>

          <!-- ── GAMECUBE — indigo & purple ───────────────────────────────── -->
          <template v-else-if="theme === 'gamecube'">
            <div
              class="flex items-center justify-between"
              style="padding: 2vh 3vw; background: linear-gradient(90deg, rgba(40,30,80,0.8) 0%, rgba(60,40,120,0.6) 50%, rgba(40,30,80,0.8) 100%); border-bottom: 2px solid rgba(82,78,170,0.4)"
            >
              <img :src="usePublicUrl('img/boxart/logos/gamecube.png')" alt="GameCube" style="height: 2.2vw; object-fit: contain; opacity: 0.9; filter: brightness(1.2)" />
              <div class="flex items-center" style="gap: 2vw">
                <BpmClock :theme-id="theme" :show-date="true" />
                <span style="color: #7a6aa0; font-size: 0.8vw">{{ gameCountLabel }}</span>
              </div>
            </div>

            <div class="flex items-center justify-center" style="gap: 3vw; padding: 3vh 4vw">
              <div
                class="relative flex-shrink-0 overflow-hidden"
                style="aspect-ratio: 514/732; height: 30vh; border-radius: 0.3vw; background-color: #1a1530; box-shadow: 0 0 1.5vw rgba(120,110,220,0.55), 0 0 3.5vw rgba(100,90,200,0.25); border: 1px solid rgba(140,130,240,0.6)"
              >
                <img v-if="heroCoverId" :src="objectUrl(heroCoverId)" :alt="heroName" class="absolute inset-0 w-full h-full object-cover" />
                <div v-else class="absolute inset-0 flex items-center justify-center" style="background: linear-gradient(135deg, #524EAA, #6a5890)">
                  <span class="font-bold" style="color: #fff; font-size: 2.5vw">{{ heroInitial }}</span>
                </div>
                <BpmBoxArtOverlay v-if="showBoxArtOverlays" :theme-id="theme" />
              </div>
              <div style="max-width: 32vw">
                <p class="font-medium tracking-widest uppercase" style="color: #8c82f0; font-size: 0.75vw; margin-bottom: 1vh">Continue Playing</p>
                <h2 class="font-bold" style="color: #e8d0ff; font-size: 2.2vw; margin-bottom: 1vh">{{ heroName }}</h2>
                <p v-if="heroPlaytimeLabel" style="color: #a8a0d0; font-size: 0.85vw; margin-bottom: 2vh">{{ heroPlaytimeLabel }}</p>
                <div class="flex items-center" style="gap: 1vw">
                  <button
                    :ref="registerHeroAction"
                    class="font-bold tracking-wide transition-all"
                    style="background: linear-gradient(135deg, #524EAA, #6a5890); color: #fff; padding: 0.9vh 2.5vw; font-size: 0.9vw; border-radius: 0.6vw; box-shadow: 0 0.3vh 1vw rgba(82,78,170,0.4)"
                    @click="heroAction"
                  >
                    {{ heroActionLabel }}
                  </button>
                  <button
                    v-if="showHeroDetails"
                    :ref="registerHeroDetails"
                    class="font-semibold transition-all"
                    style="background-color: rgba(20,15,50,0.9); color: #c8b0ff; padding: 0.9vh 1.8vw; font-size: 0.85vw; border-radius: 0.6vw; border: 1px solid rgba(82,78,170,0.5)"
                    @click="openHeroGame"
                  >
                    Details
                  </button>
                </div>
              </div>
            </div>
          </template>

          <!-- ── PSP — XMB bar ────────────────────────────────────────────── -->
          <template v-else-if="theme === 'psp'">
            <div
              class="flex items-center"
              style="padding: 1.5vh 2vw; background: linear-gradient(90deg, rgba(30,40,60,0.9) 0%, rgba(40,60,90,0.7) 50%, rgba(30,40,60,0.9) 100%); border-bottom: 1px solid rgba(60,80,120,0.3); gap: 2.5vw"
            >
              <span class="font-bold" style="color: #5a7a9a; font-size: 0.9vw">Games</span>
              <span style="color: #4a5a7a; font-size: 0.9vw">Music</span>
              <span style="color: #4a5a7a; font-size: 0.9vw">Video</span>
              <span style="color: #4a5a7a; font-size: 0.9vw">Settings</span>
              <div style="margin-left: auto">
                <BpmClock :theme-id="theme" :show-date="false" />
              </div>
            </div>

            <div class="flex items-center" style="gap: 3vw; padding: 3vh 3vw">
              <div
                class="relative flex-shrink-0 overflow-hidden"
                style="aspect-ratio: 462/801; height: 32vh; border-radius: 0.3vw; background-color: #0a1520; box-shadow: 0 0 1.5vw rgba(100,140,220,0.55), 0 0 3.5vw rgba(80,120,200,0.25); border: 1px solid rgba(120,160,240,0.6)"
              >
                <img v-if="heroCoverId" :src="objectUrl(heroCoverId)" :alt="heroName" class="absolute inset-0 w-full h-full object-cover" />
                <div v-else class="absolute inset-0 flex items-center justify-center" style="background: linear-gradient(135deg, #3C5078, #5a7a9a)">
                  <span class="font-bold" style="color: #fff; font-size: 1.5vw">{{ heroInitial }}</span>
                </div>
                <BpmBoxArtOverlay v-if="showBoxArtOverlays" :theme-id="theme" />
              </div>
              <div style="max-width: 34vw">
                <p class="font-medium tracking-widest uppercase" style="color: #6a8aaa; font-size: 0.7vw; margin-bottom: 1vh">Continue Playing</p>
                <h2 class="font-bold" style="color: #c8d8e8; font-size: 2vw; margin-bottom: 1vh">{{ heroName }}</h2>
                <p v-if="heroPlaytimeLabel" style="color: #7a8aaa; font-size: 0.8vw; margin-bottom: 2vh">{{ heroPlaytimeLabel }}</p>
                <div class="flex items-center" style="gap: 1vw">
                  <button
                    :ref="registerHeroAction"
                    class="font-bold tracking-wide"
                    style="background: linear-gradient(135deg, #3C5078, #5a7a9a); color: #e8f0f8; padding: 0.8vh 2.5vw; font-size: 0.85vw; border-radius: 0.3vw"
                    @click="heroAction"
                  >
                    {{ heroActionLabel }}
                  </button>
                  <button
                    v-if="showHeroDetails"
                    :ref="registerHeroDetails"
                    class="font-semibold"
                    style="background-color: rgba(20,30,50,0.9); color: #8a9aaa; padding: 0.8vh 1.8vw; font-size: 0.8vw; border-radius: 0.3vw; border: 1px solid rgba(60,80,120,0.4)"
                    @click="openHeroGame"
                  >
                    Details
                  </button>
                </div>
              </div>
            </div>
          </template>

          <!-- ── GAME BOY — DMG green ─────────────────────────────────────── -->
          <template v-else-if="theme === 'gameboy'">
            <div class="flex items-center justify-between" style="padding: 1.5vh 2vw; border-bottom: 0.4vw solid #9BBC0F; background-color: rgba(9,32,15,0.5)">
              <img :src="usePublicUrl('img/boxart/logos/gameboy.png')" alt="Game Boy" style="height: 1.8vw; object-fit: contain; opacity: 0.9" />
              <span style="color: #9BBC0F; font-size: 0.5vw">{{ library.length }} GAMES</span>
            </div>

            <div class="flex items-center" style="gap: 3vw; padding: 3vh 3vw">
              <div
                class="relative flex-shrink-0 overflow-hidden"
                style="aspect-ratio: 509/494; height: 24vh; border-radius: 0.2vw; border: 0.4vw solid #9BBC0F; background-color: #051810; box-shadow: 0 0 1.5vw rgba(155,188,15,0.6), 0 0 3.5vw rgba(155,188,15,0.3)"
              >
                <img v-if="heroCoverId" :src="objectUrl(heroCoverId)" :alt="heroName" class="absolute inset-0 w-full h-full object-cover" />
                <div v-else class="absolute inset-0 flex items-center justify-center" style="background-color: #0F380F">
                  <span class="font-bold" style="color: #9BBC0F; font-size: 1.5vw">{{ heroInitial }}</span>
                </div>
                <BpmBoxArtOverlay v-if="showBoxArtOverlays" :theme-id="theme" />
              </div>
              <div style="max-width: 34vw">
                <p style="color: #5a7a2f; font-size: 0.45vw; letter-spacing: 0.05vw; margin-bottom: 1vh">CONTINUE</p>
                <h2 class="font-bold" style="color: #9BBC0F; font-size: 1.2vw; letter-spacing: 0.05vw; margin-bottom: 1vh">{{ heroName }}</h2>
                <p v-if="heroPlaytimeLabel" style="color: #5a7a2f; font-size: 0.5vw; margin-bottom: 2vh">{{ heroPlaytimeLabel }}</p>
                <div class="flex items-center" style="gap: 1vw">
                  <button
                    :ref="registerHeroAction"
                    class="font-bold uppercase"
                    style="background-color: #0F380F; color: #9BBC0F; border: 0.3vw solid #9BBC0F; padding: 0.8vh 2vw; font-size: 0.5vw; letter-spacing: 0.08vw; border-radius: 0.2vw"
                    @click="heroAction"
                  >
                    {{ heroActionLabel }}
                  </button>
                  <button
                    v-if="showHeroDetails"
                    :ref="registerHeroDetails"
                    class="font-bold uppercase"
                    style="background-color: rgba(9,32,15,0.9); color: #5a7a2f; border: 0.2vw solid #5a7a2f; padding: 0.8vh 1.5vw; font-size: 0.5vw; letter-spacing: 0.08vw; border-radius: 0.2vw"
                    @click="openHeroGame"
                  >
                    Details
                  </button>
                </div>
              </div>
            </div>
          </template>

          <!-- ── SNES — light grey with controller-button colours ─────────── -->
          <template v-else-if="theme === 'snes'">
            <div
              class="flex items-center justify-between"
              style="padding: 1.5vh 3vw; background: linear-gradient(90deg, rgba(180,180,190,0.5) 0%, rgba(200,200,210,0.3) 50%, rgba(180,180,190,0.5) 100%); border-bottom: 2px solid rgba(100,100,110,0.3)"
            >
              <img :src="usePublicUrl('img/boxart/logos/snes.png')" alt="SNES" style="height: 2.2vw; object-fit: contain; opacity: 0.85" />
              <div class="flex items-center" style="gap: 2vw">
                <BpmClock :theme-id="theme" :show-date="true" />
                <span style="color: #5a5a6a; font-size: 0.8vw">{{ gameCountLabel }}</span>
              </div>
            </div>

            <div class="flex items-center justify-center" style="gap: 3vw; padding: 3vh 4vw">
              <div
                class="relative flex-shrink-0 overflow-hidden"
                style="aspect-ratio: 730/508; height: 24vh; border-radius: 0.2vw; background-color: #d0d0d8; box-shadow: 0 0 1.5vw rgba(100,100,200,0.55), 0 0 3.5vw rgba(100,100,200,0.25); border: 1px solid rgba(120,120,220,0.7)"
              >
                <img v-if="heroCoverId" :src="objectUrl(heroCoverId)" :alt="heroName" class="absolute inset-0 w-full h-full object-cover" />
                <div v-else class="absolute inset-0 flex items-center justify-center" style="background: linear-gradient(135deg, #6464B4, #8080c8)">
                  <span class="font-bold" style="color: #fff; font-size: 2.5vw">{{ heroInitial }}</span>
                </div>
                <BpmBoxArtOverlay v-if="showBoxArtOverlays" :theme-id="theme" />
              </div>
              <div style="max-width: 32vw">
                <p class="font-bold tracking-widest uppercase" style="color: #6464B4; font-size: 0.75vw; margin-bottom: 1vh">Continue Playing</p>
                <h2 class="font-bold" style="color: #2a2a3a; font-size: 2.2vw; margin-bottom: 1vh">{{ heroName }}</h2>
                <p v-if="heroPlaytimeLabel" style="color: #5a5a6a; font-size: 0.85vw; margin-bottom: 2vh">{{ heroPlaytimeLabel }}</p>
                <div class="flex items-center" style="gap: 1vw">
                  <button
                    :ref="registerHeroAction"
                    class="font-bold transition-all"
                    style="background: linear-gradient(135deg, #2862c8, #4080e8); color: #fff; padding: 0.9vh 2.5vw; font-size: 0.9vw; border-radius: 0.8vw; box-shadow: 0 0.3vh 1vw rgba(40,98,200,0.3)"
                    @click="heroAction"
                  >
                    {{ heroActionLabel }}
                  </button>
                  <button
                    v-if="showHeroDetails"
                    :ref="registerHeroDetails"
                    class="font-bold transition-all"
                    style="background: linear-gradient(135deg, #5ab033, #70d050); color: #fff; padding: 0.9vh 1.8vw; font-size: 0.85vw; border-radius: 0.8vw; box-shadow: 0 0.3vh 1vw rgba(90,176,51,0.3)"
                    @click="openHeroGame"
                  >
                    Details
                  </button>
                </div>
              </div>
            </div>
          </template>

          <!-- ── STEAM — Big Picture banner. Also the fallback skin, so an
               unrecognised theme id lands on Drop's default rather than on a
               blank hero. ─────────────────────────────────────────────────── -->
          <template v-else>
            <div class="flex items-center justify-between" style="padding: 1vh 3vw; background-color: #1b2838; border-bottom: 1px solid #2a475e">
              <div class="flex items-center" style="gap: 2vw">
                <span class="font-bold tracking-wider uppercase" style="color: #66c0f4; font-size: 0.75vw">Home</span>
                <span style="color: #4f6a80; font-size: 0.75vw">|</span>
                <span class="tracking-wider uppercase" style="color: #4f6a80; font-size: 0.75vw">Library</span>
              </div>
              <span style="color: #4f6a80; font-size: 0.75vw">{{ gameCountLabel }}</span>
            </div>

            <div class="relative w-full" style="height: 53vh; min-height: 14rem">
              <div class="absolute inset-0" style="background-color: #1b2838">
                <img
                  v-if="heroArtId"
                  :src="objectUrl(heroArtId)"
                  :alt="heroName"
                  class="w-full h-full object-cover"
                  style="opacity: 0.45"
                />
              </div>
              <div class="absolute inset-0" style="background: linear-gradient(to top, #171a21 0%, transparent 60%)" />
              <div class="absolute inset-0" style="background: linear-gradient(to right, #171a21 0%, transparent 40%)" />
              <!-- Subtle horizontal line accent at bottom -->
              <div class="absolute bottom-0 inset-x-0" style="height: 2px; background: linear-gradient(90deg, #66c0f4, transparent 60%); opacity: 0.4" />
              <div class="absolute bottom-0 left-0" style="padding: 3vh 3vw; max-width: 40vw">
                <p class="font-bold tracking-widest uppercase" style="color: #66c0f4; font-size: 0.8vw; margin-bottom: 1vh">Continue Playing</p>
                <h2 class="font-bold" style="color: #c7d5e0; font-size: 2.5vw; margin-bottom: 1vh">{{ heroName }}</h2>
                <p v-if="heroPlaytimeLabel" style="color: #8f98a0; font-size: 0.9vw; margin-bottom: 2vh">{{ heroPlaytimeLabel }}</p>
                <div class="flex items-center" style="gap: 1vw">
                  <button
                    :ref="registerHeroAction"
                    class="rounded font-bold tracking-wide transition-all hover:brightness-110 cursor-pointer"
                    style="background: linear-gradient(135deg, #1a9fff, #0d84e0); color: #fff; padding: 0.8vh 2.5vw; font-size: 0.9vw; box-shadow: 0 0.3vh 1vw rgba(26,159,255,0.3)"
                    @click="heroAction"
                  >
                    {{ heroActionLabel }}
                  </button>
                  <button
                    v-if="showHeroDetails"
                    :ref="registerHeroDetails"
                    class="rounded font-semibold transition-all hover:brightness-110 cursor-pointer"
                    style="background-color: rgba(103,193,245,0.15); color: #c7d5e0; padding: 0.8vh 1.8vw; font-size: 0.85vw"
                    @click="openHeroGame"
                  >
                    Details
                  </button>
                </div>
              </div>
            </div>
          </template>
        </div>
      </section>

      <!-- ═══════════════════════════════════════════════════════════════════
           RAILS — horizontal, D-pad navigable in both axes. A search collapses
           them so a query gets one clean answer instead of five places to look.
           ═══════════════════════════════════════════════════════════════════ -->
      <template v-if="!searching">
        <!-- Continue Playing -->
        <section v-if="continueEntries.length > 0" class="px-8 pt-6">
          <div class="flex items-baseline gap-3 mb-3">
            <h2 class="text-lg font-semibold font-display" style="color: var(--bpm-text)">Continue Playing</h2>
            <span class="text-xs tabular-nums" style="color: var(--bpm-muted)">{{ continueEntries.length }}</span>
          </div>
          <div class="bpm-rail flex gap-4 overflow-x-auto pb-2 px-1 pt-1">
            <div
              v-for="(entry, idx) in continueEntries"
              :key="entry.game.id"
              class="flex-shrink-0 bp-focus-delegate cursor-pointer"
              style="width: 10rem"
              :ref="
                (el: any) =>
                  registerTile(el, {
                    onSelect: () => navigateToGame(entry.game.id),
                    onContext: () => openContextMenu(entry),
                    onFocus: () => { prefetchGame(entry.game.id); focusedIdx = idx; },
                  })
              "
            >
              <BigPictureGameTile
                :game="entry.game"
                :status="entry.status"
                :hide-titles="hideTitles"
                :overlay-theme-id="tileOverlayTheme"
              />
            </div>
          </div>
        </section>

        <!-- Updates available -->
        <section v-if="updatesEntries.length > 0" class="px-8 pt-6">
          <div class="flex items-baseline gap-3 mb-3">
            <h2 class="text-lg font-semibold font-display" style="color: var(--bpm-text)">Updates Available</h2>
            <span class="text-xs tabular-nums" style="color: var(--bpm-muted)">{{ updatesEntries.length }}</span>
          </div>
          <div class="bpm-rail flex gap-4 overflow-x-auto pb-2 px-1 pt-1">
            <div
              v-for="entry in updatesEntries"
              :key="entry.game.id"
              class="flex-shrink-0 bp-focus-delegate cursor-pointer"
              style="width: 10rem"
              :ref="
                (el: any) =>
                  registerTile(el, {
                    onSelect: () => navigateToGame(entry.game.id),
                    onContext: () => openContextMenu(entry),
                    onFocus: () => prefetchGame(entry.game.id),
                  })
              "
            >
              <BigPictureGameTile
                :game="entry.game"
                :status="entry.status"
                :hide-titles="hideTitles"
                :overlay-theme-id="tileOverlayTheme"
              />
            </div>
          </div>
        </section>

        <!-- Collections — the old Shelves filter tab, as a rail you steer into
             instead of a fourth chip that swapped the whole page out. -->
        <section class="px-8 pt-6">
          <div class="flex items-baseline gap-3 mb-3">
            <h2 class="text-lg font-semibold font-display" style="color: var(--bpm-text)">Collections</h2>
            <span class="text-xs tabular-nums" style="color: var(--bpm-muted)">{{ collectionRows.length }}</span>
          </div>
          <BigPictureSectionError
            v-if="shelvesError"
            :ref="(el: any) => registerTile(el, { onSelect: loadShelves })"
            class="max-w-md"
            title="Couldn't load your collections"
            @retry="loadShelves"
          />
          <div v-else class="bpm-rail flex gap-4 overflow-x-auto pb-2 px-1 pt-1">
            <button
              v-for="row in collectionRows"
              :key="row.id"
              :ref="(el: any) => registerTile(el, { onSelect: () => selectGroup('collection', row.id, row.name) })"
              class="relative flex-shrink-0 flex flex-col justify-end overflow-hidden rounded-xl text-left"
              style="width: 16rem; height: 9rem; background-color: var(--bpm-surface); border: 1px solid var(--bpm-border)"
              @click="selectGroup('collection', row.id, row.name)"
            >
              <div class="pointer-events-none absolute inset-0 flex items-center justify-center gap-2 opacity-40" style="transform: rotate(-18deg) scale(1.4)">
                <img
                  v-for="(cover, i) in row.covers"
                  :key="i"
                  :src="objectUrl(cover)"
                  alt=""
                  class="h-28 w-20 shrink-0 rounded object-cover"
                />
              </div>
              <div class="pointer-events-none absolute inset-0" style="background: linear-gradient(to top, var(--bpm-bg) 10%, transparent 90%)" />
              <div class="relative px-4 pb-3">
                <p class="text-sm font-semibold truncate" style="color: var(--bpm-text)">{{ row.name }}</p>
                <p class="text-xs" style="color: var(--bpm-muted)">
                  {{ row.entries.length }} game{{ row.entries.length !== 1 ? "s" : "" }}
                  <span v-if="row.isPublic"> · Public</span>
                </p>
              </div>
            </button>

            <button
              :ref="(el: any) => registerTile(el, { onSelect: () => (showNewShelfKeyboard = true) })"
              class="flex-shrink-0 flex flex-col items-center justify-center rounded-xl"
              style="width: 16rem; height: 9rem; background-color: var(--bpm-surface); border: 1px dashed var(--bpm-border)"
              @click="showNewShelfKeyboard = true"
            >
              <PlusIcon class="size-7" style="color: var(--bpm-accent-hex)" />
              <span class="mt-2 text-sm font-medium" style="color: var(--bpm-text)">New Collection</span>
            </button>
          </div>
        </section>

        <!-- Consoles — emulated games grouped by system (toggle in settings). -->
        <section v-if="consoleSections.enabled.value && (consoleRows.length > 0 || consolesError)" class="px-8 pt-6">
          <div class="flex items-baseline gap-3 mb-3">
            <h2 class="text-lg font-semibold font-display" style="color: var(--bpm-text)">Consoles</h2>
            <span class="text-xs tabular-nums" style="color: var(--bpm-muted)">{{ consoleRows.length }}</span>
          </div>
          <BigPictureSectionError
            v-if="consolesError"
            :ref="(el: any) => registerTile(el, { onSelect: loadConsoles })"
            class="max-w-md"
            title="Couldn't load your consoles"
            detail="The console groupings didn't load."
            @retry="loadConsoles"
          />
          <div v-else class="bpm-rail flex gap-4 overflow-x-auto pb-2 px-1 pt-1">
            <button
              v-for="row in consoleRows"
              :key="row.id"
              :ref="(el: any) => registerTile(el, { onSelect: () => selectGroup('console', row.id, row.label) })"
              class="relative flex-shrink-0 flex flex-col items-center justify-center overflow-hidden rounded-xl px-4 py-3"
              style="width: 16rem; height: 9rem; background-color: var(--bpm-surface); border: 1px solid var(--bpm-border)"
              @click="selectGroup('console', row.id, row.label)"
            >
              <img
                v-if="row.art"
                :src="row.art.render"
                :alt="row.shortName"
                class="mb-2 h-14 w-auto max-w-[70%] object-contain"
                :style="row.art.pixel ? 'image-rendering: pixelated' : ''"
              />
              <p class="text-sm font-semibold uppercase tracking-wide" style="color: var(--bpm-text)">
                {{ row.label }}
              </p>
              <p class="text-xs" style="color: var(--bpm-muted)">
                {{ row.entries.length }} game{{ row.entries.length !== 1 ? "s" : "" }}
              </p>
            </button>
          </div>
        </section>
      </template>

      <!-- ═══════════════════════════════════════════════════════════════════
           ALL GAMES — sticky header (filters, sort, search) over the grid.
           The header rides up to the top bar once you scroll past the rails,
           so the controls stay put while the grid moves.
           ═══════════════════════════════════════════════════════════════════ -->
      <div
        ref="allGamesHeader"
        class="sticky top-0 z-20 flex flex-wrap items-center gap-2 px-8 py-3 mt-6 border-y"
        :style="{ backgroundColor: 'var(--bpm-bg)', borderColor: 'var(--bpm-border)' }"
      >
        <h2 v-if="!activeGroup" class="text-lg font-semibold font-display mr-2" style="color: var(--bpm-text)">
          All Games
        </h2>
        <template v-else>
          <button
            :ref="(el: any) => registerFilter(el, { onSelect: clearGroup })"
            class="inline-flex items-center gap-2 px-3 py-2 text-sm rounded-lg font-medium"
            style="background-color: var(--bpm-accent-hex); color: var(--bpm-accent-text)"
            @click="clearGroup"
          >
            <XMarkIcon class="size-4" />
            {{ activeGroup.name }}
          </button>
          <template v-if="activeGroup.kind === 'collection'">
            <button
              :ref="(el: any) => registerFilter(el, { onSelect: toggleActiveShelfVisibility })"
              class="px-3 py-2 text-sm rounded-lg transition-colors hover:bg-white/5"
              :style="{ color: activeShelfIsPublic ? 'var(--bpm-accent-hex)' : 'var(--bpm-muted)' }"
              @click="toggleActiveShelfVisibility"
            >
              {{ activeShelfIsPublic ? "Public" : "Private" }}
            </button>
            <button
              :ref="(el: any) => registerFilter(el, { onSelect: deleteActiveShelf })"
              class="px-3 py-2 text-sm rounded-lg transition-colors hover:bg-white/5"
              style="color: var(--bpm-muted)"
              @click="deleteActiveShelf"
            >
              Delete Collection
            </button>
          </template>
        </template>

        <button
          v-for="filter in filters"
          :key="filter.value"
          :ref="(el: any) => registerFilter(el, { onSelect: () => (activeFilter = filter.value) })"
          class="px-4 py-2 text-sm rounded-lg font-medium transition-colors"
          :class="[
            activeFilter === filter.value
              ? 'bg-blue-600/20 text-blue-400'
              : 'text-zinc-400 hover:text-zinc-200 hover:bg-zinc-800/50',
          ]"
          @click="activeFilter = filter.value"
        >
          {{ filter.label }}
          <span v-if="filter.count > 0" class="ml-1 text-xs opacity-60">{{ filter.count }}</span>
        </button>

        <div class="flex-1" />

        <!-- Sort/filter summary. A plain <div> here left Sort reachable only
             via the X shortcut, which nothing on screen advertises. -->
        <button
          :ref="(el: any) => registerFilter(el, { onSelect: openSortMenu })"
          class="flex items-center gap-2 px-3 py-2 text-sm rounded-lg text-zinc-500 hover:text-zinc-300 hover:bg-zinc-800/50 transition-colors"
          @click="openSortMenu"
        >
          <ArrowsUpDownIcon class="size-4" />
          <span>{{ sortLabel }}</span>
        </button>

        <button
          :ref="(el: any) => registerFilter(el, { onSelect: () => (showKeyboard = true) })"
          class="flex items-center gap-2 px-4 py-2 text-sm rounded-lg font-medium text-zinc-400 hover:text-zinc-200 hover:bg-zinc-800/50 transition-colors"
          @click="showKeyboard = true"
        >
          <MagnifyingGlassIcon class="size-4" />
          <span v-if="searchQuery">{{ searchQuery }}</span>
          <span v-else class="text-zinc-600">Search...</span>
        </button>
      </div>

      <!-- Game grid -->
      <div class="flex-1 px-8 py-6">
        <p v-if="searching" class="text-xs uppercase tracking-widest mb-4" style="color: var(--bpm-muted)">
          {{ filteredGames.length }} result{{ filteredGames.length === 1 ? "" : "s" }} for "{{ searchQuery.trim() }}"
        </p>

        <div class="grid gap-4 grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6 2xl:grid-cols-7">
          <div
            v-for="(entry, index) in filteredGames"
            :key="entry.game.id"
            class="game-tile-wrapper"
            :class="{ 'tile-visible': tilesReady }"
            :style="{ transitionDelay: `${Math.min(index * 30, 500)}ms` }"
          >
            <div class="relative">
              <BigPictureGameTile
                :ref="
                  (el: any) =>
                    registerTile(el, {
                      onSelect: () => {
                        if (multiSelectMode) {
                          toggleSelect(entry.game.id);
                          return;
                        }
                        navigateToGame(entry.game.id);
                      },
                      onContext: () => openContextMenu(entry),
                      onFocus: () => prefetchGame(entry.game.id),
                    })
                "
                :game="entry.game"
                :status="entry.status"
                :hide-titles="hideTitles"
                :overlay-theme-id="tileOverlayTheme"
              />
              <!-- Multi-select checkbox -->
              <div
                v-if="multiSelectMode"
                class="absolute top-2 left-2 z-20 size-6 rounded-md flex items-center justify-center transition-colors"
                :class="selectedGames.has(entry.game.id) ? 'bg-blue-500' : 'bg-zinc-800/80 ring-1 ring-zinc-600'"
              >
                <svg v-if="selectedGames.has(entry.game.id)" class="size-4 text-white" fill="none" stroke="currentColor" stroke-width="3" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" d="M4.5 12.75l6 6 9-13.5" />
                </svg>
              </div>
            </div>
          </div>
        </div>

        <!-- A failed library fetch gets its own face; the empty state below is
             only for a library that really has nothing in it. -->
        <div v-if="libraryError" class="flex items-center justify-center py-24">
          <BigPictureSectionError
            :ref="(el: any) => registerTile(el, { onSelect: () => loadLibrary(true) })"
            class="max-w-md"
            title="Couldn't load your library"
            :detail="libraryError"
            @retry="loadLibrary(true)"
          />
        </div>

        <div v-else-if="filteredGames.length === 0" class="flex items-center justify-center py-24">
          <div class="text-center">
            <component
              :is="searchQuery ? MagnifyingGlassIcon : Square3Stack3DIcon"
              class="size-16 mx-auto mb-4 text-zinc-600"
            />
            <h3 class="text-2xl font-semibold text-zinc-400 mb-2">
              {{ emptyTitle }}
            </h3>
            <p class="text-zinc-600 mb-4">
              {{ emptyDetail }}
            </p>
            <div class="flex items-center justify-center gap-3">
              <button
                v-if="searchQuery"
                :ref="(el: any) => registerTile(el, { onSelect: () => (searchQuery = '') })"
                class="inline-flex items-center gap-2 px-4 py-2 bg-blue-600 hover:bg-blue-500 text-white rounded-lg transition-colors text-sm font-medium"
                @click="searchQuery = ''"
              >
                <XMarkIcon class="size-4" />
                Clear search
              </button>
              <button
                v-if="activeGroup"
                :ref="(el: any) => registerTile(el, { onSelect: clearGroup })"
                class="inline-flex items-center gap-2 px-4 py-2 bg-blue-600 hover:bg-blue-500 text-white rounded-lg transition-colors text-sm font-medium"
                @click="clearGroup"
              >
                <XMarkIcon class="size-4" />
                Show all games
              </button>
              <button
                v-if="activeFilter !== 'all'"
                :ref="(el: any) => registerTile(el, { onSelect: () => (activeFilter = 'all') })"
                class="inline-flex items-center gap-2 px-4 py-2 bg-blue-600 hover:bg-blue-500 text-white rounded-lg transition-colors text-sm font-medium"
                @click="activeFilter = 'all'"
              >
                <XMarkIcon class="size-4" />
                Show all
              </button>
            </div>
          </div>
        </div>
      </div>
    </template>

    <!-- On-screen keyboards -->
    <BigPictureKeyboard
      :visible="showKeyboard"
      :model-value="searchQuery"
      placeholder="Search your library..."
      @update:model-value="searchQuery = $event"
      @close="showKeyboard = false"
      @submit="showKeyboard = false"
    />
    <BigPictureKeyboard
      :visible="showNewShelfKeyboard"
      :model-value="newShelfName"
      placeholder="Name your collection..."
      @update:model-value="newShelfName = $event"
      @close="showNewShelfKeyboard = false"
      @submit="createNewShelf"
    />

    <!-- Sort & Filter overlay -->
    <Teleport to="body">
      <Transition
        enter-active-class="transition-opacity duration-200"
        leave-active-class="transition-opacity duration-200"
        enter-from-class="opacity-0"
        leave-to-class="opacity-0"
      >
        <div
          v-if="showFilterMenu"
          class="fixed inset-0 z-50 flex items-center justify-center bg-black/70 backdrop-blur-sm"
        >
          <div class="bg-zinc-900 border border-zinc-700/50 rounded-2xl shadow-2xl p-6 max-w-2xl w-full mx-4">
            <h2 class="text-xl font-semibold font-display text-zinc-100 mb-5">Sort & Filter</h2>

            <div class="grid grid-cols-2 gap-6">
              <!-- Sort section -->
              <div>
                <p class="text-xs font-medium text-zinc-500 uppercase tracking-wider mb-2">Sort By</p>
                <div class="space-y-1.5">
                  <button
                    v-for="(label, key) in sortLabels"
                    :key="key"
                    class="w-full flex items-center justify-between px-4 py-3 rounded-xl text-sm transition-colors"
                    :class="sortMode === key
                      ? 'bg-blue-600 text-white shadow-lg shadow-blue-600/20'
                      : 'bg-zinc-800/50 text-zinc-300 hover:bg-zinc-700'"
                    :ref="(el: any) => registerOverlay(el, { onSelect: () => { sortMode = key as SortMode; } })"
                    @click="sortMode = key as SortMode"
                  >
                    <span class="font-medium">{{ label }}</span>
                    <span v-if="sortMode === key" class="text-xs opacity-75">Active</span>
                  </button>
                </div>
              </div>

              <!-- Filter section -->
              <div>
                <p class="text-xs font-medium text-zinc-500 uppercase tracking-wider mb-2">Show</p>
                <div class="space-y-1.5">
                  <button
                    v-for="f in filters"
                    :key="f.value"
                    class="w-full flex items-center justify-between px-4 py-3 rounded-xl text-sm transition-colors"
                    :class="activeFilter === f.value
                      ? 'bg-blue-600 text-white shadow-lg shadow-blue-600/20'
                      : 'bg-zinc-800/50 text-zinc-300 hover:bg-zinc-700'"
                    :ref="(el: any) => registerOverlay(el, { onSelect: () => { activeFilter = f.value; } })"
                    @click="activeFilter = f.value"
                  >
                    <span class="font-medium">{{ f.label }}</span>
                    <span class="text-xs opacity-75">{{ f.count }}</span>
                  </button>
                </div>
              </div>
            </div>

            <!-- Close -->
            <button
              :ref="(el: any) => registerOverlay(el, { onSelect: closeSortMenu })"
              class="w-full mt-5 px-4 py-3 rounded-xl text-sm font-medium bg-zinc-800/50 text-zinc-300 hover:bg-zinc-700 transition-colors"
              @click="closeSortMenu"
            >
              Done
            </button>
          </div>
        </div>
      </Transition>
    </Teleport>

    <!-- ═══ Context menu overlay ═══ -->
    <Teleport to="body">
      <Transition
        enter-active-class="transition-opacity duration-150"
        leave-active-class="transition-opacity duration-100"
        enter-from-class="opacity-0"
        leave-to-class="opacity-0"
      >
        <div v-if="contextMenuGame" class="fixed inset-0 z-[200] flex items-center justify-center">
          <div class="absolute inset-0 bg-black/60 backdrop-blur-sm" @click="closeContextMenu" />
          <div
            class="relative z-10 rounded-2xl p-6 w-80 max-w-[90vw]"
            style="background-color: var(--bpm-surface); color: var(--bpm-text)"
          >
            <!-- Game info header -->
            <div class="flex items-center gap-4 mb-5 pb-4 border-b" style="border-color: var(--bpm-border)">
              <img
                v-if="contextMenuGame.game.mCoverObjectId"
                :src="objectUrl(contextMenuGame.game.mCoverObjectId)"
                class="size-14 rounded-lg object-cover"
              />
              <div class="flex-1 min-w-0">
                <p class="font-semibold text-sm truncate">{{ contextMenuGame.game.mName }}</p>
                <p class="text-xs mt-0.5" style="color: var(--bpm-muted)">{{ contextMenuGame.status.type }}</p>
              </div>
            </div>

            <!-- Actions -->
            <div class="space-y-1">
              <button
                v-for="item in contextMenuActions"
                :key="item.id"
                :ref="(el: any) => registerCtxMenu(el, { onSelect: item.action })"
                class="w-full flex items-center gap-3 px-4 py-3 text-sm rounded-xl transition-colors hover:bg-white/5"
                @click="item.action"
              >
                <component :is="item.icon" class="size-5 flex-shrink-0" :style="{ color: item.color }" />
                <span>{{ item.label }}</span>
              </button>
            </div>

            <!-- Multi-select actions (when in select mode) -->
            <div v-if="multiSelectMode && selectedGames.size > 0" class="mt-4 pt-4 border-t" style="border-color: var(--bpm-border)">
              <p class="text-xs mb-3" style="color: var(--bpm-muted)">{{ selectedGames.size }} game{{ selectedGames.size !== 1 ? 's' : '' }} selected</p>
              <div class="flex gap-2">
                <button
                  :ref="(el: any) => registerCtxMenu(el, { onSelect: bulkUninstall })"
                  class="flex-1 px-3 py-2 text-xs rounded-lg bg-red-900/20 text-red-400 hover:bg-red-900/30 transition-colors"
                  @click="bulkUninstall"
                >
                  Uninstall Selected
                </button>
                <button
                  :ref="(el: any) => registerCtxMenu(el, { onSelect: clearSelection })"
                  class="flex-1 px-3 py-2 text-xs rounded-lg transition-colors hover:bg-white/5"
                  style="color: var(--bpm-muted)"
                  @click="clearSelection"
                >
                  Clear Selection
                </button>
              </div>
            </div>
          </div>
        </div>
      </Transition>
    </Teleport>

    <!-- Multi-select toolbar -->
    <div
      v-if="multiSelectMode"
      class="fixed bottom-0 left-0 right-0 z-[100] flex items-center justify-between px-8 py-4"
      style="background: linear-gradient(to top, var(--bpm-bg), transparent)"
    >
      <div class="flex items-center gap-3">
        <span class="text-sm font-medium" style="color: var(--bpm-text)">
          {{ selectedGames.size }} selected
        </span>
        <button
          :ref="(el: any) => registerTile(el, { onSelect: selectAll })"
          class="px-3 py-1.5 text-xs rounded-lg transition-colors hover:bg-white/10"
          style="color: var(--bpm-muted)"
          @click="selectAll"
        >
          Select All
        </button>
      </div>
      <div class="flex items-center gap-2">
        <button
          v-if="selectedGames.size > 0"
          :ref="(el: any) => registerTile(el, { onSelect: bulkUninstall })"
          class="px-4 py-2 text-sm rounded-lg bg-red-600 text-white hover:bg-red-500 transition-colors"
          @click="bulkUninstall"
        >
          Uninstall ({{ selectedGames.size }})
        </button>
        <button
          :ref="(el: any) => registerTile(el, { onSelect: exitMultiSelect })"
          class="px-4 py-2 text-sm rounded-lg transition-colors hover:bg-white/10"
          style="color: var(--bpm-muted)"
          @click="exitMultiSelect"
        >
          Cancel
        </button>
      </div>
    </div>

    <!--
      Play/Stop from the grid's context menu used to report failures to the
      console only, so a launch that could not start looked identical to one
      that was ignored.
    -->
    <BigPictureDialog
      :visible="launchError !== null"
      :title="launchErrorTitle"
      :message="launchError ?? ''"
      confirm-label="Dismiss"
      :show-cancel="false"
      @confirm="launchError = null"
    />
  </div>
</template>

<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import {
  MagnifyingGlassIcon,
  XMarkIcon,
  Square3Stack3DIcon,
  ArrowsUpDownIcon,
  FolderIcon,
  PlusIcon,
} from "@heroicons/vue/24/outline";
import {
  PlayIcon,
  ArrowDownTrayIcon,
  TrashIcon,
  CheckCircleIcon,
  StopIcon,
} from "@heroicons/vue/24/solid";
import BigPictureGameTile from "~/components/bigpicture/BigPictureGameTile.vue";
import BigPictureKeyboard from "~/components/bigpicture/BigPictureKeyboard.vue";
import BigPictureSectionError from "~/components/bigpicture/BigPictureSectionError.vue";
import BigPictureDialog from "~/components/bigpicture/BigPictureDialog.vue";
import BpmClock from "~/components/bigpicture/BpmClock.vue";
import BpmScreensaver from "~/components/bigpicture/BpmScreensaver.vue";
import BpmCrtFilter from "~/components/bigpicture/BpmCrtFilter.vue";
import BpmAnimatedBackground from "~/components/bigpicture/BpmAnimatedBackground.vue";
import BpmAchievementToast from "~/components/bigpicture/BpmAchievementToast.vue";
import BpmLaunchScreen from "~/components/bigpicture/BpmLaunchScreen.vue";
import BpmBoxArtOverlay from "~/components/bigpicture/BpmBoxArtOverlay.vue";
import BpmHomeSkeleton from "~/components/bigpicture/BpmHomeSkeleton.vue";
import { devLog } from "~/composables/dev-mode";
import { useListen } from "~/composables/useListen";
import { parseStatus, deduplicatedInvoke } from "~/composables/game";
import type { LaunchResult } from "~/composables/game";
import {
  describeLaunchFailure,
  isBenignLaunchError,
} from "~/composables/launch-failure";
import { useGamepad, GamepadButton } from "~/composables/gamepad";
import { useBpFocusableGroup } from "~/composables/bp-focusable";
import { useFocusNavigation } from "~/composables/focus-navigation";
import { useDeckMode } from "~/composables/deck-mode";
import { useQueueState } from "~/composables/downloads";
import { objectImageUrl } from "~/composables/use-object";
import { useShelves } from "~/composables/shelves";
import { useConsoleSections } from "~/composables/console-sections";
import { useServerApi, type ConsoleGroup } from "~/composables/use-server-api";
import { consoleArt } from "~/composables/console-art";
import { useBpmTheme } from "~/composables/bp-theme";
import { useBpmIdle } from "~/composables/bp-idle";
import { useBpmAmbient } from "~/composables/bp-ambient";
import { useBpmWelcome } from "~/composables/bp-welcome";
import { useBpmCursors } from "~/composables/bp-cursors";
import {
  useBpmHomeData,
  formatHomePlaytime,
} from "~/composables/bigpicture/use-bpm-home-data";
import type { Game, GameStatus, Collection, RawGameStatus } from "~/types";

definePageMeta({ layout: "bigpicture" });

/**
 * BPM home = BPM library.
 *
 * The two pages rendered the same data twice: home's "recent games" was the
 * playtime feed truncated to 20, which is a strict subset of the library, and
 * both drew their own copy of the download queue that /bigpicture/downloads
 * already owns. Ten near-identical per-console theme blocks each re-rendered a
 * spotlight, a grid and a downloads list, so a change to any of the three had
 * to be made ten times.
 *
 * The merge keeps exactly one skinned zone — the spotlight hero — and runs the
 * rails and grid off the --bpm-* tokens, so all ten skins survive on the hero
 * and the library finally picks up the active theme instead of staying the one
 * un-themed BPM page. `/bigpicture/library` redirects here.
 */

interface LibraryEntry {
  game: Game;
  status: GameStatus;
}

interface FetchLibraryResponse {
  library: Game[];
  collections: Collection[];
  other: Game[];
  missing: Game[];
}

const route = useRoute();
const router = useRouter();
const focusNav = useFocusNavigation();
const registerTile = useBpFocusableGroup("content");
const registerFilter = useBpFocusableGroup("content");
const registerOverlay = useBpFocusableGroup("sort-overlay");
const registerCtxMenu = useBpFocusableGroup("context-menu");
const api = useServerApi();
const consoleSections = useConsoleSections();

function objectUrl(id: string): string {
  return objectImageUrl(id);
}

function prefetchGame(gameId: string) {
  deduplicatedInvoke("fetch_game", { gameId }).catch(() => {});
}

function navigateToGame(gameId: string) {
  focusNav.saveFocusSnapshot(route.path);
  const target = `/bigpicture/library/${gameId}`;
  focusNav.setRouteState("backTo", route.path, target);
  router.push(target).catch((e: any) => {
    console.error(`[BPM:HOME] Navigation FAILED for ${gameId}:`, e);
  });
}

// ── Library ─────────────────────────────────────────────────────────────
const library: Ref<LibraryEntry[]> = ref([]);
const libraryById = computed(
  () => new Map(library.value.map((e) => [e.game.id, e])),
);
const loading = ref(true);
const libraryError = ref<string | null>(null);
const tilesReady = ref(false);

const hideTitles = ref(
  typeof localStorage !== "undefined"
    ? localStorage.getItem("drop:hideTitles") === "true"
    : false,
);

/**
 * `silent` is for the background revalidation half of the two-phase load: it
 * leaves the already-painted grid and its error state alone, so a slow or
 * failing hard refresh can't blank a library that already rendered from cache.
 */
async function loadLibrary(hardRefresh = false, silent = false) {
  try {
    const data = await invoke<FetchLibraryResponse>("fetch_library", {
      hardRefresh,
    });

    const seen = new Set<string>();
    const uniqueGames: Game[] = [];
    const allRawGames: Game[] = [
      ...data.library,
      ...data.collections.flatMap((c) => c.entries.map((e) => e.game)),
      ...data.other,
      ...data.missing,
    ];
    for (const game of allRawGames) {
      if (!seen.has(game.id)) {
        seen.add(game.id);
        uniqueGames.push(game);
      }
    }

    // One IPC call for every status. This was N serial `fetch_game_status`
    // round trips, each of which took a DB write guard.
    const statusMap = new Map<string, GameStatus>();
    try {
      const batchResults: [string, RawGameStatus][] = await invoke(
        "fetch_game_statuses",
        { ids: uniqueGames.map((g) => g.id) },
      );
      for (const [id, raw] of batchResults) {
        try {
          statusMap.set(id, parseStatus(raw));
        } catch {
          // No status row yet — the tile falls back to Remote below.
        }
      }
    } catch (e) {
      console.error("[BPM:HOME] Batch status fetch failed:", e);
    }

    library.value = uniqueGames.map((game) => ({
      game,
      status: statusMap.get(game.id) ?? ({ type: "Remote" } as GameStatus),
    }));
    libraryError.value = null;
  } catch (e) {
    console.error("[BPM:HOME] Failed to fetch library:", e);
    // Without this the grid fell through to "No games found · Add games to
    // your library to get started", which is the same screen a new user sees.
    if (!silent) {
      libraryError.value = e instanceof Error ? e.message : String(e);
    }
  } finally {
    loading.value = false;
    nextTick(() => {
      tilesReady.value = true;
    });
  }
}

// ── Recently played + spotlight ─────────────────────────────────────────
const homeData = useBpmHomeData();
const { recentGames, otherGames, spotlightGame } = homeData;
// The hero comes from the playtime feed and the grid from the library, so the
// page is only ready when both have landed — gating on one alone paints a
// half-built home screen and then reflows it.
const pageLoading = computed(() => loading.value || homeData.loading.value);
const showSkeleton = useDeferredLoading(() => pageLoading.value);

const formatPlaytime = formatHomePlaytime;

/**
 * The hero, resolved against the library so it gets the full Game record
 * (banner art, icon) that the playtime feed's partial payload doesn't carry.
 * Falls back to the partial when the library hasn't landed or the game is
 * no longer in it.
 */
const heroEntry = computed(() => {
  const pick = spotlightGame.value;
  if (!pick) return null;
  const full = libraryById.value.get(pick.game.id);
  return {
    game: full?.game ?? pick.game,
    status: full?.status ?? pick.status,
    playtimeSeconds: pick.playtimeSeconds,
  };
});

const heroGameId = computed(() => heroEntry.value?.game.id ?? "");
const heroName = computed(() => heroEntry.value?.game.mName ?? "");
const heroInitial = computed(() => heroName.value.charAt(0).toUpperCase());
const heroCoverId = computed(() => heroEntry.value?.game.mCoverObjectId ?? "");
// Wide skins (Steam, Xbox) fill a banner-shaped box, so they prefer the
// banner and only fall back to the cover.
const heroArtId = computed(
  () => heroEntry.value?.game.mBannerObjectId || heroCoverId.value,
);
const heroPlaytimeLabel = computed(() => {
  const seconds = heroEntry.value?.playtimeSeconds ?? 0;
  return seconds > 0 ? `${formatPlaytime(seconds)} played` : "";
});
const heroActionLabel = computed(() => {
  const type = heroEntry.value?.status.type;
  if (type === "Installed") return "Play";
  if (type === "Remote") return "Install";
  return "Details";
});
// A second button only earns its place when the primary isn't already it.
const showHeroDetails = computed(() => heroActionLabel.value !== "Details");

function openHeroGame() {
  if (heroGameId.value) navigateToGame(heroGameId.value);
}

/**
 * The hero's primary button does the thing it is named after: Play launches,
 * Install starts the download. Every skin's hero button used to navigate to
 * the game page no matter what it said.
 */
async function heroAction() {
  const entry = heroEntry.value;
  if (!entry) return;
  if (entry.status.type === "Installed") {
    await launchEntry(entry.game.id, entry.game.mName);
    return;
  }
  if (entry.status.type === "Remote") {
    try {
      await invoke("download_game", { id: entry.game.id });
    } catch (e) {
      console.error("[BPM:HOME] Download failed:", e);
      showLaunchError(
        "Couldn't start the download",
        `Drop couldn't queue ${entry.game.mName}: ${e instanceof Error ? e.message : String(e)}`,
      );
    }
    return;
  }
  openHeroGame();
}

// Stable ref callbacks — the hero's two buttons repeat across ten skins.
const registerHeroAction = (el: any) =>
  registerTile(el, { onSelect: heroAction });
const registerHeroDetails = (el: any) =>
  registerTile(el, {
    onSelect: openHeroGame,
    onFocus: () => heroGameId.value && prefetchGame(heroGameId.value),
  });

/**
 * Continue Playing rail: the recently-played list minus whatever the hero is
 * showing, joined back to the library so the tiles carry full game records.
 */
const continueEntries = computed<LibraryEntry[]>(() =>
  otherGames.value.map(
    (r) =>
      libraryById.value.get(r.game.id) ?? { game: r.game, status: r.status },
  ),
);

// One focus index for the whole page, replacing the ten per-theme ones. Only
// the PS2 skin reads it — its header echoes whatever the rail has focused,
// which is the behaviour its memory-card browser was built around.
const focusedIdx = ref(0);
const focusedRecent = computed(
  () => otherGames.value[focusedIdx.value] ?? spotlightGame.value,
);

const updatesEntries = computed<LibraryEntry[]>(() =>
  library.value.filter(
    (e) => e.status.type === "Installed" && e.status.update_available,
  ),
);

// ── Collections (shelves) ───────────────────────────────────────────────
const shelvesData = useShelves();
const shelvesError = ref(false);
const showNewShelfKeyboard = ref(false);
const newShelfName = ref("");

function loadShelves() {
  shelvesError.value = false;
  return shelvesData.fetchShelves().catch((e: unknown) => {
    console.warn("[BPM:HOME] shelves fetch failed:", e);
    shelvesError.value = true;
  });
}

async function createNewShelf() {
  const name = newShelfName.value.trim();
  showNewShelfKeyboard.value = false;
  if (!name) return;
  await shelvesData.createShelf(name);
  newShelfName.value = "";
}

// Empty collections stay on the rail on purpose. The desktop library hides
// them because its filter drawer still lists them, but this rail is the only
// place they exist in BPM — dropping them meant a collection you just created
// disappeared the moment you named it.
const collectionRows = computed(() =>
  shelvesData.shelves.value.map((shelf) => ({
    id: shelf.id,
    name: shelf.name,
    isPublic: shelf.isPublic === true,
    entries: shelf.entries
      .map((e) => libraryById.value.get(e.gameId))
      .filter((e): e is LibraryEntry => !!e),
    covers: shelf.entries
      .map((e) => e.game.mCoverObjectId)
      .filter((c): c is string => !!c)
      .slice(0, 4),
  })),
);

// ── Consoles (emulation view) ───────────────────────────────────────────
const consoleGroups = ref<ConsoleGroup[]>([]);
const consolesError = ref(false);

async function loadConsoles() {
  consolesError.value = false;
  if (!consoleSections.enabled.value) {
    consoleGroups.value = [];
    return;
  }
  try {
    consoleGroups.value = (await api.emulation.consoles()).consoles;
  } catch (e) {
    console.warn("[BPM:HOME] console grouping fetch failed:", e);
    consolesError.value = true;
    consoleGroups.value = [];
  }
}

watch(() => consoleSections.enabled.value, () => loadConsoles());

const consoleRows = computed(() => {
  if (!consoleSections.enabled.value) return [];
  return consoleGroups.value
    .map((group) => {
      const art = consoleArt(group.shortName);
      return {
        ...group,
        art,
        // `consoleArt` also corrects operator typos in the console name, so
        // prefer its canonical label over the raw group name.
        label: art?.name ?? group.shortName,
        entries: group.gameIds
          .map((id) => libraryById.value.get(id))
          .filter((e): e is LibraryEntry => !!e),
      };
    })
    .filter((g) => g.entries.length > 0);
});

// Emulated games live under their console row, not in the main grid — the
// same split the desktop library makes when the toggle is on.
const emulatedGameIds = computed<Set<string>>(() => {
  const set = new Set<string>();
  for (const row of consoleRows.value) {
    for (const e of row.entries) set.add(e.game.id);
  }
  return set;
});

// ── Grid filtering ──────────────────────────────────────────────────────
type GroupFilter = {
  kind: "collection" | "console";
  id: string;
  name: string;
} | null;

const activeGroup = ref<GroupFilter>(null);
const activeFilter = ref("all");
const searchQuery = ref("");
const showKeyboard = ref(false);
const showFilterMenu = ref(false);

const searching = computed(() => searchQuery.value.trim().length > 0);

const allGamesHeader = ref<HTMLElement | null>(null);

/**
 * Picking a collection or console filters the grid, which lives below the
 * rails — so bring it up. Without the scroll the card just highlights and the
 * result is off-screen, which reads as nothing having happened.
 */
function selectGroup(kind: "collection" | "console", id: string, name: string) {
  activeGroup.value = { kind, id, name };
  nextTick(() => {
    allGamesHeader.value?.scrollIntoView({
      behavior: reducedMotion.value ? "auto" : "smooth",
      block: "start",
    });
  });
}

function clearGroup() {
  activeGroup.value = null;
}

const activeShelf = computed(() =>
  activeGroup.value?.kind === "collection"
    ? shelvesData.shelves.value.find((s) => s.id === activeGroup.value?.id)
    : undefined,
);
const activeShelfIsPublic = computed(() => activeShelf.value?.isPublic === true);

async function toggleActiveShelfVisibility() {
  const shelf = activeShelf.value;
  if (!shelf) return;
  await shelvesData.toggleShelfVisibility(shelf.id, !activeShelfIsPublic.value);
}

async function deleteActiveShelf() {
  const shelf = activeShelf.value;
  if (!shelf) return;
  await shelvesData.deleteShelf(shelf.id);
  clearGroup();
}

const installedCount = computed(
  () => library.value.filter((e) => e.status.type === "Installed").length,
);

const filters = computed(() => [
  { label: "All", value: "all", count: library.value.length },
  { label: "Installed", value: "installed", count: installedCount.value },
  {
    label: "Not Installed",
    value: "remote",
    count: library.value.length - installedCount.value,
  },
]);

type SortMode = "name" | "recent" | "status" | "size";
const sortMode = ref<SortMode>("name");
const sortLabels: Record<SortMode, string> = {
  name: "Name",
  recent: "Recent",
  status: "Status",
  size: "Size",
};
const sortLabel = computed(() => sortLabels[sortMode.value]);

// ── Size cache for "size" sort ──────────────────────────────────────────
const gameSizes = ref<Record<string, number>>({});
const sizesLoading = ref(false);

async function loadGameSizes() {
  if (sizesLoading.value) return;
  sizesLoading.value = true;
  try {
    const installed = library.value.filter(
      (e) => e.status.type === "Installed",
    );
    for (const entry of installed) {
      if (gameSizes.value[entry.game.id] != null) continue;
      try {
        gameSizes.value[entry.game.id] = await invoke<number>(
          "get_install_size",
          { gameId: entry.game.id },
        );
      } catch {
        gameSizes.value[entry.game.id] = 0;
      }
    }
  } finally {
    sizesLoading.value = false;
  }
}

watch(sortMode, (mode) => {
  if (mode === "size") loadGameSizes();
});

const filteredGames = computed(() => {
  let games = [...library.value];

  // A search looks at everything: neither a console row nor a selected
  // collection should be able to hide a result the user just typed.
  if (!searching.value) {
    const group = activeGroup.value;
    if (group?.kind === "collection") {
      const ids = new Set(
        (collectionRows.value.find((r) => r.id === group.id)?.entries ?? []).map(
          (e) => e.game.id,
        ),
      );
      games = games.filter((e) => ids.has(e.game.id));
    } else if (group?.kind === "console") {
      const ids = new Set(
        (consoleRows.value.find((r) => r.id === group.id)?.entries ?? []).map(
          (e) => e.game.id,
        ),
      );
      games = games.filter((e) => ids.has(e.game.id));
    } else if (emulatedGameIds.value.size > 0) {
      games = games.filter((e) => !emulatedGameIds.value.has(e.game.id));
    }
  }

  if (activeFilter.value === "installed") {
    games = games.filter((e) => e.status.type === "Installed");
  } else if (activeFilter.value === "remote") {
    games = games.filter((e) => e.status.type !== "Installed");
  }

  if (searching.value) {
    const q = searchQuery.value.trim().toLowerCase();
    games = games.filter((e) => e.game.mName.toLowerCase().includes(q));
  }

  switch (sortMode.value) {
    case "name":
      games.sort((a, b) => a.game.mName.localeCompare(b.game.mName));
      break;
    case "status": {
      // Installed first, then running, then remote
      const statusOrder: Record<string, number> = {
        Running: 0,
        Downloading: 1,
        Installed: 2,
        Remote: 3,
        Queued: 4,
      };
      games.sort(
        (a, b) =>
          (statusOrder[a.status.type] ?? 99) - (statusOrder[b.status.type] ?? 99),
      );
      break;
    }
    case "recent":
      // Keep server order (most recently added first)
      games.reverse();
      break;
    case "size":
      // Largest first; uninstalled games (size 0) go to the end
      games.sort(
        (a, b) =>
          (gameSizes.value[b.game.id] ?? 0) - (gameSizes.value[a.game.id] ?? 0),
      );
      break;
  }

  return games;
});

const emptyTitle = computed(() => {
  if (searching.value) return `No games match "${searchQuery.value.trim()}"`;
  if (activeGroup.value) return `Nothing in ${activeGroup.value.name}`;
  if (activeFilter.value !== "all") {
    return `No ${activeFilter.value === "installed" ? "installed" : "uninstalled"} games`;
  }
  return "No games found";
});

const emptyDetail = computed(() => {
  if (searching.value) return "Try a different search term";
  // Names the control by the label it actually carries on the game page.
  if (activeGroup.value) return "Open a game and use Add to Shelf to put it here";
  if (activeFilter.value !== "all") return "Try changing your filter";
  return "Add games to your library to get started";
});

const gameCountLabel = computed(
  () => `${library.value.length} game${library.value.length !== 1 ? "s" : ""}`,
);

// ── Launch / stop failures ──────────────────────────────────────────────
const launchError = ref<string | null>(null);
const launchErrorTitle = ref("Launch Failed");

function showLaunchError(title: string, message: string) {
  launchErrorTitle.value = title;
  launchError.value = message;
}

async function launchEntry(gameId: string, gameName: string) {
  try {
    // `index` is not optional on the backend — omitting it made every Play
    // here fail argument deserialisation, which reached the user as nothing
    // happening at all.
    const result = await invoke<LaunchResult>("launch_game", {
      id: gameId,
      index: 0,
    });
    if (result.result === "InstallRequired") {
      const depName = result.data.name ?? "another program";
      showLaunchError(
        "Emulator not installed",
        `${gameName} runs through ${depName}, which isn't installed. Open the game's page to install it.`,
      );
    }
  } catch (e) {
    console.error("[BPM:HOME] Launch failed:", e);
    if (isBenignLaunchError(e)) return;
    const failure = describeLaunchFailure(e, gameName);
    showLaunchError(failure.title, failure.message);
  }
}

// ── Context menu ────────────────────────────────────────────────────────
const contextMenuGame = ref<LibraryEntry | null>(null);

function openContextMenu(entry: LibraryEntry) {
  contextMenuGame.value = entry;
  nextTick(() => {
    focusNav.restrictFocus("context-menu");
  });
}

function closeContextMenu() {
  contextMenuGame.value = null;
  focusNav.unrestrictFocus("content");
}

const contextMenuActions = computed(() => {
  const entry = contextMenuGame.value;
  if (!entry) return [];

  const actions: {
    id: string;
    label: string;
    icon: any;
    color: string;
    action: () => void;
  }[] = [];

  if (entry.status.type === "Installed") {
    actions.push({
      id: "play",
      label: "Play",
      icon: PlayIcon,
      color: "#3b82f6",
      action: async () => {
        closeContextMenu();
        await launchEntry(entry.game.id, entry.game.mName);
      },
    });
  } else if (entry.status.type === "Running") {
    actions.push({
      id: "stop",
      label: "Stop",
      icon: StopIcon,
      color: "#ef4444",
      action: async () => {
        closeContextMenu();
        try {
          // The backend parameter is `game_id`; `id` matched nothing, so Stop
          // silently did nothing here.
          await invoke("kill_game", { gameId: entry.game.id });
        } catch (e) {
          console.error("[BPM:HOME] Kill failed:", e);
          showLaunchError(
            "Couldn't stop the game",
            `Drop couldn't stop ${entry.game.mName}: ${e instanceof Error ? e.message : String(e)}`,
          );
        }
      },
    });
  } else if (entry.status.type === "Remote") {
    actions.push({
      id: "install",
      label: "Install",
      icon: ArrowDownTrayIcon,
      color: "#22c55e",
      action: async () => {
        closeContextMenu();
        try {
          await invoke("download_game", { id: entry.game.id });
        } catch (e) {
          console.error("[BPM:HOME] Download failed:", e);
        }
      },
    });
  }

  actions.push({
    id: "details",
    label: "View Details",
    icon: Square3Stack3DIcon,
    color: "var(--bpm-muted)",
    action: () => {
      closeContextMenu();
      navigateToGame(entry.game.id);
    },
  });

  // Adding to a shelf lives on the game's own page (it needs the shelf list);
  // taking a game back off one only makes sense while a collection is the
  // active filter, which is exactly when we know which shelf is meant.
  const shelf = activeShelf.value;
  if (shelf) {
    actions.push({
      id: "unshelve",
      label: `Remove from ${shelf.name}`,
      icon: FolderIcon,
      color: "var(--bpm-muted)",
      action: async () => {
        closeContextMenu();
        await shelvesData.removeFromShelf(shelf.id, entry.game.id);
      },
    });
  }

  actions.push({
    id: "select",
    label: multiSelectMode.value ? "Toggle Selection" : "Select Multiple",
    icon: CheckCircleIcon,
    color: "#3b82f6",
    action: () => {
      if (!multiSelectMode.value) {
        multiSelectMode.value = true;
        selectedGames.value = new Set([entry.game.id]);
      } else {
        toggleSelect(entry.game.id);
      }
      closeContextMenu();
    },
  });

  if (entry.status.type === "Installed") {
    actions.push({
      id: "uninstall",
      label: "Uninstall",
      icon: TrashIcon,
      color: "#ef4444",
      action: async () => {
        closeContextMenu();
        try {
          await invoke("uninstall_game", { id: entry.game.id });
          loadLibrary(true);
        } catch (e) {
          console.error("[BPM:HOME] Uninstall failed:", e);
        }
      },
    });
  }

  return actions;
});

// ── Multi-select ────────────────────────────────────────────────────────
const multiSelectMode = ref(false);
const selectedGames = ref<Set<string>>(new Set());

function toggleSelect(gameId: string) {
  const set = new Set(selectedGames.value);
  if (set.has(gameId)) set.delete(gameId);
  else set.add(gameId);
  selectedGames.value = set;
  if (set.size === 0) multiSelectMode.value = false;
}

function selectAll() {
  selectedGames.value = new Set(filteredGames.value.map((e) => e.game.id));
}

function clearSelection() {
  selectedGames.value = new Set();
  multiSelectMode.value = false;
  closeContextMenu();
}

function exitMultiSelect() {
  selectedGames.value = new Set();
  multiSelectMode.value = false;
}

async function bulkUninstall() {
  closeContextMenu();
  const ids = [...selectedGames.value];
  const installed = ids.filter((id) =>
    library.value.find((e) => e.game.id === id && e.status.type === "Installed"),
  );
  for (const id of installed) {
    try {
      await invoke("uninstall_game", { id });
    } catch (e) {
      console.error(`[BPM:HOME] Bulk uninstall failed for ${id}:`, e);
    }
  }
  selectedGames.value = new Set();
  multiSelectMode.value = false;
  loadLibrary(true);
}

// ── Sort overlay + gamepad shortcuts ────────────────────────────────────
const gamepad = useGamepad();
const _unsubs: (() => void)[] = [];

// Swap search/sort buttons on Gamescope (Deck reports Y↔X swapped)
const { isGamescope: _isGS, isSteamDeckHardware } = useDeckMode();
const _searchBtn = _isGS.value ? GamepadButton.West : GamepadButton.North;
const _sortBtn = _isGS.value ? GamepadButton.North : GamepadButton.West;

function openSortMenu() {
  showFilterMenu.value = true;
  focusNav.restrictFocus("sort-overlay");
}

function closeSortMenu() {
  showFilterMenu.value = false;
  focusNav.unrestrictFocus("content");
}

_unsubs.push(
  gamepad.onButton(_searchBtn, () => {
    if (showNewShelfKeyboard.value) return;
    showKeyboard.value = !showKeyboard.value;
  }),
);
_unsubs.push(
  gamepad.onButton(_sortBtn, () => {
    if (showKeyboard.value || showNewShelfKeyboard.value) return;
    if (contextMenuGame.value) return; // context menu handles its own close
    if (focusNav.contextHandled.value) return; // tile onContext handled it
    if (showFilterMenu.value) closeSortMenu();
    else openSortMenu();
  }),
);
_unsubs.push(
  gamepad.onButton(GamepadButton.East, () => {
    if (showFilterMenu.value) {
      closeSortMenu();
      return;
    }
    if (contextMenuGame.value) closeContextMenu();
  }),
);

// ── Theme + BPM chrome ──────────────────────────────────────────────────
const themeComposable = useBpmTheme();
const theme = ref<string>("steam");

// Skins whose home screen has always carried an animated backdrop. It now
// lives inside the hero band rather than behind the whole page: the hero is
// the only skinned zone, and a smaller animated area is cheaper on the Deck.
const ANIMATED_BG_THEMES = new Set([
  "wii",
  "ps2",
  "ds",
  "dreamcast",
  "gamecube",
  "psp",
  "gameboy",
  "snes",
]);
const themeHasAnimatedBg = computed(() => ANIMATED_BG_THEMES.has(theme.value));

// Mode-aware gradient backdrops for the skins that use one.
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
});

const todayLabel = computed(() =>
  new Date().toLocaleDateString("en-US", {
    month: "short",
    day: "numeric",
    year: "numeric",
  }),
);

const appState = useAppState();
const userProfilePicUrl = computed(() => {
  const picId = appState.value?.user?.profilePictureObjectId;
  return picId ? objectImageUrl(picId) : null;
});

// ── Cover overlay toggle ────────────────────────────────────────────────
// Reads the `bpm:boxArtOverlay` key the BPM settings page writes. The
// CustomEvent listener picks up live changes so toggling the setting updates
// the tiles immediately, without a route bounce.
// Default the platform-themed cover overlays OFF on Deck hardware: each adds a
// `mix-blend-mode` layer + a second image over every tile, which is real
// fill-rate cost across a grid on the iGPU. Honour an explicit user choice;
// otherwise off on Deck, on elsewhere. (Mirrors the animated-bg default below.)
const showBoxArtOverlays = ref(
  (() => {
    if (typeof localStorage === "undefined") return true;
    const stored = localStorage.getItem("bpm:boxArtOverlay");
    if (stored === "true") return true;
    if (stored === "false") return false;
    return !isSteamDeckHardware.value;
  })(),
);
function onBoxArtOverlayChange(e: Event) {
  showBoxArtOverlays.value = (e as CustomEvent<boolean>).detail !== false;
}
const tileOverlayTheme = computed(() =>
  showBoxArtOverlays.value ? theme.value : undefined,
);

const crtEnabled = ref(
  typeof localStorage !== "undefined"
    ? localStorage.getItem("bpm:crtFilter") === "true"
    : false,
);
// `reducedMotion` is user-overridable (BPM settings), defaults to
// `isSteamDeckHardware` if the user has never toggled it.
const { reducedMotion } = useReducedMotion();
const reducedAnimations = computed(() => reducedMotion.value);
// Default the animated background off on Deck — even the reduced variant's
// animation loops are enough to chip fps off the iGPU during scroll.
// Users can opt back in from the BPM settings page.
function readAnimBg(): boolean {
  if (typeof localStorage === "undefined") return true;
  const stored = localStorage.getItem("bpm:animBg");
  if (stored === "true") return true;
  if (stored === "false") return false;
  return !isSteamDeckHardware.value;
}
const animBgEnabled = ref(readAnimBg());
const currentAchievement = ref<{
  title: string;
  game: string;
  icon?: string;
} | null>(null);
const launchingGame = ref<{ name: string; coverUrl?: string } | null>(null);
const screensaverEnabled = ref(
  typeof localStorage !== "undefined"
    ? localStorage.getItem("bpm:screensaver") !== "false"
    : true,
);
const { isIdle } = useBpmIdle(120000);
useBpmAmbient();
useBpmWelcome(recentGames as any);
useBpmCursors();

// ── Lifecycle ───────────────────────────────────────────────────────────
const queueState = useQueueState();
const queue = computed(() => queueState.value?.queue ?? []);

let refreshTimeout: ReturnType<typeof setTimeout> | null = null;

useListen("update_library", () => {
  if (refreshTimeout) clearTimeout(refreshTimeout);
  // Silent: this fires while the user is looking at a populated grid, so a
  // failed background refresh must not replace it with an error card.
  refreshTimeout = setTimeout(() => loadLibrary(true, true), 500);
});

onMounted(async () => {
  window.addEventListener("bpm:boxArtOverlay", onBoxArtOverlayChange);
  devLog("state", `[BPM:HOME] mounting — theme: ${theme.value}`);

  // Cache first, then revalidate — the desktop library pattern. Both fetches
  // go out together and each batches its own status lookup, so the page waits
  // on two IPC round trips rather than one per game.
  await Promise.all([loadLibrary(false), homeData.loadRecentGames(queue.value)]);

  // Fire-and-forget: if favorites resolve quickly the spotlight swaps to a
  // random favorite; otherwise the default (most-recent) shows first.
  homeData.pickRandomFavoriteSpotlight();

  // `restoreFocusSnapshot` reads the live registry synchronously, and nothing
  // below the loading gate has been patched into the DOM yet at this point.
  await nextTick();
  if (!focusNav.restoreFocusSnapshot(route.path)) {
    focusNav.autoFocusContent("content");
  }

  loadLibrary(true, true);
  loadShelves();
  loadConsoles();
});

onUnmounted(() => {
  window.removeEventListener("bpm:boxArtOverlay", onBoxArtOverlayChange);
  for (const unsub of _unsubs) unsub();
  _unsubs.length = 0;
  if (refreshTimeout) {
    clearTimeout(refreshTimeout);
    refreshTimeout = null;
  }
});

// The page is kept alive across some BPM navigations, so re-entering home
// without a fresh mount would leave the spotlight stuck on the same game.
watch(
  () => route.path,
  (to, from) => {
    if (to === "/bigpicture" && from !== "/bigpicture") {
      homeData.pickRandomFavoriteSpotlight();
    }
  },
);
</script>

<style scoped>
/* The tile stagger these classes drive was bound in the template but never
   defined, so tiles just appeared raggedly as Vue patched the grid. The
   inline `transitionDelay` on each wrapper does the staggering; this only has
   to supply the from/to states and the transition itself. */
.game-tile-wrapper {
  opacity: 0;
  transform: translateY(8px);
  transition:
    opacity 220ms ease-out,
    transform 220ms ease-out;
}

.game-tile-wrapper.tile-visible {
  opacity: 1;
  transform: none;
}

@media (prefers-reduced-motion: reduce) {
  .game-tile-wrapper {
    opacity: 1;
    transform: none;
    transition: none;
  }
}

/* Rails scroll horizontally under D-pad control (focus-nav scrolls the focused
   tile into view on both axes), so the native bar is just noise on a TV. */
.bpm-rail {
  scrollbar-width: none;
}
.bpm-rail::-webkit-scrollbar {
  display: none;
}
</style>
