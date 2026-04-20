<template>
  <div class="flex h-full" :style="{ backgroundColor: 'var(--bpm-bg)', color: 'var(--bpm-text)' }">
    <!-- Settings sidebar -->
    <div class="w-64 border-r py-6" :style="{ backgroundColor: 'var(--bpm-surface)', borderColor: 'var(--bpm-border)' }">
      <button
        v-for="section in sections"
        :key="section.value"
        :ref="
          (el: any) =>
            registerSidebar(el, {
              onSelect: () => (activeSection = section.value),
            })
        "
        class="w-full px-6 py-3 text-left text-sm font-medium transition-colors"
        :style="{
          color: activeSection === section.value ? 'var(--bpm-accent-hex)' : 'var(--bpm-muted)',
          backgroundColor: activeSection === section.value ? 'color-mix(in srgb, var(--bpm-accent-hex) 10%, transparent)' : 'transparent',
          borderRight: activeSection === section.value ? '2px solid var(--bpm-accent-hex)' : 'none',
        }"
        @click="activeSection = section.value"
      >
        {{ section.label }}
      </button>
    </div>

    <!-- Settings content -->
    <div class="flex-1 overflow-y-auto px-8 py-6">
      <!-- ═══════ Theme ═══════ -->
      <div
        v-if="activeSection === 'theme'"
        class="space-y-5"
      >
        <h3 class="text-lg font-semibold font-display" style="color: var(--bpm-text)">
          Theme
        </h3>

        <!-- Three-column horizontal layout -->
        <div class="grid grid-cols-3 gap-6 items-start">

          <!-- ── Column 1: Visual Theme ── -->
          <div class="space-y-2">
            <h4 class="text-sm font-medium text-zinc-300 mb-2">Visual Theme</h4>
            <p class="text-zinc-500 text-xs mb-3">
              Colors and home layout for Big Picture Mode.
            </p>
            <div class="space-y-2 max-h-[60vh] overflow-y-auto pr-1">
              <button
                v-for="t in themes"
                :key="t.id"
                :ref="
                  (el: any) =>
                    registerContent(el, {
                      onSelect: () => (activeThemeId = t.id),
                    })
                "
                class="w-full flex items-center gap-3 px-4 py-3 rounded-xl text-left transition-all"
                :class="
                  activeThemeId === t.id
                    ? 'bg-blue-600/20 ring-2 ring-blue-500/60'
                    : 'bg-zinc-900/50 hover:bg-zinc-800/50'
                "
                @click="activeThemeId = t.id"
              >
                <div
                  class="size-4 rounded-full shrink-0 ring-2 ring-white/20"
                  :style="{ backgroundColor: themePreviewColors[t.id] }"
                />
                <div class="flex-1 min-w-0">
                  <p class="font-medium text-zinc-200 text-sm truncate">{{ t.label }}</p>
                  <p class="text-zinc-500 text-xs mt-0.5 truncate">{{ t.description }}</p>
                </div>
                <span v-if="activeThemeId === t.id" class="text-xs font-medium text-blue-400 shrink-0">Active</span>
              </button>
            </div>

            <!-- Dark / Light Mode toggle — below theme list -->
            <div
              :ref="(el: any) => registerContent(el, { onSelect: () => bpmTheme.toggleMode() })"
              class="flex items-center justify-between rounded-xl cursor-pointer p-3 mt-3"
              style="background-color: var(--bpm-surface)"
            >
              <div class="flex items-center gap-2">
                <svg v-if="bpmTheme.mode.value === 'dark'" class="size-4" style="color: var(--bpm-muted)" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" d="M21.752 15.002A9.718 9.718 0 0118 15.75c-5.385 0-9.75-4.365-9.75-9.75 0-1.33.266-2.597.748-3.752A9.753 9.753 0 003 11.25C3 16.635 7.365 21 12.75 21a9.753 9.753 0 009.002-5.998z" />
                </svg>
                <svg v-else class="size-4" style="color: var(--bpm-muted)" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" d="M12 3v2.25m6.364.386l-1.591 1.591M21 12h-2.25m-.386 6.364l-1.591-1.591M12 18.75V21m-4.773-4.227l-1.591 1.591M5.25 12H3m4.227-4.773L5.636 5.636M15.75 12a3.75 3.75 0 11-7.5 0 3.75 3.75 0 017.5 0z" />
                </svg>
                <span class="text-sm font-medium" style="color: var(--bpm-text)">
                  {{ bpmTheme.mode.value === 'dark' ? 'Dark' : 'Light' }}
                </span>
              </div>
              <button
                class="w-10 h-6 rounded-full transition-colors relative shrink-0"
                :class="bpmTheme.mode.value === 'light' ? 'bg-amber-500' : 'bg-zinc-600'"
                @click.stop="bpmTheme.toggleMode()"
              >
                <div
                  class="absolute top-0.5 size-5 rounded-full bg-white shadow transition-transform"
                  :class="bpmTheme.mode.value === 'light' ? 'translate-x-4' : 'translate-x-0.5'"
                />
              </button>
            </div>
          </div>

          <!-- ── Column 2: Sound Profile ── -->
          <div class="space-y-2">
            <h4 class="text-sm font-medium text-zinc-300 mb-2">Sound Profile</h4>
            <p class="text-zinc-500 text-xs mb-3">
              Audio feedback style, independent from visual theme.
            </p>
            <div class="space-y-2 max-h-[52vh] overflow-y-auto pr-1">
              <button
                v-for="sp in soundProfiles"
                :key="sp.id"
                :ref="
                  (el: any) =>
                    registerContent(el, {
                      onSelect: () => { activeSoundProfile = sp.id; audio.setProfile(sp.id); audio.preview(sp.id, 'select'); },
                    })
                "
                class="w-full flex items-center gap-3 px-4 py-3 rounded-xl text-left transition-all"
                :class="
                  activeSoundProfile === sp.id
                    ? 'bg-blue-600/20 ring-2 ring-blue-500/60'
                    : 'bg-zinc-900/50 hover:bg-zinc-800/50'
                "
                @click="activeSoundProfile = sp.id; audio.setProfile(sp.id); audio.preview(sp.id, 'select')"
              >
                <div
                  class="size-4 rounded-full shrink-0 ring-2 ring-white/20 flex items-center justify-center bg-zinc-800"
                >
                  <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="currentColor" class="size-2.5 text-zinc-400">
                    <path d="M10 3.75a.75.75 0 0 0-1.264-.546L4.703 7H3.167a.75.75 0 0 0-.7.48A6.985 6.985 0 0 0 2 10c0 .887.165 1.737.468 2.52.111.29.39.48.7.48h1.535l4.033 3.796A.75.75 0 0 0 10 16.25V3.75ZM15.95 5.05a.75.75 0 0 0-1.06 1.061 5.5 5.5 0 0 1 0 7.778.75.75 0 0 0 1.06 1.06 7 7 0 0 0 0-9.899Z" />
                    <path d="M13.829 7.172a.75.75 0 0 0-1.061 1.06 2.5 2.5 0 0 1 0 3.536.75.75 0 0 0 1.06 1.06 4 4 0 0 0 0-5.656Z" />
                  </svg>
                </div>
                <div class="flex-1 min-w-0">
                  <p class="font-medium text-zinc-200 text-sm truncate">{{ sp.label }}</p>
                  <p class="text-zinc-500 text-xs mt-0.5 truncate">{{ sp.description }}</p>
                </div>
                <span v-if="activeSoundProfile === sp.id" class="text-xs font-medium text-blue-400 shrink-0">Active</span>
              </button>
            </div>

            <!-- Audio toggle -->
            <div class="pt-2">
              <div
                :ref="
                  (el: any) =>
                    registerContent(el, {
                      onSelect: () => (audioEnabled = !audioEnabled),
                    })
                "
                class="flex items-center justify-between bg-zinc-900/50 rounded-xl cursor-pointer p-3"
              >
                <div>
                  <p class="font-medium text-zinc-200 text-sm">Enable Sounds</p>
                  <p class="text-zinc-500 text-xs mt-0.5">Audio feedback on navigation</p>
                </div>
                <button
                  class="w-11 h-6 rounded-full transition-colors relative shrink-0 ml-3"
                  :class="audioEnabled ? 'bg-blue-600' : 'bg-zinc-700'"
                  @click.stop="audioEnabled = !audioEnabled"
                >
                  <div
                    class="absolute top-0.5 size-5 rounded-full bg-white shadow transition-transform"
                    :class="audioEnabled ? 'translate-x-5' : 'translate-x-0.5'"
                  />
                </button>
              </div>
            </div>
          </div>

          <!-- ── Column 3: Effects ── -->
          <div class="space-y-2">
            <h4 class="text-sm font-medium text-zinc-300 mb-2">Effects</h4>
            <p class="text-zinc-500 text-xs mb-3">
              Visual effects and overlays.
            </p>
            <div class="space-y-2">
              <!-- Animated Backgrounds -->
              <div
                :ref="(el: any) => registerContent(el, { onSelect: () => toggleEffect('bpm:animBg') })"
                class="flex items-center justify-between bg-zinc-900/50 rounded-xl cursor-pointer p-3"
              >
                <div>
                  <p class="font-medium text-zinc-200 text-sm">Animated BGs</p>
                  <p class="text-zinc-500 text-xs mt-0.5">Animated theme backgrounds</p>
                </div>
                <button
                  class="w-11 h-6 rounded-full transition-colors relative shrink-0 ml-3"
                  :class="animBgEnabled ? 'bg-blue-600' : 'bg-zinc-700'"
                  @click.stop="toggleEffect('bpm:animBg')"
                >
                  <div
                    class="absolute top-0.5 size-5 rounded-full bg-white shadow transition-transform"
                    :class="animBgEnabled ? 'translate-x-5' : 'translate-x-0.5'"
                  />
                </button>
              </div>

              <!-- CRT Filter -->
              <div
                :ref="(el: any) => registerContent(el, { onSelect: () => toggleEffect('bpm:crtFilter') })"
                class="flex items-center justify-between bg-zinc-900/50 rounded-xl cursor-pointer p-3"
              >
                <div>
                  <p class="font-medium text-zinc-200 text-sm">CRT Filter</p>
                  <p class="text-zinc-500 text-xs mt-0.5">Retro scanline overlay</p>
                </div>
                <button
                  class="w-11 h-6 rounded-full transition-colors relative shrink-0 ml-3"
                  :class="crtEnabled ? 'bg-blue-600' : 'bg-zinc-700'"
                  @click.stop="toggleEffect('bpm:crtFilter')"
                >
                  <div
                    class="absolute top-0.5 size-5 rounded-full bg-white shadow transition-transform"
                    :class="crtEnabled ? 'translate-x-5' : 'translate-x-0.5'"
                  />
                </button>
              </div>

              <!-- Screensaver -->
              <div
                :ref="(el: any) => registerContent(el, { onSelect: () => toggleEffect('bpm:screensaver') })"
                class="flex items-center justify-between bg-zinc-900/50 rounded-xl cursor-pointer p-3"
              >
                <div>
                  <p class="font-medium text-zinc-200 text-sm">Screensaver</p>
                  <p class="text-zinc-500 text-xs mt-0.5">After 5 minutes idle</p>
                </div>
                <button
                  class="w-11 h-6 rounded-full transition-colors relative shrink-0 ml-3"
                  :class="screensaverEnabled ? 'bg-blue-600' : 'bg-zinc-700'"
                  @click.stop="toggleEffect('bpm:screensaver')"
                >
                  <div
                    class="absolute top-0.5 size-5 rounded-full bg-white shadow transition-transform"
                    :class="screensaverEnabled ? 'translate-x-5' : 'translate-x-0.5'"
                  />
                </button>
              </div>
            </div>
          </div>

        </div>
      </div>

      <!-- ═══════ Interface ═══════ -->
      <div
        v-if="activeSection === 'interface'"
        class="space-y-5 max-w-xl"
      >
        <h3 class="text-lg font-semibold text-zinc-200 font-display">
          Interface
        </h3>

        <!-- Start in Big Picture toggle -->
        <div
          :ref="
            (el: any) => registerContent(el, { onSelect: () => toggleStartInBP() })
          "
          class="flex items-center justify-between bg-zinc-900/50 rounded-xl cursor-pointer p-4"
        >
          <div>
            <p class="font-medium text-zinc-200 text-sm">
              Start in Big Picture Mode
            </p>
            <p class="text-zinc-500 text-xs mt-0.5">
              Launch Drop in fullscreen Big Picture mode
            </p>
          </div>
          <button
            class="w-12 h-7 rounded-full transition-colors relative shrink-0 ml-4"
            :class="startInBP ? 'bg-blue-600' : 'bg-zinc-700'"
            @click.stop="toggleStartInBP()"
          >
            <div
              class="absolute top-0.5 size-6 rounded-full bg-white shadow transition-transform"
              :class="startInBP ? 'translate-x-5' : 'translate-x-0.5'"
            />
          </button>
        </div>

        <!-- Hide Game Titles toggle -->
        <div
          :ref="
            (el: any) => registerContent(el, { onSelect: () => toggleHideTitles() })
          "
          class="flex items-center justify-between rounded-xl cursor-pointer p-4"
          style="background-color: var(--bpm-surface)"
        >
          <div>
            <p class="font-medium text-sm" style="color: var(--bpm-text)">
              Hide Game Titles
            </p>
            <p class="text-xs mt-0.5" style="color: var(--bpm-muted)">
              Hide text labels under game tiles for a cleaner look
            </p>
          </div>
          <button
            class="w-12 h-7 rounded-full transition-colors relative shrink-0 ml-4"
            :class="hideTitles ? 'bg-blue-600' : 'bg-zinc-700'"
            @click.stop="toggleHideTitles()"
          >
            <div
              class="absolute top-0.5 size-6 rounded-full bg-white shadow transition-transform"
              :class="hideTitles ? 'translate-x-5' : 'translate-x-0.5'"
            />
          </button>
        </div>

        <!-- Reduced Motion (turns off blur/animations on low-end GPUs like the Deck) -->
        <div
          :ref="(el: any) => registerContent(el, { onSelect: () => (reducedMotion = !reducedMotion) })"
          class="flex items-center justify-between rounded-xl cursor-pointer p-4"
          style="background-color: var(--bpm-surface)"
        >
          <div>
            <p class="font-medium text-sm" style="color: var(--bpm-text)">
              Reduced Motion
            </p>
            <p class="text-xs mt-0.5" style="color: var(--bpm-muted)">
              Disable backdrop blur and soft animations. Recommended on Steam Deck.
            </p>
          </div>
          <button
            class="w-12 h-7 rounded-full transition-colors relative shrink-0 ml-4"
            :class="reducedMotion ? 'bg-blue-600' : 'bg-zinc-700'"
            @click.stop="reducedMotion = !reducedMotion"
          >
            <div
              class="absolute top-0.5 size-6 rounded-full bg-white shadow transition-transform"
              :class="reducedMotion ? 'translate-x-5' : 'translate-x-0.5'"
            />
          </button>
        </div>

        <!-- UI Zoom (rescales the whole webview — fixes gamescope "too zoomed out") -->
        <div class="rounded-xl p-4" style="background-color: var(--bpm-surface)">
          <div class="mb-3">
            <p class="font-medium text-sm" style="color: var(--bpm-text)">Interface zoom</p>
            <p class="text-xs mt-0.5" style="color: var(--bpm-muted)">
              Rescale the whole interface. Use this if the app looks too small in Steam's Game Mode.
            </p>
          </div>
          <div class="flex items-center gap-3">
            <button
              :ref="(el: any) => registerContent(el, { onSelect: () => bumpUiZoom(-0.05) })"
              :disabled="uiZoom <= uiZoomMin"
              class="size-9 inline-flex items-center justify-center rounded-lg bg-zinc-800 text-zinc-300 hover:bg-zinc-700 disabled:opacity-30 disabled:cursor-not-allowed transition-colors"
              aria-label="Decrease UI zoom"
              @click.stop="bumpUiZoom(-0.05)"
            >
              <span class="text-lg leading-none">−</span>
            </button>
            <div class="flex-1 text-center text-sm tabular-nums text-zinc-200">
              {{ Math.round(uiZoom * 100) }}%
            </div>
            <button
              :ref="(el: any) => registerContent(el, { onSelect: () => bumpUiZoom(0.05) })"
              :disabled="uiZoom >= uiZoomMax"
              class="size-9 inline-flex items-center justify-center rounded-lg bg-zinc-800 text-zinc-300 hover:bg-zinc-700 disabled:opacity-30 disabled:cursor-not-allowed transition-colors"
              aria-label="Increase UI zoom"
              @click.stop="bumpUiZoom(0.05)"
            >
              <span class="text-lg leading-none">+</span>
            </button>
            <button
              :ref="(el: any) => registerContent(el, { onSelect: () => (uiZoom = 1) })"
              class="px-3 h-9 inline-flex items-center rounded-lg text-xs font-medium bg-zinc-800 text-zinc-300 hover:bg-zinc-700 transition-colors"
              @click.stop="uiZoom = 1"
            >
              Reset
            </button>
          </div>
        </div>

        <!-- Keyboard preference (custom BPM keyboard vs SteamOS OSK) -->
        <div class="rounded-xl p-4" style="background-color: var(--bpm-surface)">
          <div class="mb-3">
            <p class="font-medium text-sm" style="color: var(--bpm-text)">On-screen keyboard</p>
            <p class="text-xs mt-0.5" style="color: var(--bpm-muted)">
              Steam's native keyboard requires Steam to be running; falls back to Drop's built-in keyboard if unavailable.
            </p>
          </div>
          <div class="flex gap-2">
            <button
              v-for="option in keyboardOptions"
              :key="option.value"
              :ref="(el: any) => registerContent(el, { onSelect: () => (keyboardMode = option.value) })"
              class="flex-1 py-2.5 rounded-lg text-xs font-medium transition-all border"
              :class="[
                keyboardMode === option.value
                  ? 'bg-blue-600/20 text-blue-400 border-blue-500/50'
                  : 'bg-zinc-800/50 text-zinc-400 border-zinc-700/50 hover:text-zinc-200 hover:bg-zinc-800',
              ]"
              @click="keyboardMode = option.value"
            >
              {{ option.label }}
            </button>
          </div>
        </div>

      </div>

      <!-- ═══════ Performance ═══════ -->
      <div
        v-if="activeSection === 'performance'"
        class="space-y-5 max-w-xl"
      >
        <h3 class="text-lg font-semibold text-zinc-200 font-display">
          Performance
        </h3>

        <!-- MangoHud overlay (Linux only) -->
        <div class="bg-zinc-900/50 rounded-xl p-4">
          <div class="mb-3">
            <p class="font-medium text-zinc-200 text-sm">Performance Overlay</p>
            <p class="text-zinc-500 text-xs mt-0.5">
              Show MangoHud FPS/performance overlay in games (Linux only)
            </p>
          </div>
          <div class="flex gap-2">
            <button
              v-for="option in mangohudOptions"
              :key="option.value"
              :ref="
                (el: any) =>
                  registerContent(el, {
                    onSelect: () => (mangohudPreset = option.value),
                  })
              "
              class="flex-1 py-2.5 rounded-lg text-xs font-medium transition-all border"
              :class="[
                mangohudPreset === option.value
                  ? 'bg-blue-600/20 text-blue-400 border-blue-500/50'
                  : 'bg-zinc-800/50 text-zinc-400 border-zinc-700/50 hover:text-zinc-200 hover:bg-zinc-800',
              ]"
              @click="mangohudPreset = option.value"
            >
              {{ option.label }}
            </button>
          </div>
          <p class="text-zinc-600 text-xs mt-2">
            Applies per-game via MANGOHUD environment variable. Requires
            MangoHud installed.
          </p>
        </div>
      </div>

      <!-- ═══════ Steam Integration ═══════ -->
      <div
        v-if="activeSection === 'steam'"
        class="space-y-5 max-w-xl"
      >
        <h3 class="text-lg font-semibold text-zinc-200 font-display">
          Steam Integration
        </h3>

        <!-- Add to Steam Library -->
        <div class="bg-zinc-900/50 rounded-xl p-4">
          <div class="mb-3">
            <p class="font-medium text-zinc-200 text-sm">Add Drop to Steam</p>
            <p class="text-zinc-500 text-xs mt-0.5">
              Register Drop as a non-Steam game so it appears in SteamOS Game
              Mode
            </p>
          </div>
          <button
            :ref="(el: any) => registerContent(el, { onSelect: addToSteam })"
            class="px-4 py-2.5 rounded-lg text-sm font-medium transition-all border"
            :class="[
              steamShortcutResult?.success
                ? 'bg-emerald-600/20 text-emerald-400 border-emerald-500/50'
                : 'bg-blue-600/20 text-blue-400 border-blue-500/50 hover:bg-blue-600/30',
            ]"
            :disabled="steamShortcutLoading"
            @click="addToSteam"
          >
            {{
              steamShortcutLoading
                ? "Adding..."
                : steamShortcutResult?.success
                  ? "Added to Steam"
                  : "Add to Steam Library"
            }}
          </button>
          <p
            v-if="steamShortcutResult"
            class="text-xs mt-2"
            :class="
              steamShortcutResult.success
                ? 'text-emerald-400/60'
                : 'text-red-400/60'
            "
          >
            {{ steamShortcutResult.message }}
          </p>
        </div>
      </div>

      <!-- ═══════ Controller ═══════ -->
      <div
        v-if="activeSection === 'controller'"
        class="space-y-5 max-w-xl"
      >
        <h3 class="text-lg font-semibold text-zinc-200 font-display">
          Controller
        </h3>
        <div class="bg-zinc-900/50 rounded-xl p-4">
          <div class="flex items-center gap-3 mb-3">
            <div
              class="size-3 rounded-full"
              :class="gamepad.connected.value ? 'bg-green-500' : 'bg-zinc-600'"
            />
            <p class="font-medium text-zinc-200 text-sm">
              {{
                gamepad.connected.value
                  ? gamepad.controllerName.value || "Controller Connected"
                  : "No Controller Detected"
              }}
            </p>
          </div>
          <p class="text-zinc-500 text-xs">
            Connect a controller to use Big Picture Mode with a gamepad.
          </p>
        </div>

        <!-- Haptic feedback toggle -->
        <div
          :ref="
            (el: any) =>
              registerContent(el, {
                onSelect: () => (hapticEnabled = !hapticEnabled),
              })
          "
          class="flex items-center justify-between bg-zinc-900/50 rounded-xl cursor-pointer p-4"
        >
          <div>
            <p class="font-medium text-zinc-200 text-sm">Haptic Feedback</p>
            <p class="text-zinc-500 text-xs mt-0.5">
              Vibration on navigation and selection
            </p>
          </div>
          <button
            class="w-12 h-7 rounded-full transition-colors relative shrink-0 ml-4"
            :class="hapticEnabled ? 'bg-blue-600' : 'bg-zinc-700'"
            @click.stop="hapticEnabled = !hapticEnabled"
          >
            <div
              class="absolute top-0.5 size-6 rounded-full bg-white shadow transition-transform"
              :class="hapticEnabled ? 'translate-x-5' : 'translate-x-0.5'"
            />
          </button>
        </div>
      </div>

      <!-- ═══════ Storage ═══════ -->
      <div
        v-if="activeSection === 'storage'"
        class="space-y-5 max-w-xl"
      >
        <h3 class="text-lg font-semibold text-zinc-200 font-display">
          Storage
        </h3>

        <!-- Install directories -->
        <div class="bg-zinc-900/50 rounded-xl p-4">
          <div class="mb-3">
            <p class="font-medium text-zinc-200 text-sm">Install Directories</p>
            <p class="text-zinc-500 text-xs mt-0.5">
              Where games are installed on this device
            </p>
          </div>
          <div v-if="installDirs.length > 0" class="space-y-2 mb-3">
            <div
              v-for="(dir, idx) in installDirs"
              :key="idx"
              class="flex items-center justify-between bg-zinc-800/50 rounded-lg px-3 py-2"
            >
              <span class="text-xs text-zinc-300 truncate">{{ dir }}</span>
            </div>
          </div>
          <p v-else class="text-zinc-600 text-xs mb-3">
            No install directories configured.
          </p>
        </div>

        <!-- Max concurrent download threads -->
        <div class="bg-zinc-900/50 rounded-xl p-4">
          <div class="mb-3">
            <p class="font-medium text-zinc-200 text-sm">Concurrent download threads</p>
            <p class="text-zinc-500 text-xs mt-0.5">
              Higher values saturate the network faster but use more CPU.
              1–64 allowed; defaults to 4.
            </p>
          </div>
          <div class="flex items-center gap-3">
            <button
              :ref="(el: any) => registerContent(el, { onSelect: () => bumpThreads(-1) })"
              class="px-3 py-2 rounded-lg bg-zinc-800 text-zinc-300 text-sm"
              @click="bumpThreads(-1)"
            >
              −
            </button>
            <span class="min-w-[2rem] text-center text-sm font-mono text-zinc-200">
              {{ maxDownloadThreads }}
            </span>
            <button
              :ref="(el: any) => registerContent(el, { onSelect: () => bumpThreads(1) })"
              class="px-3 py-2 rounded-lg bg-zinc-800 text-zinc-300 text-sm"
              @click="bumpThreads(1)"
            >
              +
            </button>
            <span v-if="threadsSaved" class="text-xs text-green-400 ml-2">Saved</span>
          </div>
        </div>

        <!-- SD Card / Removable storage -->
        <div
          v-if="removableStorage.length > 0"
          class="bg-zinc-900/50 rounded-xl p-4"
        >
          <div class="mb-3">
            <p class="font-medium text-zinc-200 text-sm">Removable Storage</p>
            <p class="text-zinc-500 text-xs mt-0.5">
              SD cards and USB drives detected on this device
            </p>
          </div>
          <div class="space-y-2">
            <div
              v-for="(path, idx) in removableStorage"
              :key="idx"
              class="flex items-center justify-between bg-zinc-800/50 rounded-lg px-3 py-2"
            >
              <span class="text-xs text-zinc-300 truncate">{{ path }}</span>
              <button
                :ref="
                  (el: any) =>
                    registerContent(el, {
                      onSelect: () => addStorageAsInstallDir(path),
                    })
                "
                class="text-xs text-blue-400 hover:text-blue-300 ml-2 whitespace-nowrap"
                @click="addStorageAsInstallDir(path)"
              >
                Use for games
              </button>
            </div>
          </div>
        </div>
      </div>

      <!-- ═══════ Compatibility ═══════ -->
      <div
        v-if="activeSection === 'compatibility'"
        class="space-y-5 max-w-xl"
      >
        <h3 class="text-lg font-semibold text-zinc-200 font-display">
          Compatibility
        </h3>

        <!-- Proton / UMU status -->
        <div class="bg-zinc-900/50 rounded-xl p-4">
          <div class="mb-3">
            <p class="font-medium text-zinc-200 text-sm">
              Proton (Windows Game Support)
            </p>
            <p class="text-zinc-500 text-xs mt-0.5">
              Select a Proton installation to run Windows games on Linux.
            </p>
          </div>

          <div v-if="protonLoading" class="text-zinc-500 text-xs py-2">
            Discovering Proton installations...
          </div>

          <div v-else class="space-y-2">
            <!-- Auto / GE-Proton — recommended default, umu downloads it on first launch -->
            <button
              :ref="(el: any) => registerContent(el, { onSelect: () => setAutoProton() })"
              class="w-full flex items-center justify-between bg-zinc-800/50 rounded-lg px-3 py-2.5 text-left transition-colors"
              :class="[
                selectedProtonDefault === null
                  ? 'ring-1 ring-blue-500/50 bg-blue-600/10'
                  : 'hover:bg-zinc-700/50',
              ]"
              @click="setAutoProton()"
            >
              <div class="min-w-0">
                <span class="text-xs font-medium text-zinc-200 block truncate">
                  Auto (GE-Proton, recommended)
                </span>
                <span class="text-[10px] text-zinc-500 block truncate">
                  umu-launcher downloads the latest GE-Proton on first launch
                </span>
              </div>
              <div
                v-if="selectedProtonDefault === null"
                class="size-2 rounded-full bg-blue-500 shrink-0 ml-2"
              />
            </button>

            <div v-if="allProtonPaths.length === 0" class="py-2">
              <p class="text-zinc-500 text-xs">
                No local Proton installations detected. Auto works even without
                one — umu fetches GE-Proton on its own.
              </p>
            </div>
            <button
              v-for="proton in allProtonPaths"
              :key="proton.path"
              :ref="(el: any) => registerContent(el, { onSelect: () => setDefaultProton(proton.path) })"
              class="w-full flex items-center justify-between bg-zinc-800/50 rounded-lg px-3 py-2.5 text-left transition-colors"
              :class="[
                proton.path === selectedProtonDefault
                  ? 'ring-1 ring-blue-500/50 bg-blue-600/10'
                  : 'hover:bg-zinc-700/50',
              ]"
              @click="setDefaultProton(proton.path)"
            >
              <div class="min-w-0">
                <span class="text-xs font-medium text-zinc-200 block truncate">{{ proton.name }}</span>
                <span class="text-[10px] text-zinc-500 block truncate">{{ proton.path }}</span>
              </div>
              <div
                v-if="proton.path === selectedProtonDefault"
                class="size-2 rounded-full bg-blue-500 shrink-0 ml-2"
              />
            </button>
          </div>

          <p v-if="protonSaveError" class="text-red-400/80 text-xs mt-2">
            {{ protonSaveError }}
          </p>
        </div>
      </div>

      <!-- ═══════ Streaming ═══════ -->
      <div
        v-if="activeSection === 'streaming'"
        class="space-y-5 max-w-xl"
      >
        <h3 class="text-lg font-semibold text-zinc-200 font-display">
          Streaming (Sunshine)
        </h3>
        <p class="text-sm text-zinc-400">
          Configure Sunshine credentials used when starting a stream from a game page.
          These are stored locally and used to start the Sunshine process.
        </p>

        <div class="bg-zinc-900/50 rounded-xl p-4 space-y-4">
          <div>
            <label class="block text-sm font-medium text-zinc-400 mb-1">Admin Username</label>
            <input
              v-model="streamingUsername"
              :ref="(el: any) => registerContent(el, {})"
              type="text"
              class="w-full px-3 py-2 text-sm bg-zinc-800 border border-zinc-700 rounded-lg text-zinc-200 focus:outline-none focus:ring-1 focus:ring-blue-500"
              placeholder="sunshine"
              @change="saveStreamingCredentials"
            />
          </div>
          <div>
            <label class="block text-sm font-medium text-zinc-400 mb-1">Admin Password</label>
            <input
              v-model="streamingPassword"
              :ref="(el: any) => registerContent(el, {})"
              type="password"
              class="w-full px-3 py-2 text-sm bg-zinc-800 border border-zinc-700 rounded-lg text-zinc-200 focus:outline-none focus:ring-1 focus:ring-blue-500"
              placeholder="Enter password"
              @change="saveStreamingCredentials"
            />
          </div>
          <p v-if="streamingSaved" class="text-xs text-green-400">Credentials saved.</p>
        </div>
      </div>

      <!-- ═══════ About ═══════ -->
      <div
        v-if="activeSection === 'about'"
        class="space-y-5 max-w-xl"
      >
        <h3 class="text-lg font-semibold text-zinc-200 font-display">
          About Drop
        </h3>
        <div class="bg-zinc-900/50 rounded-xl p-4 space-y-2">
          <p class="text-zinc-300 text-sm">
            Drop is an open-source, self-hosted game distribution platform.
          </p>
          <p class="text-zinc-500 text-xs">Big Picture Mode v1.0</p>
          <div class="border-t border-zinc-800/50 pt-2 mt-2 space-y-1">
            <p v-if="packageFormat" class="text-zinc-500 text-xs">
              Package: {{ packageFormatLabel }}
            </p>
          </div>
        </div>

        <!-- Bug Report -->
        <div
          :ref="(el: any) => registerContent(el, { onSelect: () => navigateTo('/bigpicture/bugreport') })"
          class="flex items-center justify-between bg-zinc-900/50 rounded-xl cursor-pointer p-4 hover:bg-zinc-800/50 transition-colors"
          @click="navigateTo('/bigpicture/bugreport')"
        >
          <div>
            <p class="font-medium text-zinc-200 text-sm">Submit Bug Report</p>
            <p class="text-zinc-500 text-xs mt-0.5">
              Report an issue with system diagnostics and logs attached automatically
            </p>
          </div>
          <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="currentColor" class="size-5 text-zinc-500">
            <path fill-rule="evenodd" d="M8.22 5.22a.75.75 0 0 1 1.06 0l4.25 4.25a.75.75 0 0 1 0 1.06l-4.25 4.25a.75.75 0 0 1-1.06-1.06L11.94 10 8.22 6.28a.75.75 0 0 1 0-1.06Z" clip-rule="evenodd" />
          </svg>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";

