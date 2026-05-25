<template>
  <div
    class="flex flex-col h-full"
    :style="{ backgroundColor: 'var(--bpm-bg)', color: 'var(--bpm-text)' }"
  >
    <!-- Tab navigation -->
    <div
      class="flex items-center gap-2 px-8 py-4 border-b"
      :style="{ borderColor: 'var(--bpm-border)' }"
    >
      <button
        v-for="tab in tabs"
        :key="tab.value"
        :ref="
          (el: any) =>
            registerTab(el, { onSelect: () => (activeTab = tab.value) })
        "
        class="px-4 py-2 text-sm rounded-lg font-medium transition-colors"
        :class="[
          activeTab === tab.value
            ? 'bg-blue-600/20 text-blue-400'
            : 'text-zinc-400 hover:text-zinc-200 hover:bg-zinc-800/50',
        ]"
        @click="activeTab = tab.value"
      >
        {{ tab.label }}
      </button>
    </div>

    <!-- Loading -->
    <div v-if="loading" class="flex-1 overflow-y-auto px-8 py-6">
      <div class="grid grid-cols-4 gap-4 mb-8">
        <div
          v-for="i in 4"
          :key="i"
          class="h-20 rounded-xl bg-zinc-800/50 animate-pulse"
        />
      </div>
      <div class="space-y-3">
        <div
          v-for="i in 6"
          :key="i"
          class="h-16 rounded-xl bg-zinc-800/50 animate-pulse"
        />
      </div>
    </div>

    <!-- ═══ Activity tab ═══ -->
    <div
      v-else-if="activeTab === 'activity'"
      class="flex-1 overflow-y-auto px-8 py-6"
    >
      <!-- Weekly recap card -->
      <div
        v-if="weeklyRecap.length > 0"
        :ref="(el: any) => registerContent(el, { onSelect: onRecapSelect })"
        class="relative overflow-hidden rounded-2xl bg-gradient-to-br from-indigo-900/40 via-zinc-900/60 to-purple-900/30 ring-1 ring-indigo-500/20 mb-6 cursor-pointer"
        @click="onRecapSelect"
      >
        <div
          class="absolute top-3 left-4 text-[10px] tracking-[0.2em] uppercase text-indigo-300/80 font-medium z-10"
        >
          This week
        </div>
        <transition name="bpm-slide-fade" mode="out-in">
          <div :key="recapIndex" class="px-6 py-7 pl-6 pr-12">
            <p class="text-xs font-medium text-indigo-300 mb-1">
              {{ activeRecapSlide?.title }}
            </p>
            <p
              class="text-lg font-display font-semibold text-zinc-100 leading-snug truncate"
            >
              {{ activeRecapSlide?.subtitle }}
            </p>
          </div>
        </transition>
        <div
          v-if="weeklyRecap.length > 1"
          class="absolute bottom-3 right-4 flex gap-1.5 z-10"
        >
          <button
            v-for="(_, i) in weeklyRecap"
            :key="i"
            class="size-2 rounded-full transition-all"
            :class="i === recapIndex ? 'bg-indigo-300 w-5' : 'bg-indigo-300/30'"
            @click.stop="recapIndex = i"
          />
        </div>
      </div>

      <!-- Drop Time Machine card — sibling of weekly recap, hidden when null -->
      <div
        v-if="timeMachine"
        class="relative overflow-hidden rounded-2xl bg-gradient-to-br from-amber-900/30 via-zinc-900/60 to-rose-900/20 ring-1 ring-amber-500/20 mb-6 px-5 py-4"
      >
        <div class="flex items-center gap-2 mb-3">
          <span
            class="text-[10px] tracking-[0.2em] uppercase text-amber-300/80 font-medium"
          >
            Drop Time Machine
          </span>
        </div>
        <div class="flex items-center gap-4">
          <img
            v-if="timeMachine.user.avatarObjectId"
            :src="objectUrl(timeMachine.user.avatarObjectId)"
            class="size-10 rounded-full object-cover ring-1 ring-amber-400/30 shrink-0"
          />
          <div
            v-else
            class="size-10 rounded-full bg-zinc-700 flex items-center justify-center ring-1 ring-amber-400/30 shrink-0"
          >
            <span class="text-xs font-bold text-zinc-400">
              {{ timeMachine.user.displayName[0]?.toUpperCase() }}
            </span>
          </div>
          <div class="flex-1 min-w-0">
            <p class="text-xs text-amber-300/90 font-medium">
              {{ timeMachineLabel }}
            </p>
            <p class="text-sm text-zinc-100 truncate">
              <span class="font-medium">{{
                timeMachine.user.displayName
              }}</span>
              <span class="text-zinc-400"> · {{ timeMachine.detail }} · </span>
              <span class="text-amber-200">{{ timeMachine.game.name }}</span>
            </p>
          </div>
          <img
            v-if="timeMachine.game.coverObjectId"
            :src="objectUrl(timeMachine.game.coverObjectId)"
            class="h-14 w-10 rounded object-cover shrink-0"
            loading="lazy"
          />
        </div>
      </div>

      <!-- Personal weekly quest card — hidden when endpoint returns null -->
      <div
        v-if="weeklyChallenge"
        class="relative overflow-hidden rounded-2xl bg-gradient-to-br from-amber-900/30 via-zinc-900/70 to-orange-900/20 ring-1 ring-amber-500/25 mb-6 px-5 py-4"
      >
        <div class="flex items-center gap-2 mb-3">
          <span
            class="text-[10px] tracking-[0.2em] uppercase text-amber-300/80 font-medium"
          >
            Your weekly quest
          </span>
          <span class="text-[10px] text-zinc-500">{{
            weeklyChallengeDaysSuffix
          }}</span>
        </div>
        <div class="flex items-center gap-4">
          <div
            class="shrink-0 size-12 rounded-full bg-amber-500/15 ring-1 ring-amber-400/40 flex items-center justify-center"
          >
            <span class="text-amber-300 text-xl">{{ weeklyChallengeEmoji }}</span>
          </div>
          <div class="flex-1 min-w-0">
            <p
              class="text-base font-display font-semibold text-zinc-100 truncate"
            >
              {{ weeklyChallenge.title }}
            </p>
            <p class="text-xs text-zinc-400 mt-0.5 line-clamp-2">
              {{ weeklyChallenge.description }}
            </p>
            <div class="mt-3 flex items-center gap-3">
              <div
                class="relative h-2 flex-1 rounded-full bg-zinc-800/80 overflow-hidden"
              >
                <div
                  class="absolute inset-y-0 left-0 rounded-full bg-gradient-to-r from-amber-500 to-orange-400 transition-[width] duration-700 ease-out"
                  :style="{ width: `${weeklyChallenge.percentComplete}%` }"
                />
              </div>
              <p
                class="text-xs font-medium text-zinc-300 tabular-nums shrink-0"
              >
                {{ formatChallengeValue(weeklyChallenge.currentValue) }} /
                {{ formatChallengeValue(weeklyChallenge.targetValue) }}
              </p>
            </div>
          </div>
        </div>
      </div>

      <!-- Stats cards -->
      <div class="grid grid-cols-4 gap-3 mb-8">
        <div
          :ref="
            (el: any) =>
              registerContent(el, { onSelect: () => (activeTab = 'players') })
          "
          class="bg-zinc-900/60 rounded-xl p-4 cursor-pointer hover:bg-zinc-800/60 transition-colors"
          @click="activeTab = 'players'"
        >
          <p class="text-2xl font-bold text-zinc-100">
            {{ stats.totalUsers.toLocaleString() }}
          </p>
          <p class="text-xs text-blue-400 mt-1">Players &rarr;</p>
        </div>
        <div class="bg-zinc-900/60 rounded-xl p-4">
          <p class="text-2xl font-bold text-zinc-100">
            {{ stats.totalGames.toLocaleString() }}
          </p>
          <p class="text-xs text-zinc-500 mt-1">Games</p>
        </div>
        <div class="bg-zinc-900/60 rounded-xl p-4">
          <p class="text-2xl font-bold text-zinc-100">
            {{ stats.totalPlaytimeHours.toLocaleString() }}h
          </p>
          <p class="text-xs text-zinc-500 mt-1">Total Playtime</p>
        </div>
        <div class="bg-zinc-900/60 rounded-xl p-4">
          <p class="text-2xl font-bold text-zinc-100">
            {{ stats.totalAchievementUnlocks.toLocaleString() }}
          </p>
          <p class="text-xs text-zinc-500 mt-1">Achievements Unlocked</p>
        </div>
      </div>

      <!-- Around right now strip -->
      <div v-if="nowPlaying.length > 0" class="mb-8">
        <div class="flex items-baseline gap-2 mb-3">
          <h2 class="text-base font-display font-semibold text-zinc-100">
            Around right now
          </h2>
          <span class="relative flex size-2.5">
            <span
              class="animate-ping absolute inline-flex h-full w-full rounded-full bg-emerald-400 opacity-75"
            />
            <span
              class="relative inline-flex rounded-full size-2.5 bg-emerald-500"
            />
          </span>
          <span class="text-xs text-zinc-500"
            >{{ nowPlaying.length }}
            {{ nowPlaying.length === 1 ? "player" : "players" }}</span
          >
        </div>
        <div
          class="flex gap-3 overflow-x-auto pb-2"
          style="scrollbar-width: thin"
        >
          <button
            v-for="entry in nowPlaying"
            :key="`${entry.userId}-${entry.startedAt}`"
            :ref="
              (el: any) =>
                registerContent(el, { onSelect: () => goToGame(entry.game.id) })
            "
            class="shrink-0 group flex items-center gap-3 rounded-xl bg-emerald-500/5 ring-1 ring-emerald-500/20 hover:ring-emerald-400/50 transition-all p-2.5 pr-4"
            @click="goToGame(entry.game.id)"
          >
            <div class="relative shrink-0">
              <img
                v-if="entry.avatarObjectId"
                :src="objectUrl(entry.avatarObjectId)"
                class="size-10 rounded-full object-cover ring-2 ring-emerald-400/60"
              />
              <div
                v-else
                class="size-10 rounded-full bg-zinc-700 flex items-center justify-center ring-2 ring-emerald-400/60"
              >
                <span class="text-xs font-bold text-zinc-400">{{
                  entry.displayName[0]?.toUpperCase()
                }}</span>
              </div>
              <span
                class="absolute -bottom-0.5 -right-0.5 size-3 rounded-full bg-emerald-500 ring-2 ring-zinc-950"
              />
            </div>
            <img
              v-if="entry.game.coverObjectId"
              :src="objectUrl(entry.game.coverObjectId)"
              class="h-12 w-9 rounded object-cover shrink-0"
              loading="lazy"
            />
            <div class="text-left min-w-0 max-w-[10rem]">
              <p class="text-xs text-zinc-400 truncate">
                {{ entry.displayName }}
              </p>
              <p
                class="text-sm font-medium text-zinc-100 truncate group-hover:text-emerald-300 transition-colors"
              >
                {{ entry.game.name }}
              </p>
            </div>
          </button>
        </div>
      </div>

      <!-- Game roulette card -->
      <div
        :ref="
          (el: any) => registerContent(el, { onSelect: onRouletteCardSelect })
        "
        class="relative overflow-hidden rounded-2xl bg-gradient-to-br from-fuchsia-900/30 via-zinc-900/60 to-cyan-900/30 ring-1 ring-fuchsia-500/20 mb-8 px-5 py-4 cursor-pointer"
        @click="onRouletteCardSelect"
      >
        <div class="flex items-center gap-2 mb-3">
          <span
            class="text-[10px] tracking-[0.2em] uppercase text-fuchsia-300/80 font-medium"
          >
            Game roulette
          </span>
          <span class="text-[10px] text-zinc-500"
            >Can't decide? Let fate pick.</span
          >
        </div>
        <div class="flex items-center gap-4">
          <div
            class="relative shrink-0 size-20 rounded-xl overflow-hidden bg-zinc-800 ring-1 ring-fuchsia-500/30"
          >
            <img
              v-if="rouletteCoverId"
              :src="objectUrl(rouletteCoverId)"
              class="w-full h-full object-cover"
              :class="{
                'opacity-90': rouletteSpinning,
                'roulette-tada': rouletteSettled,
              }"
              loading="lazy"
            />
            <div
              v-else
              class="w-full h-full flex items-center justify-center text-zinc-600 text-2xl"
            >
              ✨
            </div>
          </div>
          <div class="flex-1 min-w-0">
            <template v-if="rouletteResult">
              <p
                class="text-base font-display font-semibold text-zinc-100 truncate"
              >
                {{ rouletteResult.game.name }}
              </p>
              <p class="text-xs text-fuchsia-300 mt-0.5">
                {{ rouletteCaption }}
              </p>
            </template>
            <p v-else-if="rouletteEmpty" class="text-sm text-zinc-400">
              Nothing to spin up yet — install a game first.
            </p>
            <p v-else-if="rouletteSpinning" class="text-sm text-zinc-400">
              Spinning...
            </p>
            <p v-else class="text-sm text-zinc-300">Press A / click to spin.</p>
          </div>
        </div>
      </div>

      <!-- Activity type filter -->
      <div class="flex gap-2 mb-4">
        <button
          v-for="filter in activityFilters"
          :key="filter.value"
          :ref="
            (el: any) =>
              registerTab(el, {
                onSelect: () => (activityFilter = filter.value),
              })
          "
          class="px-3 py-1 rounded-full text-xs font-medium transition-colors"
          :class="
            activityFilter === filter.value
              ? 'bg-blue-600/20 text-blue-400'
              : 'bg-zinc-800 text-zinc-400 hover:bg-zinc-700'
          "
          @click="activityFilter = filter.value"
        >
          {{ filter.label }}
        </button>
      </div>

      <!-- Activity feed (clustered) -->
      <div class="space-y-2">
        <div
          v-for="cluster in clusteredActivity"
          :key="cluster.key"
          :ref="
            (el: any) =>
              registerContent(el, {
                onSelect: () => goToGame(cluster.game?.id),
              })
          "
          class="flex items-center gap-4 bg-zinc-900/40 rounded-xl p-4"
        >
          <div
            class="size-10 rounded-full bg-zinc-800 flex-shrink-0 overflow-hidden"
          >
            <img
              v-if="cluster.user.profilePictureObjectId"
              :src="objectUrl(cluster.user.profilePictureObjectId)"
              class="w-full h-full object-cover"
            />
            <div
              v-else
              class="w-full h-full flex items-center justify-center text-zinc-500 text-sm font-bold"
            >
              {{
                (cluster.user.displayName || cluster.user.username || "?")[0]
              }}
            </div>
          </div>

          <div class="flex-1 min-w-0">
            <!--
              Same fix as the desktop row: bare `<template v-if>` text was
              getting stripped of surrounding whitespace, mashing words
              together. The verb segment is now a `<span>` wrapper with
              hardcoded `&nbsp;` padding so the spaces sit inside a text
              node (where Vue's compiler always preserves them). Verb copy
              "was in" → "was playing" because "was in <game>" reads like
              a location.
            -->
            <p class="text-sm text-zinc-200">
              <span class="font-medium">{{
                cluster.user.displayName || cluster.user.username
              }}</span>
              <span
                >&nbsp;{{
                  cluster.kind === "request"
                    ? "requested"
                    : cluster.totalDuration && cluster.totalDuration > 0
                      ? "played"
                      : cluster.achievements.length > 0
                        ? "unlocked achievements in"
                        : "was playing"
                }}&nbsp;</span
              >
              <span
                v-if="cluster.kind === 'request' && cluster.request"
                class="text-purple-400"
                >{{ cluster.request.title }}</span
              >
              <span v-else-if="cluster.game" class="text-blue-400">{{
                cluster.game.mName
              }}</span>
              <span
                v-if="
                  cluster.kind !== 'request' &&
                  cluster.totalDuration &&
                  cluster.totalDuration > 0
                "
                class="text-zinc-500"
                >&nbsp;for {{ formatDuration(cluster.totalDuration) }}</span
              >
            </p>
            <p
              v-if="cluster.achievements.length > 0"
              class="text-xs text-zinc-400 mt-1"
            >
              <span class="text-yellow-500 mr-1">unlocked</span>
              <template
                v-for="(ach, i) in cluster.achievements.slice(0, 3)"
                :key="ach.id"
              >
                <span class="text-yellow-300">{{ ach.title }}</span>
                <span
                  v-if="i < Math.min(cluster.achievements.length, 3) - 1"
                  class="text-zinc-500"
                  >,
                </span>
              </template>
              <span v-if="cluster.achievements.length > 3" class="text-zinc-500"
                >, +{{ cluster.achievements.length - 3 }} more</span
              >
            </p>
            <p class="text-xs text-zinc-600 mt-0.5">
              {{ formatTimeAgo(cluster.timestamp) }}
            </p>
          </div>

          <div
            v-if="cluster.game?.mCoverObjectId && cluster.kind !== 'request'"
            class="rounded-lg overflow-hidden flex-shrink-0 bg-zinc-800"
          >
            <img
              :src="objectUrl(cluster.game.mCoverObjectId)"
              class="h-20 w-[3.75rem] object-cover"
              loading="lazy"
            />
          </div>
        </div>
      </div>

      <div v-if="activity.length >= 30" class="flex justify-center py-6">
        <button
          :ref="
            (el: any) => registerContent(el, { onSelect: loadMoreActivity })
          "
          class="px-6 py-2 rounded-lg bg-zinc-800 text-zinc-300 text-sm font-medium hover:bg-zinc-700 transition-colors"
          @click="loadMoreActivity"
        >
          Load More
        </button>
      </div>
    </div>

    <!-- ═══ Players tab ═══ -->
    <div
      v-else-if="activeTab === 'players'"
      class="flex-1 overflow-y-auto px-8 py-6"
    >
      <div class="space-y-2">
        <div
          v-for="entry in leaderboard"
          :key="entry.user.id"
          :ref="
            (el: any) =>
              registerContent(el, {
                onSelect: () => viewProfile(entry.user.id),
              })
          "
          class="flex items-center gap-4 bg-zinc-900/40 rounded-xl p-4 cursor-pointer"
        >
          <div
            class="size-12 rounded-full bg-zinc-800 flex-shrink-0 overflow-hidden"
          >
            <img
              v-if="entry.user.profilePictureObjectId"
              :src="objectUrl(entry.user.profilePictureObjectId)"
              class="w-full h-full object-cover"
            />
            <div
              v-else
              class="w-full h-full flex items-center justify-center text-zinc-500 text-lg font-bold"
            >
              {{ entry.user.displayName[0] }}
            </div>
          </div>

          <div class="flex-1 min-w-0">
            <p
              class="text-sm font-medium text-zinc-200 truncate flex items-center gap-1"
            >
              <span class="truncate">{{ entry.user.displayName }}</span>
              <span
                v-if="mvp && entry.user.id === mvp.userId"
                :title="mvpTooltip"
                class="text-yellow-400 shrink-0"
                aria-label="Today's MVP"
                >👑</span
              >
            </p>
            <p class="text-xs text-zinc-600">@{{ entry.user.username }}</p>
          </div>

          <div class="flex gap-6 text-right">
            <div>
              <p class="text-sm font-medium text-zinc-200">
                {{ entry.playtimeHours.toLocaleString() }}h
              </p>
              <p class="text-xs text-zinc-600">Playtime</p>
            </div>
            <div>
              <p class="text-sm font-medium text-zinc-200">
                {{ entry.gamesPlayed }}
              </p>
              <p class="text-xs text-zinc-600">Games</p>
            </div>
            <div>
              <p class="text-sm font-medium text-zinc-200">
                {{ entry.achievements }}
              </p>
              <p class="text-xs text-zinc-600">Achievements</p>
            </div>
          </div>
        </div>

        <p
          v-if="leaderboard.length === 0"
          class="text-zinc-500 text-center py-12 text-sm"
        >
          No players found.
        </p>
      </div>
    </div>

    <!-- ═══ Leaderboard tab ═══ -->
    <div
      v-else-if="activeTab === 'leaderboard'"
      class="flex-1 overflow-y-auto px-8 py-6"
    >
      <div class="space-y-2">
        <div
          v-for="entry in leaderboard"
          :key="entry.user.id"
          :ref="
            (el: any) =>
              registerContent(el, {
                onSelect: () => viewProfile(entry.user.id),
              })
          "
          class="flex items-center gap-4 bg-zinc-900/40 rounded-xl p-4 cursor-pointer"
        >
          <div class="w-8 text-center flex-shrink-0">
            <span
              class="text-lg font-bold"
              :class="
                entry.rank <= 3 ? rankColors[entry.rank - 1] : 'text-zinc-500'
              "
            >
              {{ entry.rank }}
            </span>
          </div>

          <div
            class="size-10 rounded-full bg-zinc-800 flex-shrink-0 overflow-hidden"
          >
            <img
              v-if="entry.user.profilePictureObjectId"
              :src="objectUrl(entry.user.profilePictureObjectId)"
              class="w-full h-full object-cover"
            />
            <div
              v-else
              class="w-full h-full flex items-center justify-center text-zinc-500 text-sm font-bold"
            >
              {{ entry.user.displayName[0] }}
            </div>
          </div>

          <div class="flex-1 min-w-0">
            <p
              class="text-sm font-medium text-zinc-200 truncate flex items-center gap-1"
            >
              <span class="truncate">{{ entry.user.displayName }}</span>
              <span
                v-if="mvp && entry.user.id === mvp.userId"
                :title="mvpTooltip"
                class="text-yellow-400 shrink-0"
                aria-label="Today's MVP"
                >👑</span
              >
            </p>
            <p class="text-xs text-zinc-600">@{{ entry.user.username }}</p>
          </div>

          <div class="flex gap-6 text-right">
            <div>
              <p class="text-sm font-medium text-zinc-200">
                {{ entry.playtimeHours.toLocaleString() }}h
              </p>
              <p class="text-xs text-zinc-600">Playtime</p>
            </div>
            <div>
              <p class="text-sm font-medium text-zinc-200">
                {{ entry.gamesPlayed }}
              </p>
              <p class="text-xs text-zinc-600">Games</p>
            </div>
            <div>
              <p class="text-sm font-medium text-zinc-200">
                {{ entry.achievements }}
              </p>
              <p class="text-xs text-zinc-600">Achievements</p>
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- ═══ Shared Shelves tab ═══ -->
    <div
      v-else-if="activeTab === 'shelves'"
      class="flex-1 overflow-y-auto px-8 py-6"
    >
      <div v-if="sharedShelves.length > 0" class="space-y-8">
        <div v-for="shelf in sharedShelves" :key="shelf.id">
          <div class="flex items-center gap-3 mb-3">
            <div
              class="size-8 rounded-full bg-zinc-800 overflow-hidden flex-shrink-0"
            >
              <img
                v-if="shelf.user.profilePictureObjectId"
                :src="objectUrl(shelf.user.profilePictureObjectId)"
                class="w-full h-full object-cover"
              />
              <div
                v-else
                class="w-full h-full flex items-center justify-center text-zinc-500 text-sm font-bold"
              >
                {{ shelf.user.displayName[0] }}
              </div>
            </div>
            <div>
              <h3
                class="text-base font-semibold"
                style="color: var(--bpm-text)"
              >
                {{ shelf.name }}
              </h3>
              <p class="text-xs" style="color: var(--bpm-muted)">
                by {{ shelf.user.displayName }} &middot;
                {{ shelf.entries.length }} game{{
                  shelf.entries.length !== 1 ? "s" : ""
                }}
              </p>
            </div>
          </div>
          <div
            class="flex gap-4 overflow-x-auto pb-3 px-1"
            style="scrollbar-width: thin"
          >
            <div
              v-for="entry in shelf.entries"
              :key="entry.gameId"
              class="flex-shrink-0"
              style="width: 9rem"
              :ref="
                (el: any) =>
                  registerContent(el, {
                    onSelect: () => goToGame(entry.gameId),
                  })
              "
            >
              <div
                class="rounded-lg overflow-hidden cursor-pointer transition-transform hover:scale-105"
                style="aspect-ratio: 3/4"
              >
                <img
                  v-if="entry.game.mCoverObjectId"
                  :src="objectUrl(entry.game.mCoverObjectId)"
                  class="w-full h-full object-cover"
                  loading="lazy"
                />
                <div
                  v-else
                  class="w-full h-full flex items-center justify-center text-lg font-bold"
                  style="
                    background-color: var(--bpm-surface);
                    color: var(--bpm-accent-hex);
                  "
                >
                  {{ entry.game.mName[0] }}
                </div>
              </div>
              <p class="text-xs mt-1 truncate" style="color: var(--bpm-text)">
                {{ entry.game.mName }}
              </p>
            </div>
          </div>
        </div>
      </div>
      <div v-else class="flex items-center justify-center py-24">
        <div class="text-center">
          <h3 class="text-xl font-semibold mb-2" style="color: var(--bpm-text)">
            No shared shelves yet
          </h3>
          <p class="text-sm" style="color: var(--bpm-muted)">
            Make a shelf public from the Library &rarr; Shelves tab to share it
            with the community
          </p>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import {
  useServerApi,
  type CommunityStats,
  type CommunityActivityItem,
  type LeaderboardUser,
  type NowPlayingEntry,
  type WeeklyRecapSlide,
  type MvpToday,
  type TimeMachineEvent,
  type WeeklyChallenge,
  type RouletteResult,
} from "~/composables/use-server-api";
import { serverUrl } from "~/composables/use-server-fetch";
import { useBpFocusableGroup } from "~/composables/bp-focusable";
import { useFocusNavigation } from "~/composables/focus-navigation";
import { clusterActivity } from "~/composables/use-community-clusters";

definePageMeta({ layout: "bigpicture" });

const api = useServerApi();
const router = useRouter();
const focusNav = useFocusNavigation();

const registerTab = useBpFocusableGroup("content");
const registerContent = useBpFocusableGroup("content");

const loading = ref(true);
const activeTab = ref("activity");
const activityFilter = ref("all");

const stats = ref<CommunityStats>({
  totalGames: 0,
  totalUsers: 0,
  totalPlaytimeHours: 0,
  totalPlaySessions: 0,
  totalAchievementUnlocks: 0,
  totalRequests: 0,
  pendingRequests: 0,
  totalLeaderboardEntries: 0,
});
const activity = ref<CommunityActivityItem[]>([]);
const leaderboard = ref<LeaderboardUser[]>([]);
const nowPlaying = ref<NowPlayingEntry[]>([]);
const weeklyRecap = ref<WeeklyRecapSlide[]>([]);
const mvp = ref<MvpToday | null>(null);
const timeMachine = ref<TimeMachineEvent | null>(null);
const weeklyChallenge = ref<WeeklyChallenge | null>(null);
const recapIndex = ref(0);

// ── Roulette state (BPM-side, hand-rolled so we don't need to register the
// shared component in this focusable grid).
const rouletteResult = ref<RouletteResult | null>(null);
const rouletteSpinning = ref(false);
const rouletteEmpty = ref(false);
const rouletteSettled = ref(false);
const roulettePreviewCoverId = ref<string | null>(null);
let rouletteCycleTimer: ReturnType<typeof setInterval> | null = null;
let rouletteTadaTimer: ReturnType<typeof setTimeout> | null = null;

const rouletteCoverId = computed(() => {
  if (rouletteSpinning.value) return roulettePreviewCoverId.value;
  return rouletteResult.value?.game.coverObjectId ?? null;
});

const rouletteCaption = computed(() => {
  if (!rouletteResult.value) return "";
  switch (rouletteResult.value.source) {
    case "rediscovery":
      return "You haven't played in a while";
    case "library":
      return "From your library";
    case "social": {
      const n = rouletteResult.value.alsoPlayedBy?.length ?? 0;
      if (n === 0) return "Played by others on this server";
      if (n === 1)
        return `${rouletteResult.value.alsoPlayedBy![0].displayName} has played this`;
      return `${n} friends have played this`;
    }
    default:
      return "";
  }
});

const rouletteCoverPool = computed(() => {
  const pool = new Set<string>();
  for (const a of activity.value) {
    if (a.game?.mCoverObjectId) pool.add(a.game.mCoverObjectId);
  }
  for (const n of nowPlaying.value) {
    if (n.game?.coverObjectId) pool.add(n.game.coverObjectId);
  }
  return [...pool].slice(0, 40);
});

function pickRouletteCover(): string | null {
  const pool = rouletteCoverPool.value;
  if (pool.length === 0) return roulettePreviewCoverId.value;
  return pool[Math.floor(Math.random() * pool.length)] ?? null;
}

async function spinRoulette() {
  if (rouletteSpinning.value) return;
  rouletteEmpty.value = false;
  rouletteSettled.value = false;
  rouletteSpinning.value = true;

  const cycleMs = 80;
  const minCycles = 8;
  const maxCycles = 12;
  const targetCycles =
    minCycles + Math.floor(Math.random() * (maxCycles - minCycles + 1));
  const minDurationMs = targetCycles * cycleMs;

  if (rouletteCycleTimer) clearInterval(rouletteCycleTimer);
  rouletteCycleTimer = setInterval(() => {
    roulettePreviewCoverId.value = pickRouletteCover();
  }, cycleMs);

  const [fetched] = await Promise.all([
    api.community.roulette().catch((e) => {
      console.warn("[bpm roulette] fetch failed:", e);
      return null;
    }),
    new Promise((r) => setTimeout(r, minDurationMs)),
  ]);

  if (rouletteCycleTimer) {
    clearInterval(rouletteCycleTimer);
    rouletteCycleTimer = null;
  }

  rouletteSpinning.value = false;
  if (fetched) {
    rouletteResult.value = fetched;
    rouletteEmpty.value = false;
    rouletteSettled.value = true;
    if (rouletteTadaTimer) clearTimeout(rouletteTadaTimer);
    rouletteTadaTimer = setTimeout(() => {
      rouletteSettled.value = false;
    }, 700);
  } else {
    rouletteResult.value = null;
    rouletteEmpty.value = true;
  }
}

/**
 * Card-level activation. On first press (no result yet, not spinning) we
 * kick off a spin. Subsequent presses while we have a settled pick navigate
 * to its destination — owned games go to the BPM library, social picks to
 * the BPM store.
 */
function onRouletteCardSelect() {
  if (rouletteSpinning.value) return;
  if (!rouletteResult.value) {
    spinRoulette();
    return;
  }
  // BPM's library page handles both owned (installed) and store-discovery
  // states for the same game id, so we can route both source kinds to the
  // same route — same pattern the BPM store uses to "drill in" to a tile.
  const gameId = rouletteResult.value.game.id;
  const target = `/bigpicture/library/${gameId}`;
  focusNav.setRouteState("backTo", "/bigpicture/community", target);
  router.push(target);
}

const weeklyChallengeDaysSuffix = computed(() => {
  if (!weeklyChallenge.value) return "";
  const d = weeklyChallenge.value.daysRemaining;
  if (d <= 0) return "ends today";
  if (d === 1) return "1 day left";
  return `${d} days left`;
});

function formatChallengeValue(n: number): string {
  if (!weeklyChallenge.value) return String(n);
  // `play_hours` and `genre_focus` measure whole hours; the other seven
  // kinds are raw counts (including binary 0/1 quests).
  const isHours =
    weeklyChallenge.value.kind === "play_hours" ||
    weeklyChallenge.value.kind === "genre_focus";
  return isHours ? `${n}h` : n.toLocaleString();
}

// Per-kind emoji for the BPM card — mirrors the heroicon mapping in
// `CommunityWeeklyChallenge.vue` (the desktop variant). Kept inline rather
// than imported so BPM doesn't have to drag in the heroicons set just for
// this card.
const weeklyChallengeEmoji = computed(() => {
  if (!weeklyChallenge.value) return "⏱";
  switch (weeklyChallenge.value.kind) {
    case "play_hours":
      return "⏱";
    case "unlock_count":
      return "🏆";
    case "play_variety":
      return "🎲";
    case "rediscover":
      return "⏳";
    case "marathon":
      return "🏃";
    case "night_owl":
      return "🌙";
    case "new_to_you":
      return "✨";
    case "genre_focus":
      return "🏷";
    case "fresh_drop":
      return "🌱";
    default:
      return "⏱";
  }
});

const mvpTooltip = computed(() => {
  if (!mvp.value) return "";
  const hours = Math.max(1, Math.round(mvp.value.sessionSeconds / 3600));
  const playLabel =
    mvp.value.sessionSeconds === 0 ? "no playtime" : `${hours}h playtime`;
  return `Today's MVP — ${playLabel} · ${mvp.value.achievementsUnlocked} achievement${mvp.value.achievementsUnlocked === 1 ? "" : "s"}`;
});

const timeMachineLabel = computed(() => {
  if (!timeMachine.value) return "";
  switch (timeMachine.value.daysAgo) {
    case 365:
      return "A year ago today";
    case 180:
      return "Six months ago today";
    case 90:
      return "Three months ago today";
    case 30:
      return "One month ago today";
    default:
      return `${timeMachine.value.daysAgo} days ago today`;
  }
});
let recapTimer: ReturnType<typeof setInterval> | null = null;

const rankColors = ["text-yellow-400", "text-zinc-300", "text-amber-600"];

const tabs = [
  { label: "Activity", value: "activity" },
  { label: "Players", value: "players" },
  { label: "Leaderboard", value: "leaderboard" },
  { label: "Shared Shelves", value: "shelves" },
];

const activityFilters = [
  { label: "All", value: "all" },
  { label: "Sessions", value: "session" },
  { label: "Achievements", value: "achievement" },
  { label: "Requests", value: "request" },
];

const filteredActivity = computed(() => {
  if (activityFilter.value === "all") return activity.value;
  return activity.value.filter((a) => a.type === activityFilter.value);
});

const clusteredActivity = computed(() =>
  clusterActivity(filteredActivity.value),
);

const activeRecapSlide = computed(
  () => weeklyRecap.value[recapIndex.value] ?? weeklyRecap.value[0],
);

function onRecapSelect() {
  const slide = activeRecapSlide.value;
  if (!slide) return;
  if (slide.gameId) goToGame(slide.gameId);
  else if (slide.userId) viewProfile(slide.userId);
}

function objectUrl(id: string): string {
  return serverUrl(`api/v1/object/${id}`);
}

function goToGame(gameId?: string) {
  if (!gameId) return;
  const target = `/bigpicture/library/${gameId}`;
  focusNav.setRouteState("backTo", "/bigpicture/community", target);
  router.push(target);
}

function viewProfile(userId: string) {
  focusNav.saveFocusSnapshot("/bigpicture/community");
  router.push(`/bigpicture/profile/${userId}`);
}

function formatDuration(seconds: number): string {
  if (seconds < 60) return `${seconds}s`;
  if (seconds < 3600) return `${Math.round(seconds / 60)}m`;
  const hours = Math.floor(seconds / 3600);
  const mins = Math.round((seconds % 3600) / 60);
  return mins > 0 ? `${hours}h ${mins}m` : `${hours}h`;
}

function formatTimeAgo(timestamp: string): string {
  const diff = Date.now() - new Date(timestamp).getTime();
  const minutes = Math.floor(diff / 60000);
  if (minutes < 1) return "just now";
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.floor(hours / 24);
  if (days < 7) return `${days}d ago`;
  return `${Math.floor(days / 7)}w ago`;
}

async function loadMoreActivity() {
  if (activity.value.length === 0) return;
  const lastTimestamp = activity.value[activity.value.length - 1].timestamp;
  const more = await api.community.activity(30, lastTimestamp);
  activity.value.push(...more);
}

// Shared shelves data
interface SharedShelf {
  id: string;
  name: string;
  user: {
    id: string;
    username: string;
    displayName: string;
    profilePictureObjectId: string | null;
  };
  entries: Array<{
    gameId: string;
    game: {
      id: string;
      mName: string;
      mCoverObjectId: string | null;
      mIconObjectId: string | null;
    };
  }>;
}
const sharedShelves = ref<SharedShelf[]>([]);

function startRecapTimer() {
  if (recapTimer) clearInterval(recapTimer);
  if (weeklyRecap.value.length > 1) {
    recapTimer = setInterval(() => {
      recapIndex.value = (recapIndex.value + 1) % weeklyRecap.value.length;
    }, 8000);
  }
}

onMounted(async () => {
  try {
    const [
      statsData,
      activityData,
      leaderboardData,
      shelvesData,
      nowPlayingData,
      recapData,
      mvpData,
      timeMachineData,
      weeklyChallengeData,
    ] = await Promise.all([
      api.community.stats().catch(() => stats.value),
      api.community.activity().catch(() => []),
      api.community.leaderboard().catch(() => ({ playtime: [] })),
      fetch(serverUrl("api/v1/community/shelves"))
        .then((r) => (r.ok ? r.json() : []))
        .catch(() => []),
      api.community.nowPlaying().catch(() => []),
      api.community.weeklyRecap().catch(() => []),
      api.community.mvpToday().catch(() => null),
      api.community.timeMachine().catch(() => null),
      api.community.weeklyChallenge().catch(() => null),
    ]);
    stats.value = statsData;
    activity.value = activityData;
    leaderboard.value = leaderboardData.playtime;
    sharedShelves.value = shelvesData;
    nowPlaying.value = nowPlayingData;
    weeklyRecap.value = recapData;
    mvp.value = mvpData;
    timeMachine.value = timeMachineData;
    weeklyChallenge.value = weeklyChallengeData;
    startRecapTimer();
  } catch (e) {
    console.error("Failed to load community data:", e);
  } finally {
    loading.value = false;
    nextTick(() => focusNav.autoFocusContent("content"));
  }
});

onUnmounted(() => {
  if (recapTimer) clearInterval(recapTimer);
  if (rouletteCycleTimer) clearInterval(rouletteCycleTimer);
  if (rouletteTadaTimer) clearTimeout(rouletteTadaTimer);
});

watch(
  () => weeklyRecap.value.length,
  () => {
    recapIndex.value = 0;
    startRecapTimer();
  },
);
</script>

<style scoped>
.bpm-slide-fade-enter-active,
.bpm-slide-fade-leave-active {
  transition:
    opacity 350ms ease,
    transform 350ms ease;
}
.bpm-slide-fade-enter-from {
  opacity: 0;
  transform: translateY(6px);
}
.bpm-slide-fade-leave-to {
  opacity: 0;
  transform: translateY(-6px);
}

@keyframes roulette-tada {
  0% {
    transform: scale(1);
  }
  30% {
    transform: scale(1.08);
  }
  60% {
    transform: scale(0.97);
  }
  100% {
    transform: scale(1);
  }
}

.roulette-tada {
  animation: roulette-tada 700ms cubic-bezier(0.34, 1.56, 0.64, 1);
}
</style>
