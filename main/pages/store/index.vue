<template>
  <!-- Wide outer container — keeps horizontal padding on ultrawide
       displays. The header + Featured-tab content are individually
       capped to 1600px inside; Browse is intentionally left
       full-width because its tile grid wants the screen real estate. -->
  <div class="w-full px-10 xl:px-14 py-6">
    <!-- Header column — capped to the same 1600px as the Featured
         body so the title / tabs / search bar align with the hero
         and cards instead of stretching across the full viewport. -->
    <div class="mx-auto max-w-[1600px]">
      <!-- Header — title + tabs -->
      <div class="mb-6">
        <h1 class="text-2xl font-display font-bold text-zinc-100">Store</h1>
        <p class="mt-1 text-sm text-zinc-400">
          Browse and add games to your library.
        </p>
      </div>

      <!-- Top bar: tab nav + search/select toggle -->
      <div class="flex items-center gap-2 mb-6 border-b border-zinc-700/50">
      <button
        v-for="tab in tabs"
        :key="tab.value"
        class="relative px-5 py-3 text-sm font-medium transition-colors"
        :class="
          activeTab === tab.value
            ? 'text-blue-400'
            : 'text-zinc-400 hover:text-zinc-200'
        "
        @click="activeTab = tab.value"
      >
        {{ tab.label }}
        <span
          v-if="activeTab === tab.value"
          class="absolute bottom-0 left-0 right-0 h-0.5 bg-blue-500 rounded-full"
        />
      </button>

      <div class="flex-1" />

      <!-- Search box — visible on both tabs, but only the Browse tab acts on
           it for filtered results. Submitting from Featured switches to
           Browse and runs the query. Clear-X appears once the user has
           typed something so they can wipe the field (and the active
           query) without selecting + deleting. -->
      <div class="relative">
        <MagnifyingGlassIcon
          class="absolute left-3 top-1/2 -translate-y-1/2 size-4 text-zinc-500 pointer-events-none"
        />
        <input
          v-model="searchInput"
          type="text"
          placeholder="Search the store..."
          class="rounded-lg border border-zinc-700 bg-zinc-800/50 pl-9 pr-9 py-2 text-sm text-zinc-100 placeholder:text-zinc-500 focus:bg-zinc-800 focus:ring-2 focus:ring-blue-500 outline-none transition-colors w-72"
          @keydown.enter="submitSearch"
          @keydown.escape="clearSearch"
        />
        <button
          v-if="searchInput.length > 0"
          class="absolute right-2 top-1/2 -translate-y-1/2 rounded p-1 text-zinc-500 hover:text-zinc-200 hover:bg-zinc-700/50 transition-colors"
          aria-label="Clear search"
          @click="clearSearch"
        >
          <XMarkIcon class="size-3.5" />
        </button>
      </div>
    </div>
    </div>

    <!-- Loading skeleton on first load only -->
    <div v-if="loading" class="space-y-8">
      <div class="aspect-[21/9] rounded-2xl bg-zinc-800/50 animate-pulse" />
      <GameTileGrid>
        <div
          v-for="i in 8"
          :key="i"
          class="aspect-[3/4] rounded-xl bg-zinc-800/50 animate-pulse"
        />
      </GameTileGrid>
    </div>

    <!-- ═══ Featured tab ═══ -->
    <!-- Cap the whole Featured column to a centred 1600px so the
         hero, Recently Updated cards, and Discovery Tabs all line up
         to the same column.  Without this, the Tabs section looked
         like a slim widget marooned in a wide page; with it, the
         three sections read as one designed surface. Browse tab is
         intentionally left full-width because its grid wants the
         screen real estate. -->
    <div
      v-else-if="activeTab === 'featured'"
      class="mx-auto max-w-[1600px]"
    >
      <!-- Hero carousel -->
      <section
        v-if="featured.length > 0"
        class="mb-12"
      >
        <div
          class="relative rounded-2xl overflow-hidden cursor-pointer group aspect-[21/9]"
          @click="goToGame(featured[heroIndex]?.id)"
        >
          <img
            v-if="featured[heroIndex]?.mBannerObjectId"
            :src="objectUrl(featured[heroIndex].mBannerObjectId)"
            :alt="featured[heroIndex].mName"
            class="w-full h-full object-cover"
          />
          <BannerFallback
            v-else-if="featured[heroIndex]"
            :name="featured[heroIndex].mName"
            text-size="text-8xl"
          />
          <!-- Gradient masks. Softened from the previous from-zinc-950/95
               so the banner art still reads through the lower third of
               the hero instead of being washed to near-solid black. -->
          <div
            class="absolute inset-0 bg-gradient-to-t from-zinc-950/85 via-zinc-950/30 to-transparent"
          />
          <div
            class="absolute inset-0 bg-gradient-to-r from-zinc-950/60 via-transparent to-transparent"
          />
          <div class="absolute bottom-0 inset-x-0 p-8 pr-32">
            <h2
              class="text-4xl font-display font-bold text-zinc-100 drop-shadow-lg mb-2"
            >
              {{ featured[heroIndex]?.mName }}
            </h2>
            <p
              class="text-sm text-zinc-200/90 line-clamp-2 max-w-3xl mb-3"
            >
              {{ featured[heroIndex]?.mShortDescription }}
            </p>
            <div
              v-if="featured[heroIndex]?.tags?.length"
              class="flex gap-2"
            >
              <span
                v-for="tag in featured[heroIndex].tags!.slice(0, 4)"
                :key="tag.id"
                class="px-2.5 py-0.5 rounded-full text-xs font-medium bg-blue-500/30 text-blue-200 backdrop-blur-sm"
              >
                {{ tag.name }}
              </span>
            </div>
          </div>

          <!-- X-of-Y indicator. Sits top-right so a user can tell at a
               glance how deep the carousel is without counting dots.
               Hidden on single-slide carousels (where there's nothing
               to count). -->
          <div
            v-if="featured.length > 1"
            class="absolute top-4 right-4 px-2.5 py-1 rounded-full bg-zinc-950/60 backdrop-blur-sm text-[11px] font-medium text-zinc-200 tabular-nums"
          >
            {{ heroIndex + 1 }} / {{ featured.length }}
          </div>

          <!-- Carousel dots. Manual nav resets the auto-rotate cadence
               so a click doesn't trigger an immediate swap to the next
               slide a beat later. -->
          <div
            v-if="featured.length > 1"
            class="absolute bottom-4 right-6 flex gap-1.5"
          >
            <button
              v-for="(_, i) in featured"
              :key="i"
              class="size-2 rounded-full transition-all"
              :class="
                i === heroIndex
                  ? 'bg-blue-500 w-5'
                  : 'bg-zinc-500/70 hover:bg-zinc-400'
              "
              :aria-label="`Slide ${i + 1}`"
              @click.stop="goToHeroSlide(i)"
            />
          </div>
        </div>
      </section>

      <!-- Recently Updated — Steam-style banner-card carousel.  Pulled
           from /api/v1/store with sort='updated' so the order reflects
           when each catalogue entry last changed. -->
      <StoreRecentlyUpdated
        v-if="recentlyUpdated.length > 0"
        :games="recentlyUpdated"
        :library-game-ids="libraryGameIdSet"
        @select="goToGame"
        @browse-all="goToBrowseUpdated"
      />

      <!-- Discovery tabs — New / Most Played / Random with a sticky
           preview panel on the right (Steam's "New & Trending" widget
           the user wanted us to match).  Per-tab data is fetched
           lazily on tab change so the initial Featured paint stays
           cheap. -->
      <StoreDiscoveryTabs
        :games="discoveryListGames"
        :loading="discoveryLoading"
        :tab="discoveryTab"
        :library-game-ids="libraryGameIdSet"
        @select="goToGame"
        @tab="onDiscoveryTabChange"
        @see-more="onDiscoverySeeMore"
      />

      <div
        v-if="
          featured.length === 0 &&
          recentlyUpdated.length === 0 &&
          discoveryListGames.length === 0
        "
        class="text-center text-zinc-500 py-20 text-sm"
      >
        No featured games yet. Try the Browse tab to see what's available.
      </div>
    </div>

    <!-- ═══ Browse tab ═══ -->
    <!-- Browse content now caps to the same 1600px column as the
         header and the Featured tab, so the visual width is
         consistent across the whole store.  Earlier iterations left
         Browse full-width to maximise tile count; the trade-off
         wasn't worth the inconsistency with the rest of the page. -->
    <div
      v-else-if="activeTab === 'browse'"
      class="mx-auto max-w-[1600px]"
    >
      <!-- "Can't decide?" banner — lives at the top of Browse so the user
           sees it before scrolling into the grid. Same component the
           community page used to host; cover pool is now built from
           browse + featured data instead of activity feeds. -->
      <GameRoulette
        class="mb-5"
        :cover-pool="rouletteCoverPool"
        @select="onRouletteSelect"
      />

      <!-- Filter / sort controls -->
      <div class="flex items-center gap-3 mb-3 flex-wrap text-sm">
        <span class="text-zinc-500">Sort:</span>
        <button
          v-for="opt in sortOptions"
          :key="opt.value"
          class="px-3 py-1.5 rounded-md text-xs font-medium transition-colors"
          :class="
            browseSort === opt.value
              ? 'bg-blue-600/20 text-blue-400 ring-1 ring-blue-500/40'
              : 'bg-zinc-800/50 text-zinc-400 hover:text-zinc-200 hover:bg-zinc-700/50'
          "
          @click="setSort(opt.value)"
        >
          {{ opt.label }}
        </button>

        <span class="mx-2 h-4 w-px bg-zinc-700" />

        <!-- Library / collection selector (server-side filter). -->
        <select
          v-if="libraries.length > 0"
          v-model="selectedLibraryId"
          class="rounded-md bg-zinc-800/50 text-zinc-200 px-2.5 py-1.5 text-xs font-medium border border-zinc-700/50 focus:ring-2 focus:ring-blue-500 outline-none"
          @change="loadBrowse(true)"
        >
          <option value="">All libraries</option>
          <option v-for="lib in libraries" :key="lib.id" :value="lib.id">
            {{ lib.name }}
          </option>
        </select>

        <!-- More filters drawer button — opens side panel with advanced
             knobs that don't fit in the chip row. -->
        <button
          class="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-md text-xs font-medium transition-colors"
          :class="
            activeAdvancedFilterCount > 0
              ? 'bg-blue-600/20 text-blue-400 ring-1 ring-blue-500/40'
              : 'bg-zinc-800/50 text-zinc-400 hover:text-zinc-200 hover:bg-zinc-700/50'
          "
          @click="filterDrawerOpen = true"
        >
          <AdjustmentsHorizontalIcon class="size-3" />
          Filters
          <span
            v-if="activeAdvancedFilterCount > 0"
            class="ml-1 rounded-full bg-blue-500 text-white text-[10px] font-bold px-1.5 leading-4"
          >
            {{ activeAdvancedFilterCount }}
          </span>
        </button>

        <span
          v-if="hasActiveFilters"
          class="text-xs text-zinc-500"
        >
          {{ displayedResults.length }} of {{ browseTotal }}
        </span>

        <div class="flex-1" />

        <button
          v-if="hasActiveFilters"
          class="text-xs text-zinc-500 hover:text-zinc-300 underline"
          @click="clearBrowseFilters"
        >
          Clear filters
        </button>
      </div>

      <!-- Active filter chips — quick-remove handles for whatever filters
           are on, including selected tags from the drawer. Keeps the
           drawer's state visible without having to re-open it. -->
      <div
        v-if="activeFilterChips.length > 0"
        class="flex flex-wrap gap-1.5 mb-5"
      >
        <button
          v-for="chip in activeFilterChips"
          :key="chip.key"
          class="inline-flex items-center gap-1 rounded-full bg-blue-500/15 text-blue-300 ring-1 ring-blue-500/30 px-2.5 py-1 text-xs hover:bg-blue-500/25 transition-colors"
          @click="removeFilterChip(chip)"
        >
          <span class="text-blue-400/70">{{ chip.label }}:</span>
          <span>{{ chip.value }}</span>
          <XMarkIcon class="size-3" />
        </button>
      </div>

      <!-- Grid -->
      <div v-if="browseLoading && browseResults.length === 0" class="text-zinc-500 text-sm py-10">
        Loading games...
      </div>
      <!-- Smart empty state. When the grid renders no results, advertise
           which filter is most likely the culprit so the user can drop
           it inline without playing whack-a-mole with the chip row. -->
      <div
        v-else-if="displayedResults.length === 0"
        class="py-20 text-center"
      >
        <p class="text-sm text-zinc-300 font-medium">
          No games match those filters.
        </p>
        <div
          v-if="emptyStateSuggestions.length > 0"
          class="mt-4 inline-flex flex-col items-center gap-2"
        >
          <p class="text-xs text-zinc-500">Try removing:</p>
          <div class="flex flex-wrap gap-1.5 justify-center max-w-md">
            <button
              v-for="sug in emptyStateSuggestions"
              :key="sug.key"
              class="inline-flex items-center gap-1 rounded-full bg-blue-500/15 text-blue-300 ring-1 ring-blue-500/30 px-2.5 py-1 text-xs hover:bg-blue-500/25 transition-colors"
              @click="sug.apply"
            >
              <span class="text-blue-400/70">{{ sug.label }}:</span>
              <span>{{ sug.value }}</span>
              <XMarkIcon class="size-3" />
            </button>
          </div>
        </div>
        <button
          v-else
          class="mt-3 text-xs text-blue-400 hover:text-blue-300 underline"
          @click="clearBrowseFilters"
        >
          Clear all filters
        </button>
      </div>
      <GameTileGrid v-else :item-count="displayedResults.length">
        <GameTile
          v-for="game in displayedResults"
          :key="game.id"
          :cover-url="game.mCoverObjectId ? objectUrl(game.mCoverObjectId) : null"
          :name="game.mName"
          :rom="game.isEmulated"
          :update-available="game.updateAvailable ?? false"
          @select="goToGame(game.id)"
        />
      </GameTileGrid>

      <!-- Pagination — classic numbered nav.  The Browse grid is now
           replaced wholesale each time the page changes (not appended
           to), so picking page 7 → page 3 → page 7 round-trips cleanly
           without doubling up rows.  Ellipsis collapses long page
           sequences so the bar stays readable past 10+ pages. -->
      <div
        v-if="browseTotalPages > 1"
        class="mt-8 flex flex-col items-center gap-2"
      >
        <div class="flex items-center gap-1 text-xs">
          <button
            class="rounded-md p-2 transition-colors"
            :class="
              browsePage > 1 && !browseLoading
                ? 'bg-zinc-800/50 text-zinc-300 hover:bg-zinc-800 hover:text-zinc-100'
                : 'bg-zinc-900/40 text-zinc-700 cursor-not-allowed'
            "
            :disabled="browsePage <= 1 || browseLoading"
            aria-label="Previous page"
            @click="goToPage(browsePage - 1)"
          >
            <ChevronLeftIcon class="size-4" />
          </button>

          <button
            v-for="(p, i) in browsePageNumbers"
            :key="`${p}-${i}`"
            class="min-w-[2rem] h-8 px-2 rounded-md text-xs font-semibold transition-colors tabular-nums"
            :class="
              p === '...'
                ? 'text-zinc-500 cursor-default'
                : p === browsePage
                  ? 'bg-blue-600 text-white'
                  : 'bg-zinc-800/50 text-zinc-300 hover:bg-zinc-800 hover:text-zinc-100'
            "
            :disabled="p === '...' || p === browsePage || browseLoading"
            @click="typeof p === 'number' && goToPage(p)"
          >
            {{ p }}
          </button>

          <button
            class="rounded-md p-2 transition-colors"
            :class="
              browsePage < browseTotalPages && !browseLoading
                ? 'bg-zinc-800/50 text-zinc-300 hover:bg-zinc-800 hover:text-zinc-100'
                : 'bg-zinc-900/40 text-zinc-700 cursor-not-allowed'
            "
            :disabled="browsePage >= browseTotalPages || browseLoading"
            aria-label="Next page"
            @click="goToPage(browsePage + 1)"
          >
            <ChevronRightIcon class="size-4" />
          </button>
        </div>
        <p class="text-xs text-zinc-500 tabular-nums">
          {{ browsePageStart }}–{{ browsePageEnd }} of {{ browseTotal }}
        </p>
      </div>
    </div>

    <!-- ═══ Collections tab ═══ -->
    <!-- Curated store collections as clickable cards. Each opens the native
         /store/collection/[id] page (games grid + "add entire collection"). -->
    <div
      v-else-if="activeTab === 'collections'"
      class="mx-auto max-w-[1600px]"
    >
      <div
        v-if="collectionsLoading && collections.length === 0"
        class="py-10 text-sm text-zinc-500"
      >
        Loading collections...
      </div>
      <div
        v-else-if="collections.length === 0"
        class="py-20 text-center text-sm text-zinc-500"
      >
        No collections yet. The store's curated collections will show up here.
      </div>
      <div
        v-else
        class="grid gap-5 grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4"
      >
        <button
          v-for="c in collections"
          :key="c.id"
          class="group relative overflow-hidden rounded-2xl bg-zinc-900 text-left ring-1 ring-zinc-800/60 transition-all hover:-translate-y-1 hover:ring-blue-500/40"
          @click="goToCollection(c.id)"
        >
          <div class="relative aspect-[16/9] overflow-hidden bg-zinc-800">
            <img
              v-if="c.coverObjectId"
              :src="objectUrl(c.coverObjectId)"
              :alt="c.name"
              class="h-full w-full object-cover transition-transform duration-300 group-hover:scale-105"
            />
            <div
              v-else
              class="flex h-full w-full items-center justify-center"
            >
              <RectangleStackIcon class="size-10 text-zinc-600" />
            </div>
            <div
              class="absolute inset-0 bg-gradient-to-t from-zinc-950/90 via-zinc-950/20 to-transparent"
            />
            <div class="absolute inset-x-0 bottom-0 p-4">
              <h3
                class="font-display text-lg font-bold text-zinc-100 drop-shadow"
              >
                {{ c.name }}
              </h3>
              <p class="mt-0.5 text-xs text-zinc-400">
                {{ c.gameCount }} game{{ c.gameCount === 1 ? "" : "s" }}
              </p>
            </div>
          </div>
          <p
            v-if="c.description"
            class="line-clamp-2 px-4 py-3 text-sm text-zinc-400"
          >
            {{ c.description }}
          </p>
        </button>
      </div>
    </div>

    <!-- Filter drawer — slides in from the right with advanced controls
         that don't belong in the chip row. Closed by default; opened via
         the "Filters" button. -->
    <Transition
      enter-active-class="transition-opacity duration-200"
      enter-from-class="opacity-0"
      enter-to-class="opacity-100"
      leave-active-class="transition-opacity duration-150"
      leave-from-class="opacity-100"
      leave-to-class="opacity-0"
    >
      <div
        v-if="filterDrawerOpen"
        class="fixed inset-0 bg-zinc-950/60 backdrop-blur-sm z-40"
        @click="filterDrawerOpen = false"
      />
    </Transition>
    <Transition
      enter-active-class="transition-transform duration-200 ease-out"
      enter-from-class="translate-x-full"
      enter-to-class="translate-x-0"
      leave-active-class="transition-transform duration-150 ease-in"
      leave-from-class="translate-x-0"
      leave-to-class="translate-x-full"
    >
      <aside
        v-if="filterDrawerOpen"
        class="fixed top-0 right-0 bottom-0 w-96 bg-zinc-900 border-l border-zinc-800 z-50 overflow-y-auto"
      >
        <div class="sticky top-0 bg-zinc-900 border-b border-zinc-800 px-5 py-4 flex items-center justify-between">
          <h3 class="text-base font-display font-semibold text-zinc-100">
            Filters
          </h3>
          <button
            class="rounded-md p-1.5 text-zinc-400 hover:bg-zinc-800 hover:text-zinc-100 transition-colors"
            @click="filterDrawerOpen = false"
          >
            <XMarkIcon class="size-5" />
          </button>
        </div>

        <div class="px-5 py-4 space-y-6 text-sm">
          <!-- Tags multi-select. Each tag toggles into the CSV the
               server side actually understands. -->
          <section v-if="allTags.length > 0">
            <h4 class="text-xs uppercase tracking-widest text-zinc-500 mb-2">
              Tags
            </h4>
            <input
              v-model="tagSearch"
              type="text"
              placeholder="Search tags..."
              class="w-full rounded-md border border-zinc-700 bg-zinc-800/50 px-3 py-1.5 text-xs text-zinc-100 placeholder:text-zinc-500 focus:ring-2 focus:ring-blue-500 outline-none mb-2"
            />
            <!-- Match mode — only meaningful with 2+ tags selected. "All
                 tags" sends tagMode=and (intersection); "Any tag" is the
                 default union. -->
            <div
              v-if="selectedTagIds.length >= 2"
              class="mb-2 flex items-center gap-2"
            >
              <span class="text-xs text-zinc-500">Match</span>
              <div
                class="inline-flex overflow-hidden rounded-md ring-1 ring-zinc-700"
              >
                <button
                  type="button"
                  class="px-2.5 py-1 text-xs transition-colors"
                  :class="
                    tagMode === 'and'
                      ? 'bg-blue-600 text-white'
                      : 'text-zinc-400 hover:bg-zinc-800'
                  "
                  @click="setTagMode('and')"
                >
                  All tags
                </button>
                <button
                  type="button"
                  class="px-2.5 py-1 text-xs transition-colors"
                  :class="
                    tagMode === 'or'
                      ? 'bg-blue-600 text-white'
                      : 'text-zinc-400 hover:bg-zinc-800'
                  "
                  @click="setTagMode('or')"
                >
                  Any tag
                </button>
              </div>
            </div>
            <div class="max-h-64 overflow-y-auto pr-1 space-y-1">
              <label
                v-for="tag in filteredTagList"
                :key="tag.id"
                class="flex items-center gap-2 px-2 py-1 rounded hover:bg-zinc-800/60 cursor-pointer"
              >
                <input
                  type="checkbox"
                  :checked="selectedTagIds.includes(tag.id)"
                  class="size-3.5 rounded bg-zinc-800 border-zinc-700 text-blue-500 focus:ring-blue-500 focus:ring-offset-0"
                  @change="toggleTag(tag.id)"
                />
                <span class="text-xs text-zinc-300">{{ tag.name }}</span>
              </label>
            </div>
          </section>

          <!-- Game type — emulated (ROM) vs native. Maps to client-side
               filter on `isEmulated`. -->
          <section>
            <h4 class="text-xs uppercase tracking-widest text-zinc-500 mb-2">
              Game type
            </h4>
            <div class="grid grid-cols-3 gap-1.5">
              <button
                v-for="opt in emulatedOptions"
                :key="opt.value"
                class="px-2 py-1.5 rounded-md text-xs font-medium transition-colors"
                :class="
                  emulatedFilter === opt.value
                    ? 'bg-blue-600 text-white'
                    : 'bg-zinc-800/50 text-zinc-400 hover:text-zinc-200'
                "
                @click="emulatedFilter = opt.value"
              >
                {{ opt.label }}
              </button>
            </div>
          </section>

          <!-- Release year range. Inclusive on both ends; blank = no
               constraint on that side. -->
          <section>
            <h4 class="text-xs uppercase tracking-widest text-zinc-500 mb-2">
              Release year
            </h4>
            <div class="flex items-center gap-2">
              <input
                v-model.number="releaseYearFrom"
                type="number"
                placeholder="From"
                class="w-full rounded-md border border-zinc-700 bg-zinc-800/50 px-2.5 py-1.5 text-xs text-zinc-100 placeholder:text-zinc-500 focus:ring-2 focus:ring-blue-500 outline-none"
              />
              <span class="text-zinc-500">–</span>
              <input
                v-model.number="releaseYearTo"
                type="number"
                placeholder="To"
                class="w-full rounded-md border border-zinc-700 bg-zinc-800/50 px-2.5 py-1.5 text-xs text-zinc-100 placeholder:text-zinc-500 focus:ring-2 focus:ring-blue-500 outline-none"
              />
            </div>
          </section>

          <!-- Platform — derived from the launchPlatform field on each
               StoreGame. The server already accepts `platform` as a query
               param, so this filter goes through the wire (not client
               post-filter). -->
          <section v-if="availablePlatforms.length > 0">
            <h4 class="text-xs uppercase tracking-widest text-zinc-500 mb-2">
              Platform
            </h4>
            <div class="grid grid-cols-2 gap-1.5">
              <button
                v-for="opt in availablePlatforms"
                :key="opt"
                class="px-2 py-1.5 rounded-md text-xs font-medium transition-colors"
                :class="
                  selectedPlatforms.includes(opt)
                    ? 'bg-blue-600 text-white'
                    : 'bg-zinc-800/50 text-zinc-400 hover:text-zinc-200'
                "
                @click="togglePlatform(opt)"
              >
                {{ opt }}
              </button>
            </div>
          </section>
        </div>

        <div class="sticky bottom-0 bg-zinc-900 border-t border-zinc-800 px-5 py-3 flex items-center gap-2">
          <button
            class="flex-1 rounded-md bg-zinc-800 px-3 py-2 text-xs font-semibold text-zinc-300 hover:bg-zinc-700 transition-colors"
            @click="clearBrowseFilters"
          >
            Reset
          </button>
          <button
            class="flex-1 rounded-md bg-blue-600 px-3 py-2 text-xs font-semibold text-white hover:bg-blue-500 transition-colors"
            @click="filterDrawerOpen = false"
          >
            Done
          </button>
        </div>
      </aside>
    </Transition>
  </div>