import { useGamepad } from "~/composables/gamepad";
import { useBpFocusableGroup } from "~/composables/bp-focusable";
import { useBpAudio, soundProfiles, type SoundProfileId } from "~/composables/bp-audio";
import { useBpmTheme, themes, type ThemeId } from "~/composables/bp-theme";
import { useDeckMode } from "~/composables/deck-mode";
import { useUiZoom } from "~/composables/ui-zoom";
import { type Ref } from "vue";

definePageMeta({ layout: "bigpicture" });

const gamepad = useGamepad();
const activeSection = ref("theme");

// C7 fix: persist BPM settings in localStorage
const startInBP = ref(
  typeof localStorage !== "undefined"
    ? localStorage.getItem("drop:startInBPM") === "true"
    : false,
);
watch(startInBP, (val) => {
  if (typeof localStorage !== "undefined") {
    localStorage.setItem("drop:startInBPM", String(val));
  }
});

function toggleStartInBP() {
  startInBP.value = !startInBP.value;
}

const hideTitles = ref(
  typeof localStorage !== "undefined"
    ? localStorage.getItem("drop:hideTitles") === "true"
    : false,
);
watch(hideTitles, (val) => {
  if (typeof localStorage !== "undefined") {
    localStorage.setItem("drop:hideTitles", String(val));
  }
});

