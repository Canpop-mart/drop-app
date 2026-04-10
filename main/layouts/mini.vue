<template>
  <div class="flex flex-col bg-zinc-950 overflow-hidden h-screen">
    <MiniHeader />
    <div data-app-scroll class="relative grow overflow-y-auto">
      <slot />
    </div>
  </div>
</template>