</template>

<script setup lang="ts">
import {
  MagnifyingGlassIcon,
  AdjustmentsHorizontalIcon,
  XMarkIcon,
  ChevronLeftIcon,
  ChevronRightIcon,
  RectangleStackIcon,
} from "@heroicons/vue/24/outline";
import {
  useServerApi,
  type StoreCollectionSummary,
  type StoreGame,
  type StoreTag,
  type TrendingGame,
} from "~/composables/use-server-api";
import { serverUrl } from "~/composables/use-server-fetch";
import { deduplicatedInvoke } from "~/composables/game";
import StoreShelf from "~/components/StoreShelf.vue";
import StoreRecentlyUpdated from "~/components/StoreRecentlyUpdated.vue";
import StoreDiscoveryTabs from "~/components/StoreDiscoveryTabs.vue";
import GameRoulette from "~/components/GameRoulette.vue";
import BannerFallback from "~/components/BannerFallback.vue";

// ── Browse-state restoration ─────────────────────────────────────────────
//
// Module-level snapshot of the user's last Browse-tab state. Survives
// component remount within a session so navigating into /store/{id}
// and back returns to the same tab + page + filter set the user came
// from. The snapshot is intentionally NOT persisted to disk — a full
// app restart resets to defaults, which matches "fresh session, fresh
// view" expectations.
//
// We do this with module state rather than route query params for two
// reasons: (1) we'd need to encode a dozen filter refs into the URL,
// which gets noisy; (2) `router.back()` from /store/{id} should land
// on the same `/store` URL it came from regardless of which filters
// were active — using query params would make every filter change
// push a new history entry, which makes the back button useless.
type StoredBrowseSnapshot = {
  activeTab: "featured" | "browse" | "collections";
  searchQuery: string;
  browseSort: "recent" | "updated" | "name";
  browsePage: number;
  selectedTagIds: string[];
  tagMode: "and" | "or";
  selectedLibraryId: string;
  selectedPlatforms: string[];
  emulatedFilter: "all" | "native" | "rom";
  releaseYearFrom: number | null;
  releaseYearTo: number | null;
};
// Cast away the literal-null narrowing TS would otherwise apply at
// every read site — assignments happen inside snapshotBrowseState()
// which TS treats as opaque, so reads at module top level would
// otherwise stay narrowed to `null` and the `?.` chains would type as
// `never`.
//
// Backed by sessionStorage as a second layer: HMR in dev, route
// pre-fetches, or any module re-evaluation can wipe the in-memory
// `let`, but sessionStorage survives those.  The session-storage
// fallback is loaded lazily on first access by `readBrowseSnapshot()`.
const BROWSE_SNAPSHOT_STORAGE_KEY = "drop.store.browseSnapshot";