function toggleHideTitles() {
  hideTitles.value = !hideTitles.value;
}

// ── Reduced motion — persisted via localStorage, consumed by BPM chrome
//    (backdrop-blur off, animations shortened). Default true when we detect
//    Steam Deck hardware so first-run is already snappy.
const deckMode = useDeckMode();
const reducedMotion = ref(
  typeof localStorage !== "undefined"
    ? (localStorage.getItem("bpm:reducedMotion") ?? (deckMode.isSteamDeckHardware.value ? "true" : "false")) === "true"
    : false,
);
watch(reducedMotion, (val) => {
  if (typeof localStorage !== "undefined") {
    localStorage.setItem("bpm:reducedMotion", String(val));
    // Broadcast so layout-level providers pick it up without a full reload
    window.dispatchEvent(new CustomEvent("bpm:reducedMotion", { detail: val }));
  }
});

// UI zoom — rescales the entire webview. Useful when gamescope renders the
// layout smaller than expected.
const { zoom: uiZoom, minZoom: uiZoomMin, maxZoom: uiZoomMax } = useUiZoom();
function bumpUiZoom(delta: number) {
  uiZoom.value = Math.round((uiZoom.value + delta) * 100) / 100;
}

// Keyboard preference: "custom" uses Drop's on-screen keyboard; "steam"
// tries steam://open/keyboard first and falls back to custom on failure.
type KeyboardMode = "custom" | "steam";
const keyboardOptions: { label: string; value: KeyboardMode }[] = [
  { label: "Drop (built-in)", value: "custom" },
  { label: "Steam OSK", value: "steam" },
];
const keyboardMode = ref<KeyboardMode>(
  typeof localStorage !== "undefined"
    ? ((localStorage.getItem("bpm:keyboardMode") as KeyboardMode) ?? "custom")
    : "custom",
);
watch(keyboardMode, (val) => {
  if (typeof localStorage !== "undefined") {
    localStorage.setItem("bpm:keyboardMode", val);
  }
});

const registerSidebar = useBpFocusableGroup("content");
const registerContent = useBpFocusableGroup("content");

const sections = [
  { label: "Theme", value: "theme" },
  { label: "Interface", value: "interface" },
  { label: "Performance", value: "performance" },
  { label: "Steam", value: "steam" },
  { label: "Controller", value: "controller" },
  { label: "Storage", value: "storage" },
  { label: "Compatibility", value: "compatibility" },
  { label: "Streaming", value: "streaming" },
  { label: "About", value: "about" },
];

// ── Streaming credentials ──────────────────────────────────────────────────

const streamingUsername = ref("sunshine");
const streamingPassword = ref("");
const streamingSaved = ref(false);

onMounted(async () => {
  try {
    const settings = await invoke<Record<string, any>>("fetch_settings");
    if (settings.sunshineUsername) streamingUsername.value = settings.sunshineUsername;
    if (settings.sunshinePassword) streamingPassword.value = settings.sunshinePassword;
  } catch {
    // Settings not available yet — keep defaults
  }
});

async function saveStreamingCredentials() {
  try {
    await invoke("update_settings", {
      newSettings: {
        sunshineUsername: streamingUsername.value,
        sunshinePassword: streamingPassword.value,
      },
    });
    streamingSaved.value = true;
    setTimeout(() => { streamingSaved.value = false; }, 2000);
  } catch (e) {
    console.error("[BPM:SETTINGS] Failed to save streaming credentials:", e);
  }
}