let storedBrowseSnapshot = null as StoredBrowseSnapshot | null;

function readBrowseSnapshot(): StoredBrowseSnapshot | null {
  if (storedBrowseSnapshot) return storedBrowseSnapshot;
  if (typeof window === "undefined") return null;
  try {
    const raw = window.sessionStorage.getItem(BROWSE_SNAPSHOT_STORAGE_KEY);
    if (!raw) return null;
    storedBrowseSnapshot = JSON.parse(raw) as StoredBrowseSnapshot;
    return storedBrowseSnapshot;
  } catch {
    // Corrupt entry — wipe and start fresh.
    try {
      window.sessionStorage.removeItem(BROWSE_SNAPSHOT_STORAGE_KEY);
    } catch {
      // ignore
    }
    return null;
  }
}

function writeBrowseSnapshot(s: StoredBrowseSnapshot) {
  storedBrowseSnapshot = s;
  if (typeof window === "undefined") return;
  try {
    window.sessionStorage.setItem(
      BROWSE_SNAPSHOT_STORAGE_KEY,
      JSON.stringify(s),
    );
  } catch {
    // Quota / disabled storage / etc. — module state still works.
  }
}

useHead({ title: "Store" });

const route = useRoute();
const router = useRouter();
const api = useServerApi();

// Deep link: /store?gameId=... opens that game's detail page directly.
// Used by other surfaces (e.g. error pages, library) to send users to a
// specific game's store entry — desktop sends them to the dedicated
// /store/[id] page now (BPM-parity for the click-from-store flow).
const incomingGameId = route.query.gameId?.toString();
if (incomingGameId) {
  router.replace(`/store/${incomingGameId}`);
}