// ── MangoHud ────────────────────────────────────────────────────────────────

const mangohudOptions = [
  { label: "Off", value: "off" },
  { label: "FPS Only", value: "minimal" },
  { label: "Standard", value: "standard" },
  { label: "Full", value: "full" },
];

const mangohudPreset = ref("off");

// Load the current global MangoHud preset from backend settings
onMounted(async () => {
  try {
    const settings = await invoke<{ globalMangohud?: string }>(
      "fetch_settings",
    );
    if (settings.globalMangohud) {
      mangohudPreset.value = settings.globalMangohud;
    }
  } catch {
    // Settings not available — keep default
  }
});

watch(mangohudPreset, async (val) => {
  try {
    await invoke("update_settings", {
      newSettings: { globalMangohud: val === "off" ? null : val },
    });
  } catch (e) {
    console.warn("Failed to save MangoHud setting:", e);
  }
});

// ── Haptic feedback ─────────────────────────────────────────────────────────

const hapticEnabled = ref(
  typeof localStorage !== "undefined"
    ? localStorage.getItem("drop:haptic") !== "false"
    : true,
);
watch(hapticEnabled, (val) => {
  if (typeof localStorage !== "undefined") {
    localStorage.setItem("drop:haptic", String(val));
  }
});

// ── Effects ─────────────────────────────────────────────────────────────────

const animBgEnabled = ref(
  typeof localStorage !== "undefined"
    ? localStorage.getItem("bpm:animBg") !== "false"
    : true,
);

const crtEnabled = ref(
  typeof localStorage !== "undefined"
    ? localStorage.getItem("bpm:crtFilter") === "true"
    : false,
);


const screensaverEnabled = ref(
  typeof localStorage !== "undefined"
    ? localStorage.getItem("bpm:screensaver") !== "false"
    : true,
);

// Map keys to their refs so we can look them up by string
// (Vue auto-unwraps refs in templates, so we can't pass Ref objects through template bindings)
const effectRefs: Record<string, Ref<boolean>> = {
  "bpm:animBg": animBgEnabled,
  "bpm:crtFilter": crtEnabled,
  "bpm:screensaver": screensaverEnabled,
};

function toggleEffect(key: string) {
  const toggleRef = effectRefs[key];
  if (!toggleRef) {
    console.error("[BPM:SETTINGS] Unknown effect key:", key);
    return;
  }
  const newVal = !toggleRef.value;
  console.log("[BPM:SETTINGS] toggleEffect:", key, "->", newVal);
  toggleRef.value = newVal;
  try {
    if (typeof localStorage !== "undefined") {
      localStorage.setItem(key, String(newVal));
    }
  } catch (e) {
    console.warn("[BPM:SETTINGS] localStorage.setItem failed for", key, e);
  }
}