const tabs = [
  { label: "Featured", value: "featured" },
  { label: "Browse", value: "browse" },
  { label: "Collections", value: "collections" },
] as const;

// Refs seed directly from the snapshot (module state, falling back
// to sessionStorage) instead of going through a post-mount "now
// patch the refs" dance.  That dance was racy: the lazy-load watch
// on `activeTab` fires when the value flips from "featured" →
// "browse", and Vue's scheduler runs that callback before any
// nextTick-scheduled suppression flag has had a chance to clear.
// By initialising refs from the snapshot, the value never changes —
// no watch fires — no `loadBrowse(true)` clobbering the restored
// `browsePage`.
//
// We read once into a local `initial` constant so all refs see the
// SAME snapshot — if we called `readBrowseSnapshot()` per-ref and
// the module re-loaded between calls (unlikely but possible during
// dev HMR) the refs could end up with a mix of restored + default
// values.
const initial = readBrowseSnapshot();

const activeTab = ref<(typeof tabs)[number]["value"]>(
  initial?.activeTab ?? "featured",
);

// State
const loading = ref(true);
const browseLoading = ref(false);

// Search uses a controlled input (`searchInput`) plus a committed value
// (`searchQuery`) that the query layer reads. Enter/submit promotes the
// input to the query and switches to Browse. Keeps the typing experience
// snappy without spamming requests on every keystroke.
const searchInput = ref(initial?.searchQuery ?? "");
const searchQuery = ref(initial?.searchQuery ?? "");
const heroIndex = ref(0);
// "Newest" was dropped: it sorted by Game.mReleased (the metadata provider's
// original release date) which gives wrong answers for remakes/remasters
// and isn't what users actually want here. The two useful axes are
// "Recently added" (Game.created — when admins added it to this Drop) and
// "Recently updated" (max version.created — when the catalogue last changed
// for this game). Default to Recently added.
const browseSort = ref<"recent" | "updated" | "name">(
  initial?.browseSort ?? "recent",
);

// ── Filter state ──────────────────────────────────────────────────────────
//
// Two flavours of filter live side-by-side:
//   • Server-side (sent to /api/v1/store): tag IDs, library, sort, search.
//     The API only knows about these.
//   • Client-side (post-filter on the returned page): developer, publisher,
//     release year range, emulated/native. The server doesn't expose
//     filters for these but every relevant field is already on `StoreGame`,
//     so we can match locally without round-tripping.
//
// The split matters for pagination: server-side filters narrow the result
// set the server pages over (and the page nav counts pages of that set),
// while client-side filters just hide rows from the already-loaded page.
// The "showing X–Y of Z" caption reports the *server* count, so it's
// accurate even when client-side filters are visually dropping rows.
const selectedTagIds = ref<string[]>(
  initial?.selectedTagIds ? [...initial.selectedTagIds] : [],
);
// How multiple selected tags combine: "or" = any (default), "and" = all
// (intersection). Only meaningful with 2+ tags selected; the toggle and the
// query param are both gated on that.
const tagMode = ref<"and" | "or">(initial?.tagMode ?? "or");
const selectedLibraryId = ref<string>(initial?.selectedLibraryId ?? "");
const selectedPlatforms = ref<string[]>(
  initial?.selectedPlatforms
    ? [...initial.selectedPlatforms]
    : [],
);
const tagSearch = ref("");
const emulatedFilter = ref<"all" | "native" | "rom">(
  initial?.emulatedFilter ?? "all",
);
const releaseYearFrom = ref<number | null>(
  initial?.releaseYearFrom ?? null,
);
const releaseYearTo = ref<number | null>(
  initial?.releaseYearTo ?? null,
);
const filterDrawerOpen = ref(false);

const emulatedOptions = [
  { label: "All", value: "all" as const },
  { label: "Native", value: "native" as const },
  { label: "ROM", value: "rom" as const },
];

// Each chip maps 1:1 onto a column / aggregate on the server.
//   recent  = Game.created            → "added to this Drop"
//   updated = max(versions.created)   → "catalogue entry last changed"
//   name    = Game.mName              → alphabetical
const sortOptions = [
  { label: "Recently added", value: "recent" as const },
  { label: "Recently updated", value: "updated" as const },
  { label: "A–Z", value: "name" as const },
];

// Featured data
const featured = ref<StoreGame[]>([]);
const trending = ref<TrendingGame[]>([]);
const recentGames = ref<StoreGame[]>([]);

// "Recently Updated" — separate fetch with sort='updated', distinct
// from `recentGames` (sort='recent') because the two answer different
// questions ("what changed lately" vs "what was added lately").
const recentlyUpdated = ref<StoreGame[]>([]);

// Game IDs the caller already owns — drives the "In library" badge on
// the new Featured-tab widgets. Lazy: empty Set until the membership
// fetch lands, so the badges just don't render until then.
const libraryGameIdSet = ref<Set<string>>(new Set());

// Discovery tabs (New / Most Played / Random).  Each tab keeps its
// own cached game list so flipping back to a previously-loaded tab is
// instant.  `discoveryListGames` is just the active tab's slice.
type DiscoveryTabValue = "new" | "popular" | "random";
const discoveryTab = ref<DiscoveryTabValue>("new");
const discoveryLoading = ref(false);
const discoveryByTab = reactive({
  new: [] as StoreGame[],
  popular: [] as StoreGame[],
  random: [] as StoreGame[],
});
const discoveryListGames = computed(() => discoveryByTab[discoveryTab.value]);

// Browse data + pagination state. We use classic numbered pagination
// (page 1 .. N) rather than "load more" — round-tripping back and
// forth between pages stays clean because each page render replaces
// the result set instead of appending.
//
// `BROWSE_PAGE_SIZE` is computed once at mount as `cols × 5` so each
// page renders as a perfect rectangle (no partial trailing row).  We
// don't re-compute on resize: the trade-off would be re-fetching the
// catalog every time the user nudged the window, and the existing
// page layout still reads fine if the user does resize between
// fetches. The col count derives from the same breakpoints
// GameTileGrid uses (3 / 4 / 5 / 6 / 7 / 8).
function computeBrowsePageSize(): number {
  if (typeof window === "undefined") return 40;
  const w = window.innerWidth;
  const cols =
    w >= 1536 ? 8 : w >= 1280 ? 7 : w >= 1024 ? 6 : w >= 768 ? 5 : w >= 640 ? 4 : 3;
  return cols * 5;
}
const BROWSE_PAGE_SIZE = computeBrowsePageSize();
const browseResults = ref<StoreGame[]>([]);
const browseTotal = ref(0);
const browsePage = ref(initial?.browsePage ?? 1);

// Tag + library catalogs (loaded lazily once we land on the Browse tab).
const allTags = ref<StoreTag[]>([]);
const libraries = ref<Array<{ id: string; name: string }>>([]);

function objectUrl(id: string): string {
  return serverUrl(`api/v1/object/${id}`);
}

function goToGame(gameId?: string) {
  if (!gameId) return;
  // Prefetch via deduplicated invoke so the detail page mounts fast.
  // Click from the store always lands on the store presentation
  // (/store/[id]) — the library detail page is reserved for the
  // owns-the-game / install-management surface.
  deduplicatedInvoke("fetch_game", { gameId }).catch(() => {});
  router.push(`/store/${gameId}`);
}

// Cover pool for the roulette spin animation. We seed the wheel from
// whatever covers we've already pulled for other surfaces of this page
// (featured carousel, trending shelf, recent shelf, current browse
// results) so the animation has something to flip through without
// paying an extra fetch. De-duped + capped at 40. The Featured tab is
// usually loaded by the time the user lands on Browse, so this pool is
// almost always non-empty on first spin.
const rouletteCoverPool = computed(() => {
  const pool = new Set<string>();
  for (const g of featured.value) {
    if (g.mCoverObjectId) pool.add(g.mCoverObjectId);
  }
  for (const g of trending.value) {
    if (g.mCoverObjectId) pool.add(g.mCoverObjectId);
  }
  for (const g of recentGames.value) {
    if (g.mCoverObjectId) pool.add(g.mCoverObjectId);
  }
  for (const g of browseResults.value) {
    if (g.mCoverObjectId) pool.add(g.mCoverObjectId);
  }
  return [...pool].slice(0, 40);
});