// ── Audio feedback ──────────────────────────────────────────────────────────

const audio = useBpAudio();
const audioEnabled = ref(audio.enabled);
const activeSoundProfile = ref<SoundProfileId>(audio.profile);
watch(audioEnabled, (val) => {
  audio.setEnabled(val);
});

// ── Theme ──────────────────────────────────────────────────────────────────

const bpmTheme = useBpmTheme();
const activeThemeId = ref<ThemeId>(bpmTheme.theme);

const themePreviewColors: Record<ThemeId, string> = {
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

watch(activeThemeId, (id) => {
  bpmTheme.setTheme(id);
  // Preview the select sound on theme change
  audio.preview(id as SoundProfileId, "select");
});

// ── Steam shortcut ──────────────────────────────────────────────────────────

const steamShortcutLoading = ref(false);
const steamShortcutResult = ref<{ success: boolean; message: string } | null>(
  null,
);

async function addToSteam() {
  if (steamShortcutLoading.value) return;
  steamShortcutLoading.value = true;
  try {
    steamShortcutResult.value = await invoke("register_steam_shortcut");
  } catch (e) {
    steamShortcutResult.value = {
      success: false,
      message: `Failed: ${e instanceof Error ? e.message : String(e)}`,
    };
  } finally {
    steamShortcutLoading.value = false;
  }
}

// ── Package format detection ────────────────────────────────────────────────

const packageFormat = ref<string | null>(null);
onMounted(async () => {
  try {
    packageFormat.value = await invoke("detect_package_format");
  } catch {
    // Command may not exist on all platforms
  }
});

const packageFormatLabel = computed(() => {
  switch (packageFormat.value) {
    case "flatpak": return "Flatpak";
    case "appImage": return "AppImage (self-updating)";
    case "systemPackage": return "System package (DEB/RPM)";
    case "windowsInstaller": return "Windows";
    case "macOsBundle": return "macOS";
    default: return packageFormat.value ?? "Unknown";
  }
});

// ── Storage / Install dirs ──────────────────────────────────────────────────

const installDirs = ref<string[]>([]);
const removableStorage = ref<string[]>([]);
const maxDownloadThreads = ref(4);
const threadsSaved = ref(false);
let threadsDirty = false;
let threadsSaveTimer: ReturnType<typeof setTimeout> | null = null;

onMounted(async () => {
  try {
    const settings = await invoke<{ maxDownloadThreads?: number }>("fetch_settings");
    if (typeof settings.maxDownloadThreads === "number") {
      maxDownloadThreads.value = settings.maxDownloadThreads;
    }
  } catch { /* ignore */ }
});

function bumpThreads(delta: number) {
  const next = Math.max(1, Math.min(64, maxDownloadThreads.value + delta));
  if (next === maxDownloadThreads.value) return;
  maxDownloadThreads.value = next;
  threadsDirty = true;
  if (threadsSaveTimer) clearTimeout(threadsSaveTimer);
  // Debounce rapid presses so we only send one update_settings call.
  threadsSaveTimer = setTimeout(async () => {
    if (!threadsDirty) return;
    threadsDirty = false;
    try {
      await invoke("update_settings", {
        newSettings: { maxDownloadThreads: maxDownloadThreads.value },
      });
      threadsSaved.value = true;
      setTimeout(() => { threadsSaved.value = false; }, 1500);
    } catch (e) {
      console.warn("[BPM:SETTINGS] Failed to save maxDownloadThreads:", e);
    }
  }, 400);
}

onMounted(async () => {
  try {
    installDirs.value = await invoke("fetch_download_dir_stats");
  } catch { /* ignore */ }

  try {
    removableStorage.value = await invoke("detect_removable_storage");
  } catch { /* ignore on non-Linux */ }
});

async function addStorageAsInstallDir(path: string) {
  const gameDir = `${path}/drop-games`;
  try {
    await invoke("add_download_dir", { newDir: gameDir });
    installDirs.value = await invoke("fetch_download_dir_stats");
  } catch (e) {
    console.error("Failed to add storage dir:", e);
  }
}

// ── Proton / Compatibility ─────────────────────────────────────────────────

interface ProtonPath {
  path: string;
  name: string;
}

const protonLoading = ref(true);
const allProtonPaths = ref<ProtonPath[]>([]);
const selectedProtonDefault = ref<string | null>(null);
const protonSaveError = ref<string | null>(null);

onMounted(async () => {
  try {
    const result = await invoke<{
      autodiscovered: ProtonPath[];
      custom: ProtonPath[];
      default: string | null;
    }>("fetch_proton_paths");

    allProtonPaths.value = [...result.autodiscovered, ...result.custom];
    selectedProtonDefault.value = result.default;
    // No auto-selection here: leaving `default` as null is the "Auto
    // (GE-Proton)" opt-in, which umu-launcher resolves on first launch.
    // Previously we silently picked the first discovered Proton, which
    // made the Auto option impossible to preserve across visits.
  } catch (e) {
    console.warn("[BPM:SETTINGS] Proton discovery failed:", e);
  } finally {
    protonLoading.value = false;
  }
});

async function setDefaultProton(path: string) {
  protonSaveError.value = null;
  try {
    await invoke("set_default", { path });
    selectedProtonDefault.value = path;
  } catch (e) {
    protonSaveError.value = `Failed to set default: ${e instanceof Error ? e.message : String(e)}`;
  }
}

async function setAutoProton() {
  protonSaveError.value = null;
  try {
    await invoke("clear_default_proton");
    selectedProtonDefault.value = null;
  } catch (e) {
    protonSaveError.value = `Failed to enable Auto: ${e instanceof Error ? e.message : String(e)}`;
  }
}

</script>