function onRouletteSelect(payload: { gameId: string; owned: boolean }) {
  // The roulette card pushes its own destination route; we only side-
  // effect the metadata prefetch so the destination page hydrates fast.
  // (Mirrors goToGame's deduplicated invoke pattern.)
  deduplicatedInvoke("fetch_game", { gameId: payload.gameId }).catch(() => {});
}

function submitSearch() {
  searchQuery.value = searchInput.value.trim();
  activeTab.value = "browse";
  loadBrowse(true);
}

// Clear both the input and the committed query, then refresh the
// browse list so the grid stops being filtered. Hooked to the X
// button in the header and the Escape key on the input.
function clearSearch() {
  if (!searchInput.value && !searchQuery.value) return;
  searchInput.value = "";
  searchQuery.value = "";
  if (activeTab.value === "browse") {
    loadBrowse(true);
  }
}

function setSort(value: typeof browseSort.value) {
  browseSort.value = value;
  loadBrowse(true);
}

function toggleTag(tagId: string) {
  const i = selectedTagIds.value.indexOf(tagId);
  if (i === -1) selectedTagIds.value.push(tagId);
  else selectedTagIds.value.splice(i, 1);
  loadBrowse(true);
}

function setTagMode(mode: "and" | "or") {
  if (tagMode.value === mode) return;
  tagMode.value = mode;
  // Only re-query when 2+ tags are selected — with 0 or 1 tag the mode
  // doesn't change the result set.
  if (selectedTagIds.value.length >= 2) loadBrowse(true);
}

function togglePlatform(platform: string) {
  const i = selectedPlatforms.value.indexOf(platform);
  if (i === -1) selectedPlatforms.value.push(platform);
  else selectedPlatforms.value.splice(i, 1);
  loadBrowse(true);
}

function clearBrowseFilters() {
  searchInput.value = "";
  searchQuery.value = "";
  selectedTagIds.value = [];
  selectedLibraryId.value = "";
  selectedPlatforms.value = [];
  tagSearch.value = "";
  emulatedFilter.value = "all";
  releaseYearFrom.value = null;
  releaseYearTo.value = null;
  loadBrowse(true);
}

// Platform options — discovered from whatever launchPlatform values
// appear in the loaded result set. Server has no /platforms endpoint, so
// the dropdown grows as the user pages through the catalog.
const availablePlatforms = computed(() => {
  const set = new Set<string>();
  for (const g of browseResults.value) {
    if (g.launchPlatform) set.add(g.launchPlatform);
  }
  return [...set].sort();
});

// Filter the tag list by the in-drawer search box. Avoids forcing the
// user to scroll through hundreds of catalog tags when they know what
// they're looking for.
const filteredTagList = computed(() => {
  const q = tagSearch.value.trim().toLowerCase();
  if (!q) return allTags.value;
  return allTags.value.filter((t) => t.name.toLowerCase().includes(q));
});

// How many advanced (non-default) filters are active. Drives the badge
// count on the "Filters" button so the user can see at a glance whether
// they've narrowed the catalog and need to clear before browsing fresh.
const activeAdvancedFilterCount = computed(() => {
  let n = 0;
  n += selectedTagIds.value.length;
  if (selectedLibraryId.value) n += 1;
  n += selectedPlatforms.value.length;
  if (emulatedFilter.value !== "all") n += 1;
  if (releaseYearFrom.value || releaseYearTo.value) n += 1;
  return n;
});

const hasActiveFilters = computed(
  () => Boolean(searchQuery.value) || activeAdvancedFilterCount.value > 0,
);

// Chip row above the grid — one chip per active filter so the user can
// pop them off without re-opening the drawer.
type FilterChip =
  | { key: string; kind: "tag"; tagId: string; label: string; value: string }
  | { key: string; kind: "library"; label: string; value: string }
  | {
      key: string;
      kind: "platform";
      platform: string;
      label: string;
      value: string;
    }
  | { key: string; kind: "emulated"; label: string; value: string }
  | { key: string; kind: "year"; label: string; value: string };

const activeFilterChips = computed<FilterChip[]>(() => {
  const chips: FilterChip[] = [];
  for (const id of selectedTagIds.value) {
    const tag = allTags.value.find((t) => t.id === id);
    chips.push({
      key: `tag:${id}`,
      kind: "tag",
      tagId: id,
      label: "Tag",
      value: tag?.name ?? id,
    });
  }
  if (selectedLibraryId.value) {
    const lib = libraries.value.find((l) => l.id === selectedLibraryId.value);
    chips.push({
      key: "library",
      kind: "library",
      label: "Library",
      value: lib?.name ?? selectedLibraryId.value,
    });
  }
  for (const p of selectedPlatforms.value) {
    chips.push({
      key: `platform:${p}`,
      kind: "platform",
      platform: p,
      label: "Platform",
      value: p,
    });
  }
  if (emulatedFilter.value !== "all") {
    chips.push({
      key: "emulated",
      kind: "emulated",
      label: "Type",
      value: emulatedFilter.value === "rom" ? "ROM" : "Native",
    });
  }
  if (releaseYearFrom.value || releaseYearTo.value) {
    const from = releaseYearFrom.value ?? "";
    const to = releaseYearTo.value ?? "";
    chips.push({
      key: "year",
      kind: "year",
      label: "Year",
      value: `${from}${from || to ? "–" : ""}${to}`,
    });
  }
  return chips;
});

function removeFilterChip(chip: FilterChip) {
  switch (chip.kind) {
    case "tag":
      toggleTag(chip.tagId);
      return;
    case "library":
      selectedLibraryId.value = "";
      loadBrowse(true);
      return;
    case "platform":
      togglePlatform(chip.platform);
      return;
    case "emulated":
      emulatedFilter.value = "all";
      return;
    case "year":
      releaseYearFrom.value = null;
      releaseYearTo.value = null;
      return;
  }
}

// "Try removing:" hints rendered under the empty state. We expose the
// up-to-3 most likely culprits in priority order: a free-text search
// query first (most aggressive), then narrowed metadata (year range,
// emulated/native), then membership filters (tags, library, platform).
// Each suggestion is a self-contained `{ label, value, apply }` so the
// empty-state template can render and dispatch without knowing the
// shape of any one filter.
type EmptyStateSuggestion = {
  key: string;
  label: string;
  value: string;
  apply: () => void;
};

const emptyStateSuggestions = computed<EmptyStateSuggestion[]>(() => {
  const out: EmptyStateSuggestion[] = [];

  if (searchQuery.value) {
    out.push({
      key: "search",
      label: "Search",
      value: `"${searchQuery.value}"`,
      apply: () => clearSearch(),
    });
  }

  if (releaseYearFrom.value || releaseYearTo.value) {
    const from = releaseYearFrom.value ?? "";
    const to = releaseYearTo.value ?? "";
    out.push({
      key: "year",
      label: "Year",
      value: `${from}${from || to ? "–" : ""}${to}`,
      apply: () => {
        releaseYearFrom.value = null;
        releaseYearTo.value = null;
      },
    });
  }

  if (emulatedFilter.value !== "all") {
    out.push({
      key: "emulated",
      label: "Type",
      value: emulatedFilter.value === "rom" ? "ROM" : "Native",
      apply: () => {
        emulatedFilter.value = "all";
      },
    });
  }

  // Tag chips often dominate the filter set when present — fold the
  // first one in so the user gets a one-click escape if the picked
  // tag is too narrow.
  for (const id of selectedTagIds.value.slice(0, 2)) {
    const tag = allTags.value.find((t) => t.id === id);
    out.push({
      key: `tag:${id}`,
      label: "Tag",
      value: tag?.name ?? id,
      apply: () => toggleTag(id),
    });
  }

  if (selectedLibraryId.value) {
    const lib = libraries.value.find((l) => l.id === selectedLibraryId.value);
    out.push({
      key: "library",
      label: "Library",
      value: lib?.name ?? selectedLibraryId.value,
      apply: () => {
        selectedLibraryId.value = "";
        loadBrowse(true);
      },
    });
  }

  // Cap to 3 so the empty state stays a hint, not a second filter UI.
  return out.slice(0, 3);
});

// Apply client-side filters on top of whatever the server returned.
// Server already handled tags/library/platform/sort/search; we layer
// the two fields the API doesn't expose as filters: year and emulation
// type. Both are returned per-game (mReleased + isEmulated) so the
// filter is exact, not heuristic.
const displayedResults = computed<StoreGame[]>(() => {
  const yFrom = releaseYearFrom.value;
  const yTo = releaseYearTo.value;
  const wantRom = emulatedFilter.value === "rom";
  const wantNative = emulatedFilter.value === "native";

  return browseResults.value.filter((g) => {
    if (wantRom && !g.isEmulated) return false;
    if (wantNative && g.isEmulated) return false;

    if (yFrom != null || yTo != null) {
      const released = g.mReleased ? new Date(g.mReleased) : null;
      const year = released && !isNaN(released.getTime())
        ? released.getFullYear()
        : null;
      if (year == null) return false;
      if (yFrom != null && year < yFrom) return false;
      if (yTo != null && year > yTo) return false;
    }

    return true;
  });
});

// Total page count derived from server count + page size.  At least
// 1 so the pagination bar never disappears mid-load on a transient
// `browseTotal === 0`.
const browseTotalPages = computed(() =>
  Math.max(1, Math.ceil(browseTotal.value / BROWSE_PAGE_SIZE)),
);

// 1-indexed "showing X–Y of Z" range for the small caption under the
// page buttons. End is clamped against `browseTotal` for the final
// page when the result count isn't a clean multiple of page size.
const browsePageStart = computed(() =>
  browseTotal.value === 0 ? 0 : (browsePage.value - 1) * BROWSE_PAGE_SIZE + 1,
);
const browsePageEnd = computed(() =>
  Math.min(browsePage.value * BROWSE_PAGE_SIZE, browseTotal.value),
);

// Visible page-number tokens for the pagination bar. Collapses long
// sequences with '...' so the bar stays compact past ~7 pages.
// Strategy: always show first + last + current ± 1, with ellipsis
// markers where there are gaps. Returns `number | '...'` so the
// template can distinguish clickable cells from spacers.
const browsePageNumbers = computed<Array<number | "...">>(() => {
  const total = browseTotalPages.value;
  const current = browsePage.value;
  if (total <= 7) {
    return Array.from({ length: total }, (_, i) => i + 1);
  }
  const pages: Array<number | "..."> = [1];
  const left = Math.max(2, current - 1);
  const right = Math.min(total - 1, current + 1);
  if (left > 2) pages.push("...");
  for (let p = left; p <= right; p++) pages.push(p);
  if (right < total - 1) pages.push("...");
  pages.push(total);
  return pages;
});

// Hero auto-advance every 8s. Manual dot clicks reset the timer so the
// user doesn't get yanked to the next slide a beat after their click —
// `goToHeroSlide` is the canonical entry point for those clicks.
let heroInterval: ReturnType<typeof setInterval> | null = null;
function startHeroRotation() {
  stopHeroRotation();
  heroInterval = setInterval(() => {
    if (featured.value.length > 1) {
      heroIndex.value = (heroIndex.value + 1) % featured.value.length;
    }
  }, 8_000);
}
function stopHeroRotation() {
  if (heroInterval) {
    clearInterval(heroInterval);
    heroInterval = null;
  }
}
function goToHeroSlide(i: number) {
  heroIndex.value = i;
  // Reset the cadence so the next auto-advance is a full 8s away,
  // not "whatever fraction of 8s was left on the prior tick."
  startHeroRotation();
}

async function loadFeaturedData() {
  try {
    const [feat, trend, recent, updated] = await Promise.all([
      api.store.featured().catch(() => [] as StoreGame[]),
      api.store
        .trending(7, 7)
        .then((d) => d.results)
        .catch(() => [] as TrendingGame[]),
      // Carry-over: kept for the discovery tab's "New" feed even
      // though the standalone "Recently Added" shelf was retired in
      // favour of the discovery widget. `sort='recent'` orders by
      // Game.created (when an admin added it to this Drop instance).
      api.store
        .browse({ sort: "recent", take: 14 })
        .then((d) => d.results)
        .catch(() => [] as StoreGame[]),
      // Recently Updated — sort='updated' is max(version.created),
      // so this surfaces games whose catalogue entries changed lately
      // (new version uploaded, metadata edit, etc.) — distinct from
      // Recently Added.
      api.store
        .browse({ sort: "updated", take: 12 })
        .then((d) => d.results)
        .catch(() => [] as StoreGame[]),
    ]);
    featured.value = feat;
    trending.value = trend;
    recentGames.value = recent;
    recentlyUpdated.value = updated;

    // Seed the discovery tabs once the featured/trending fetches are
    // in flight — "new" reuses the recent list, "popular" reuses the
    // trending list (typed as TrendingGame which is a StoreGame
    // superset).
    discoveryByTab.new = recent;
    discoveryByTab.popular = trend as unknown as StoreGame[];
  } catch (e) {
    console.error("[STORE] Failed to load featured:", e);
  }
}

/** Lazy-fetch the games for whichever discovery tab the user selects.
 *  Cached per tab so flipping back to a previously-loaded tab is
 *  instant; the "random" tab refetches each time the user picks it so
 *  they always get a fresh shuffle. */
async function ensureDiscoveryTabLoaded(t: DiscoveryTabValue) {
  if (t !== "random" && discoveryByTab[t].length > 0) return;
  discoveryLoading.value = true;
  try {
    if (t === "new") {
      const data = await api.store.browse({ sort: "recent", take: 14 });
      discoveryByTab.new = data.results;
    } else if (t === "popular") {
      const data = await api.store.trending(14, 30);
      discoveryByTab.popular = data.results as unknown as StoreGame[];
    } else {
      const data = await api.store.browse({ sort: "random", take: 14 });
      discoveryByTab.random = data.results;
    }
  } catch (e) {
    console.warn("[STORE] Failed to load discovery tab", t, e);
  } finally {
    discoveryLoading.value = false;
  }
}

function onDiscoveryTabChange(t: DiscoveryTabValue) {
  discoveryTab.value = t;
  ensureDiscoveryTabLoaded(t);
}

function onDiscoverySeeMore(t: DiscoveryTabValue) {
  // "See more" → jump to Browse with the matching server-side sort
  // pre-applied. Random doesn't have a corresponding browse sort that
  // makes sense for a See More flow; route it to the default browse.
  searchInput.value = "";
  searchQuery.value = "";
  if (t === "new") browseSort.value = "recent";
  else if (t === "popular") browseSort.value = "updated";
  // (random falls through to whatever sort is current.)
  activeTab.value = "browse";
  browsePage.value = 1;
  loadBrowse(true);
}

function goToBrowseUpdated() {
  searchInput.value = "";
  searchQuery.value = "";
  browseSort.value = "updated";
  activeTab.value = "browse";
  browsePage.value = 1;
  loadBrowse(true);
}

/** Resolve which games the caller already has in their library so the
 *  Featured-tab widgets can surface an "In library" badge.  Soft-fails
 *  to an empty set if the membership endpoint is unreachable. */
async function loadLibraryMembership() {
  try {
    const res = await fetch(serverUrl("api/v1/collection/default"));
    if (!res.ok) return;
    const collection = await res.json();
    const entries: Array<{ gameId: string }> = collection.entries ?? [];
    libraryGameIdSet.value = new Set(entries.map((e) => e.gameId));
  } catch (e) {
    console.warn("[STORE] Library membership fetch failed:", e);
  }
}

// ── Collections (curated store collections) ───────────────────────────────
// Lazy-loaded the first time the user opens the Collections tab so the
// Featured paint isn't blocked on a fetch they may never look at.
const collections = ref<StoreCollectionSummary[]>([]);
const collectionsLoading = ref(false);

async function loadCollections() {
  if (collections.value.length > 0 || collectionsLoading.value) return;
  collectionsLoading.value = true;
  try {
    collections.value = await api.store.collections();
  } catch (e) {
    console.error("[STORE] Failed to load collections:", e);
  } finally {
    collectionsLoading.value = false;
  }
}

function goToCollection(id: string) {
  router.push(`/store/collection/${id}`);
}

// Build the server-side query — only the params the API actually
// understands. CSV the tag IDs together since that's what /api/v1/store
// expects (single `tags=a,b,c` query string).
type BrowseParams = NonNullable<Parameters<typeof api.store.browse>[0]>;

function buildBrowseParams(skip: number): BrowseParams {
  // Narrow to the literal union the API typings expect — TS otherwise
  // widens `effectiveSort` to plain `string` since it's a conditional
  // result from refs typed as broader unions.
  const effectiveSort: NonNullable<BrowseParams["sort"]> = searchQuery.value
    ? "relevance"
    : browseSort.value;
  return {
    skip,
    take: BROWSE_PAGE_SIZE,
    q: searchQuery.value || undefined,
    tags: selectedTagIds.value.length
      ? selectedTagIds.value.join(",")
      : undefined,
    // Only send the non-default "and" when it actually changes the result
    // (2+ tags). Keeps the common single-tag query clean.
    tagMode:
      selectedTagIds.value.length >= 2 && tagMode.value === "and"
        ? "and"
        : undefined,
    library: selectedLibraryId.value || undefined,
    platform: selectedPlatforms.value.length
      ? selectedPlatforms.value.join(",")
      : undefined,
    sort: effectiveSort,
    order: effectiveSort === "name" ? "asc" : undefined,
  };
}

async function loadBrowse(reset = false) {
  if (reset) {
    browseResults.value = [];
    browsePage.value = 1;
  }
  browseLoading.value = true;
  try {
    const data = await api.store.browse(
      buildBrowseParams((browsePage.value - 1) * BROWSE_PAGE_SIZE),
    );
    // Replace, don't append — each page render is a clean fetch so
    // round-tripping (page 7 → page 3 → page 7) never doubles up rows.
    browseResults.value = data.results;
    browseTotal.value = data.count;
  } catch (e) {
    console.error("[STORE] Failed to load browse:", e);
  } finally {
    browseLoading.value = false;
  }
}

/** Jump to a specific page (1-indexed) and reload. Clamps to valid
 *  range so a stale UI click on a page that no longer exists (e.g.
 *  after a filter narrowed the result set) doesn't blow up. */
function goToPage(p: number) {
  if (browseLoading.value) return;
  const target = Math.max(1, Math.min(p, browseTotalPages.value));
  if (target === browsePage.value) return;
  browsePage.value = target;
  loadBrowse(false);
  // Scroll the grid back to the top so the user sees page 1 of the
  // new page, not whatever they were looking at on the prior one.
  if (typeof window !== "undefined") {
    nextTick(() => window.scrollTo({ top: 0, behavior: "smooth" }));
  }
}

// Tags + libraries — cheap-to-fetch reference data we pull once and reuse
// for the drawer. Failures are non-fatal: the drawer just hides the
// corresponding section if either list is empty.
async function loadFilterCatalogs() {
  const [tags, libs] = await Promise.all([
    api.store.tags().catch(() => [] as StoreTag[]),
    api.store.libraries().catch(
      () => [] as Array<{ id: string; name: string }>,
    ),
  ]);
  allTags.value = [...tags].sort((a, b) => a.name.localeCompare(b.name));
  libraries.value = [...libs].sort((a, b) => a.name.localeCompare(b.name));
}

/** Snapshot the current Browse view to module state AND
 *  sessionStorage.  Module state is the fast path; sessionStorage
 *  is the survivor that lets us recover after HMR, a route prefetch
 *  that re-evaluated the module, or any unexpected wipe of the
 *  module-level `let`.  Cheap: just a few refs copied into a plain
 *  object plus one JSON.stringify. */
function snapshotBrowseState() {
  writeBrowseSnapshot({
    activeTab: activeTab.value,
    searchQuery: searchQuery.value,
    browseSort: browseSort.value,
    browsePage: browsePage.value,
    selectedTagIds: [...selectedTagIds.value],
    tagMode: tagMode.value,
    selectedLibraryId: selectedLibraryId.value,
    selectedPlatforms: [...selectedPlatforms.value],
    emulatedFilter: emulatedFilter.value,
    releaseYearFrom: releaseYearFrom.value,
    releaseYearTo: releaseYearTo.value,
  });
}

onMounted(async () => {
  await Promise.all([
    loadFeaturedData(),
    loadFilterCatalogs(),
    loadLibraryMembership(),
  ]);
  // Browse is loaded lazily — only when the user actually opens that tab —
  // so the initial Featured render isn't blocked on a full-catalog query.
  loading.value = false;
  startHeroRotation();

  // If the snapshot put us straight on the Browse tab, the lazy-load
  // watch below won't fire (its initial value isn't a change), so we
  // kick off the fetch ourselves with the restored page + filters.
  if (activeTab.value === "browse") {
    loadBrowse(false);
  } else if (activeTab.value === "collections") {
    loadCollections();
  }
});

// Lazy-load browse when the user opens the tab for the first time
// within the current session. Snapshot-restored sessions already have
// their initial Browse fetch fired from onMounted above, so this watch
// only catches subsequent user-driven tab switches.
watch(activeTab, (tab) => {
  if (tab === "browse" && browseResults.value.length === 0 && !browseLoading.value) {
    loadBrowse(true);
  } else if (tab === "collections") {
    loadCollections();
  }
});

// Persist user-driven state changes to the module snapshot. The watch
// fires after Vue commits the new ref values, so by the time we
// snapshot we're capturing the latest view the user sees.  We watch
// the broad set of refs that together describe a "where am I in the
// Browse tab" answer; any one of them changing is meaningful.
watch(
  [
    activeTab,
    browsePage,
    searchQuery,
    browseSort,
    selectedTagIds,
    tagMode,
    selectedLibraryId,
    selectedPlatforms,
    emulatedFilter,
    releaseYearFrom,
    releaseYearTo,
  ],
  () => {
    snapshotBrowseState();
  },
  { deep: true },
);

onBeforeUnmount(() => {
  stopHeroRotation();
  // One final snapshot on unmount catches any in-flight state changes
  // that the watch's microtask hadn't flushed yet (e.g. clicking a
  // game tile fires goToGame which navigates immediately).
  snapshotBrowseState();
});
</script>
